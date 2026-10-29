use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier, OnceLock};
use std::thread;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use hdf5_reader::{Hdf5File, OpenOptions, SliceInfo, SliceInfoElem};
#[cfg(feature = "bench-memory-profile")]
use peak_alloc::PeakAlloc;
use rayon::ThreadPoolBuilder;
use tempfile::TempDir;

use netcdf_reader::{NcFile, NcGroup, NcSliceInfo, NcSliceInfoElem};

#[cfg(feature = "bench-memory-profile")]
#[global_allocator]
static PEAK_ALLOC: PeakAlloc = PeakAlloc;

#[derive(Clone, Copy)]
enum NumericKind {
    F32,
    F64,
}

impl NumericKind {
    fn element_size(self) -> usize {
        match self {
            Self::F32 => std::mem::size_of::<f32>(),
            Self::F64 => std::mem::size_of::<f64>(),
        }
    }
}

#[derive(Clone, Copy)]
struct SliceSpec {
    start: &'static [usize],
    count: &'static [usize],
    stride: &'static [usize],
}

#[derive(Clone, Copy)]
enum FixtureSource {
    Existing {
        subdir: &'static str,
        file: &'static str,
    },
    Generated(GeneratedFixtureKind),
}

#[derive(Clone, Copy)]
enum GeneratedFixtureKind {
    LargeCdf5,
    LargeRecordCdf5,
    LargeNc4Compressed,
    NestedNc4Groups,
}

#[derive(Clone, Copy)]
struct BenchCase {
    id: &'static str,
    fixture: FixtureSource,
    variable: &'static str,
    kind: NumericKind,
    shape: &'static [usize],
    slice: Option<SliceSpec>,
    is_netcdf4: bool,
}

const SHAPE_CDF1_SIMPLE: &[usize] = &[5, 10];
const SHAPE_NC4_BASIC: &[usize] = &[5, 10];
const SHAPE_NC4_COMPRESSED: &[usize] = &[100, 100];
const SHAPE_NC4_GROUPS_TEMPERATURE: &[usize] = &[3];
const SHAPE_NESTED_NC4_PRESSURE: &[usize] = &[3];
const SHAPE_LARGE_CDF5: &[usize] = &[2048, 1024];
const SHAPE_LARGE_RECORD_CDF5: &[usize] = &[1024, 1024];
const SHAPE_LARGE_NC4_COMPRESSED: &[usize] = &[2048, 1024];

const UNIT_STRIDE_2D: &[usize] = &[1, 1];

const SLICE_NC4_BASIC: SliceSpec = SliceSpec {
    start: &[1, 2],
    count: &[3, 4],
    stride: UNIT_STRIDE_2D,
};
const HOT_SLICE_NC4_BASIC: SliceSpec = SliceSpec {
    start: &[1, 2],
    count: &[1, 1],
    stride: UNIT_STRIDE_2D,
};
const SLICE_NC4_COMPRESSED: SliceSpec = SliceSpec {
    start: &[12, 18],
    count: &[28, 35],
    stride: UNIT_STRIDE_2D,
};
const HOT_SLICE_NC4_COMPRESSED: SliceSpec = SliceSpec {
    start: &[12, 18],
    count: &[4, 4],
    stride: UNIT_STRIDE_2D,
};
const SLICE_LARGE_NC4_COMPRESSED: SliceSpec = SliceSpec {
    start: &[256, 192],
    count: &[384, 320],
    stride: UNIT_STRIDE_2D,
};
const HOT_SLICE_LARGE_NC4_COMPRESSED: SliceSpec = SliceSpec {
    start: &[256, 192],
    count: &[4, 4],
    stride: UNIT_STRIDE_2D,
};
const SLICE_LARGE_CDF5_STRIDED: SliceSpec = SliceSpec {
    start: &[256, 0],
    count: &[384, 1024],
    stride: &[3, 1],
};
const SLICE_LARGE_RECORD_CDF5_STRIDED: SliceSpec = SliceSpec {
    start: &[32, 16],
    count: &[160, 224],
    stride: &[3, 4],
};

const CASES: &[BenchCase] = &[
    BenchCase {
        id: "cdf1_simple",
        fixture: FixtureSource::Existing {
            subdir: "netcdf3",
            file: "cdf1_simple.nc",
        },
        variable: "temp",
        kind: NumericKind::F32,
        shape: SHAPE_CDF1_SIMPLE,
        slice: None,
        is_netcdf4: false,
    },
    BenchCase {
        id: "nc4_basic",
        fixture: FixtureSource::Existing {
            subdir: "netcdf4",
            file: "nc4_basic.nc",
        },
        variable: "data",
        kind: NumericKind::F64,
        shape: SHAPE_NC4_BASIC,
        slice: Some(SLICE_NC4_BASIC),
        is_netcdf4: true,
    },
    BenchCase {
        id: "nc4_compressed",
        fixture: FixtureSource::Existing {
            subdir: "netcdf4",
            file: "nc4_compressed.nc",
        },
        variable: "compressed",
        kind: NumericKind::F32,
        shape: SHAPE_NC4_COMPRESSED,
        slice: Some(SLICE_NC4_COMPRESSED),
        is_netcdf4: true,
    },
    BenchCase {
        id: "nc4_groups",
        fixture: FixtureSource::Existing {
            subdir: "netcdf4",
            file: "nc4_groups.nc",
        },
        variable: "obs/temperature",
        kind: NumericKind::F32,
        shape: SHAPE_NC4_GROUPS_TEMPERATURE,
        slice: None,
        is_netcdf4: true,
    },
    BenchCase {
        id: "nested_nc4_groups",
        fixture: FixtureSource::Generated(GeneratedFixtureKind::NestedNc4Groups),
        variable: "obs/surface/pressure",
        kind: NumericKind::F64,
        shape: SHAPE_NESTED_NC4_PRESSURE,
        slice: None,
        is_netcdf4: true,
    },
    BenchCase {
        id: "large_cdf5",
        fixture: FixtureSource::Generated(GeneratedFixtureKind::LargeCdf5),
        variable: "data",
        kind: NumericKind::F32,
        shape: SHAPE_LARGE_CDF5,
        slice: Some(SLICE_LARGE_CDF5_STRIDED),
        is_netcdf4: false,
    },
    BenchCase {
        id: "large_record_cdf5",
        fixture: FixtureSource::Generated(GeneratedFixtureKind::LargeRecordCdf5),
        variable: "series",
        kind: NumericKind::F32,
        shape: SHAPE_LARGE_RECORD_CDF5,
        slice: Some(SLICE_LARGE_RECORD_CDF5_STRIDED),
        is_netcdf4: false,
    },
    BenchCase {
        id: "large_nc4_compressed",
        fixture: FixtureSource::Generated(GeneratedFixtureKind::LargeNc4Compressed),
        variable: "compressed",
        kind: NumericKind::F32,
        shape: SHAPE_LARGE_NC4_COMPRESSED,
        slice: Some(SLICE_LARGE_NC4_COMPRESSED),
        is_netcdf4: true,
    },
];

struct GeneratedFixtures {
    _temp_dir: TempDir,
    large_cdf5: PathBuf,
    large_record_cdf5: PathBuf,
    large_nc4_compressed: PathBuf,
    nested_nc4_groups: PathBuf,
    sparse_huge_logical_nc4: PathBuf,
}

static GENERATED_FIXTURES: OnceLock<GeneratedFixtures> = OnceLock::new();
static VALIDATION_ONCE: OnceLock<()> = OnceLock::new();

fn existing_fixture_path(subdir: &str, file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("testdata")
        .join(subdir)
        .join(file)
}

fn generated_fixtures() -> &'static GeneratedFixtures {
    GENERATED_FIXTURES.get_or_init(|| {
        let temp_dir = tempfile::tempdir().unwrap();
        let large_cdf5 = temp_dir.path().join("bench_large_cdf5.nc");
        let large_record_cdf5 = temp_dir.path().join("bench_large_record_cdf5.nc");
        let large_nc4_compressed = temp_dir.path().join("bench_large_nc4_compressed.nc");
        let nested_nc4_groups = temp_dir.path().join("bench_nested_nc4_groups.nc");
        let sparse_huge_logical_nc4 = temp_dir.path().join("bench_sparse_huge_logical_nc4.nc");

        create_large_cdf5_fixture(&large_cdf5);
        create_large_record_cdf5_fixture(&large_record_cdf5);
        create_large_nc4_compressed_fixture(&large_nc4_compressed);
        create_nested_nc4_groups_fixture(&nested_nc4_groups);
        create_sparse_huge_logical_nc4_fixture(&sparse_huge_logical_nc4);

        GeneratedFixtures {
            _temp_dir: temp_dir,
            large_cdf5,
            large_record_cdf5,
            large_nc4_compressed,
            nested_nc4_groups,
            sparse_huge_logical_nc4,
        }
    })
}

fn create_large_cdf5_fixture(path: &Path) {
    let mut file = netcdf::create_with(path, netcdf::Options::_64BIT_DATA).unwrap();
    file.add_dimension("row", SHAPE_LARGE_CDF5[0]).unwrap();
    file.add_dimension("col", SHAPE_LARGE_CDF5[1]).unwrap();
    file.add_variable::<f32>("data", &["row", "col"]).unwrap();
    file.enddef().unwrap();
    let mut variable = file.variable_mut("data").unwrap();
    for row in 0..SHAPE_LARGE_CDF5[0] {
        let values: Vec<f32> = (0..SHAPE_LARGE_CDF5[1])
            .map(|col| {
                ((row * 17 + col * 3) % 2048) as f32 * 0.25 + ((row + col) % 11) as f32 * 0.01
            })
            .collect();
        variable.put_values(&values, (row, ..)).unwrap();
    }
}

fn create_large_record_cdf5_fixture(path: &Path) {
    let mut file = netcdf::create_with(path, netcdf::Options::_64BIT_DATA).unwrap();
    file.add_unlimited_dimension("time").unwrap();
    file.add_dimension("col", SHAPE_LARGE_RECORD_CDF5[1])
        .unwrap();
    file.add_variable::<f32>("series", &["time", "col"])
        .unwrap();
    file.enddef().unwrap();

    let mut variable = file.variable_mut("series").unwrap();
    for record in 0..SHAPE_LARGE_RECORD_CDF5[0] {
        let values: Vec<f32> = (0..SHAPE_LARGE_RECORD_CDF5[1])
            .map(|col| {
                let coarse = ((record * 13 + col * 7) % 2048) as f32 * 0.125;
                let fine = ((record + col * 3) % 17) as f32 * 0.03125;
                coarse + fine
            })
            .collect();
        variable.put_values(&values, (record, ..)).unwrap();
    }
}

fn create_large_nc4_compressed_fixture(path: &Path) {
    let mut file = netcdf::create_with(path, netcdf::Options::NETCDF4).unwrap();
    file.add_dimension("row", SHAPE_LARGE_NC4_COMPRESSED[0])
        .unwrap();
    file.add_dimension("col", SHAPE_LARGE_NC4_COMPRESSED[1])
        .unwrap();
    {
        let mut variable = file
            .add_variable::<f32>("compressed", &["row", "col"])
            .unwrap();
        variable.set_chunking(&[128, 128]).unwrap();
        variable.set_compression(4, true).unwrap();
    }
    file.enddef().unwrap();
    let mut variable = file.variable_mut("compressed").unwrap();

    for row in 0..SHAPE_LARGE_NC4_COMPRESSED[0] {
        let values: Vec<f32> = (0..SHAPE_LARGE_NC4_COMPRESSED[1])
            .map(|col| {
                let coarse = ((row / 8 + col / 16) % 64) as f32;
                let fine = ((row * 31 + col * 7) % 9) as f32 * 0.125;
                coarse + fine
            })
            .collect();
        variable.put_values(&values, (row, ..)).unwrap();
    }
}

fn create_nested_nc4_groups_fixture(path: &Path) {
    let mut file = netcdf::create_with(path, netcdf::Options::NETCDF4).unwrap();
    file.add_dimension("station", 4).unwrap();
    file.add_variable::<f32>("root_series", &["station"])
        .unwrap();
    {
        let mut obs = file.add_group("obs").unwrap();
        obs.add_dimension("time", SHAPE_NESTED_NC4_PRESSURE[0])
            .unwrap();
        obs.add_variable::<f32>("temperature", &["time"]).unwrap();
        let mut surface = obs.add_group("surface").unwrap();
        surface.add_variable::<f64>("pressure", &["time"]).unwrap();
    }

    file.enddef().unwrap();

    {
        let mut root = file.variable_mut("root_series").unwrap();
        let root_data = [0.0_f32, 1.0, 2.0, 3.0];
        root.put_values(&root_data, (..,)).unwrap();
    }
    {
        let mut obs = file.group_mut("obs").unwrap().unwrap();
        let mut temperature = obs.variable_mut("temperature").unwrap();
        let temperature_data = [20.5_f32, 21.0, 19.8];
        temperature.put_values(&temperature_data, (..,)).unwrap();
    }
    {
        let mut surface = file.group_mut("obs/surface").unwrap().unwrap();
        let mut pressure = surface.variable_mut("pressure").unwrap();
        let pressure_data = [1013.25_f64, 1012.0, 1014.5];
        pressure.put_values(&pressure_data, (..,)).unwrap();
    }
}

fn create_sparse_huge_logical_nc4_fixture(path: &Path) {
    const HUGE_DIM: usize = 1 << 20;

    let mut file = netcdf::create_with(path, netcdf::Options::NETCDF4).unwrap();
    file.add_dimension("row", HUGE_DIM).unwrap();
    file.add_dimension("col", HUGE_DIM).unwrap();
    {
        let mut variable = file.add_variable::<f32>("sparse", &["row", "col"]).unwrap();
        variable.set_chunking(&[1024, 1024]).unwrap();
        variable.set_fill_value(42.5_f32).unwrap();
    }
    file.enddef().unwrap();
}

fn case_path(case: &BenchCase) -> PathBuf {
    match case.fixture {
        FixtureSource::Existing { subdir, file } => {
            let path = existing_fixture_path(subdir, file);
            assert!(path.exists(), "missing fixture {}", path.display());
            path
        }
        FixtureSource::Generated(GeneratedFixtureKind::LargeCdf5) => {
            generated_fixtures().large_cdf5.clone()
        }
        FixtureSource::Generated(GeneratedFixtureKind::LargeRecordCdf5) => {
            generated_fixtures().large_record_cdf5.clone()
        }
        FixtureSource::Generated(GeneratedFixtureKind::LargeNc4Compressed) => {
            generated_fixtures().large_nc4_compressed.clone()
        }
        FixtureSource::Generated(GeneratedFixtureKind::NestedNc4Groups) => {
            generated_fixtures().nested_nc4_groups.clone()
        }
    }
}

fn case_elements(case: &BenchCase) -> usize {
    case.shape
        .iter()
        .copied()
        .fold(1usize, usize::saturating_mul)
}

fn case_bytes(case: &BenchCase) -> usize {
    case_elements(case) * case.kind.element_size()
}

fn slice_bytes(case: &BenchCase) -> Option<usize> {
    case.slice.map(|slice| {
        slice
            .count
            .iter()
            .copied()
            .fold(1usize, usize::saturating_mul)
            * case.kind.element_size()
    })
}

fn hot_slice_for_case(case: &BenchCase) -> Option<SliceSpec> {
    match case.id {
        "nc4_basic" => Some(HOT_SLICE_NC4_BASIC),
        "nc4_compressed" => Some(HOT_SLICE_NC4_COMPRESSED),
        "large_nc4_compressed" => Some(HOT_SLICE_LARGE_NC4_COMPRESSED),
        _ => None,
    }
}

fn thread_counts() -> Vec<usize> {
    if let Ok(raw) = std::env::var("BENCH_THREAD_LIST") {
        let mut values: Vec<_> = raw
            .split(',')
            .filter_map(|part| part.trim().parse::<usize>().ok())
            .filter(|threads| *threads > 0)
            .collect();
        values.sort_unstable();
        values.dedup();
        if !values.is_empty() {
            return values;
        }
    }

    let available = thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(4);

    let mut values = vec![1, 2, 4, 8];
    values.retain(|threads| *threads <= available.max(1));
    if values.last().copied() != Some(available) {
        values.push(available.max(1));
    }
    values.sort_unstable();
    values.dedup();
    values
}

fn hot_ops_per_thread() -> usize {
    std::env::var("BENCH_HOT_OPS_PER_THREAD")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|count| *count > 0)
        .unwrap_or(256)
}

fn checksum_f32<'a>(values: impl IntoIterator<Item = &'a f32>) -> u64 {
    let mut len = 0usize;
    let sum = values.into_iter().fold(0.0_f64, |acc, value| {
        len += 1;
        acc + f64::from(*value)
    });
    sum.to_bits() ^ len as u64
}

fn checksum_f64<'a>(values: impl IntoIterator<Item = &'a f64>) -> u64 {
    let mut len = 0usize;
    let sum = values.into_iter().fold(0.0_f64, |acc, value| {
        len += 1;
        acc + *value
    });
    sum.to_bits() ^ len as u64
}

fn full_read_checksum_netcdf_rust_file(file: &NcFile, case: &BenchCase) -> u64 {
    match case.kind {
        NumericKind::F32 => checksum_f32(file.read_variable::<f32>(case.variable).unwrap().iter()),
        NumericKind::F64 => checksum_f64(file.read_variable::<f64>(case.variable).unwrap().iter()),
    }
}

fn full_read_checksum_netcdf_rust_file_in_pool(
    file: &NcFile,
    case: &BenchCase,
    pool: &rayon::ThreadPool,
) -> u64 {
    match case.kind {
        NumericKind::F32 => checksum_f32(
            file.read_variable_in_pool::<f32>(case.variable, pool)
                .unwrap()
                .iter(),
        ),
        NumericKind::F64 => checksum_f64(
            file.read_variable_in_pool::<f64>(case.variable, pool)
                .unwrap()
                .iter(),
        ),
    }
}

fn full_read_checksum_netcdf_rust(path: &Path, case: &BenchCase) -> u64 {
    let file = NcFile::open(path).unwrap();
    full_read_checksum_netcdf_rust_file(&file, case)
}

fn full_read_checksum_georust_file(file: &netcdf::File, case: &BenchCase) -> u64 {
    let variable = file.variable(case.variable).unwrap();
    match case.kind {
        NumericKind::F32 => checksum_f32(variable.get_values::<f32, _>(..).unwrap().iter()),
        NumericKind::F64 => checksum_f64(variable.get_values::<f64, _>(..).unwrap().iter()),
    }
}

fn full_read_checksum_georust(path: &Path, case: &BenchCase) -> u64 {
    let file = netcdf::open(path).unwrap();
    full_read_checksum_georust_file(&file, case)
}

fn variable_hdf5_path(variable: &str) -> String {
    format!("/{}", variable.trim_start_matches('/'))
}

fn slice_end(start: usize, count: usize, stride: usize) -> usize {
    if count == 0 {
        start
    } else {
        start + (count - 1) * stride + 1
    }
}

fn slice_selection(slice: SliceSpec) -> SliceInfo {
    SliceInfo {
        selections: slice
            .start
            .iter()
            .zip(slice.count.iter())
            .zip(slice.stride.iter())
            .map(|((start, count), stride)| SliceInfoElem::Slice {
                start: *start as u64,
                end: slice_end(*start, *count, *stride) as u64,
                step: *stride as u64,
            })
            .collect(),
    }
}

fn nc_slice_selection(slice: SliceSpec) -> NcSliceInfo {
    NcSliceInfo {
        selections: slice
            .start
            .iter()
            .zip(slice.count.iter())
            .zip(slice.stride.iter())
            .map(|((start, count), stride)| NcSliceInfoElem::Slice {
                start: *start as u64,
                end: slice_end(*start, *count, *stride) as u64,
                step: *stride as u64,
            })
            .collect(),
    }
}

fn slice_checksum_netcdf_rust(path: &Path, case: &BenchCase, slice: SliceSpec) -> u64 {
    let file = Hdf5File::open(path).unwrap();
    let dataset = file.dataset(&variable_hdf5_path(case.variable)).unwrap();
    let selection = slice_selection(slice);
    match case.kind {
        NumericKind::F32 => checksum_f32(dataset.read_slice::<f32>(&selection).unwrap().iter()),
        NumericKind::F64 => checksum_f64(dataset.read_slice::<f64>(&selection).unwrap().iter()),
    }
}

fn slice_checksum_netcdf_rust_file(file: &NcFile, case: &BenchCase, slice: SliceSpec) -> u64 {
    let selection = nc_slice_selection(slice);
    match case.kind {
        NumericKind::F32 => checksum_f32(
            file.read_variable_slice::<f32>(case.variable, &selection)
                .unwrap()
                .iter(),
        ),
        NumericKind::F64 => checksum_f64(
            file.read_variable_slice::<f64>(case.variable, &selection)
                .unwrap()
                .iter(),
        ),
    }
}

fn slice_checksum_netcdf_rust_file_parallel_in_pool(
    file: &NcFile,
    case: &BenchCase,
    slice: SliceSpec,
    pool: &rayon::ThreadPool,
) -> u64 {
    let selection = nc_slice_selection(slice);
    pool.install(|| match case.kind {
        NumericKind::F32 => checksum_f32(
            file.read_variable_slice_parallel::<f32>(case.variable, &selection)
                .unwrap()
                .iter(),
        ),
        NumericKind::F64 => checksum_f64(
            file.read_variable_slice_parallel::<f64>(case.variable, &selection)
                .unwrap()
                .iter(),
        ),
    })
}

fn slice_checksum_netcdf_rust_dataset(
    dataset: &hdf5_reader::Dataset,
    case: &BenchCase,
    slice: SliceSpec,
) -> u64 {
    let selection = slice_selection(slice);
    match case.kind {
        NumericKind::F32 => checksum_f32(dataset.read_slice::<f32>(&selection).unwrap().iter()),
        NumericKind::F64 => checksum_f64(dataset.read_slice::<f64>(&selection).unwrap().iter()),
    }
}

fn full_read_checksum_netcdf_rust_dataset(dataset: &hdf5_reader::Dataset, case: &BenchCase) -> u64 {
    match case.kind {
        NumericKind::F32 => checksum_f32(dataset.read_array::<f32>().unwrap().iter()),
        NumericKind::F64 => checksum_f64(dataset.read_array::<f64>().unwrap().iter()),
    }
}

fn full_read_checksum_netcdf_rust_dataset_in_pool(
    dataset: &hdf5_reader::Dataset,
    case: &BenchCase,
    pool: &rayon::ThreadPool,
) -> u64 {
    match case.kind {
        NumericKind::F32 => checksum_f32(dataset.read_array_in_pool::<f32>(pool).unwrap().iter()),
        NumericKind::F64 => checksum_f64(dataset.read_array_in_pool::<f64>(pool).unwrap().iter()),
    }
}

fn slice_checksum_georust(path: &Path, case: &BenchCase, slice: SliceSpec) -> u64 {
    let file = netcdf::open(path).unwrap();
    slice_checksum_georust_file(&file, case, slice)
}

fn slice_checksum_georust_file(file: &netcdf::File, case: &BenchCase, slice: SliceSpec) -> u64 {
    let variable = file.variable(case.variable).unwrap();
    let strides: Vec<isize> = slice.stride.iter().map(|&stride| stride as isize).collect();
    match case.kind {
        NumericKind::F32 => checksum_f32(
            variable
                .get_values::<f32, _>((slice.start, slice.count, strides.as_slice()))
                .unwrap()
                .iter(),
        ),
        NumericKind::F64 => checksum_f64(
            variable
                .get_values::<f64, _>((slice.start, slice.count, strides.as_slice()))
                .unwrap()
                .iter(),
        ),
    }
}

fn walk_netcdf_rust_group(group: &NcGroup) -> usize {
    let mut total = group.name.len();
    total += group.dimensions.len();
    total += group.attributes.len();

    for dimension in &group.dimensions {
        total += dimension.name.len();
        total += dimension.size as usize;
        total += usize::from(dimension.is_unlimited);
    }

    for variable in &group.variables {
        total += variable.name().len();
        total += variable.ndim();
        total += usize::try_from(variable.num_elements().unwrap()).unwrap();
        total += variable.attributes().len();
        for dimension in variable.dimensions() {
            total += dimension.name.len();
            total += dimension.size as usize;
        }
    }

    for attribute in &group.attributes {
        total += attribute.name.len();
    }

    for child in &group.groups {
        total += walk_netcdf_rust_group(child);
    }

    total
}

fn metadata_netcdf_rust_file(file: &NcFile) -> usize {
    walk_netcdf_rust_group(file.root_group().unwrap())
}

fn metadata_netcdf_rust(path: &Path) -> usize {
    let file = NcFile::open(path).unwrap();
    metadata_netcdf_rust_file(&file)
}

fn walk_georust_group(group: &netcdf::Group<'_>) -> usize {
    let mut total = group.name().len();

    for dimension in group.dimensions() {
        total += dimension.name().len();
        total += dimension.len();
        total += usize::from(dimension.is_unlimited());
    }

    for attribute in group.attributes() {
        total += attribute.name().len();
    }

    for variable in group.variables() {
        total += variable.name().len();
        total += variable.dimensions().len();
        total += variable.len();
        total += variable.attributes().count();
        for dimension in variable.dimensions() {
            total += dimension.name().len();
            total += dimension.len();
        }
    }

    for child in group.groups() {
        total += walk_georust_group(&child);
    }

    total
}

fn metadata_georust_file(file: &netcdf::File) -> usize {
    let mut total = 0usize;

    for dimension in file.dimensions() {
        total += dimension.name().len();
        total += dimension.len();
        total += usize::from(dimension.is_unlimited());
    }

    for attribute in file.attributes() {
        total += attribute.name().len();
    }

    for variable in file.variables() {
        total += variable.name().len();
        total += variable.dimensions().len();
        total += variable.len();
        total += variable.attributes().count();
        for dimension in variable.dimensions() {
            total += dimension.name().len();
            total += dimension.len();
        }
    }

    if let Ok(groups) = file.groups() {
        for group in groups {
            total += walk_georust_group(&group);
        }
    }

    total
}

fn metadata_georust(path: &Path) -> usize {
    let file = netcdf::open(path).unwrap();
    metadata_georust_file(&file)
}

fn open_only_netcdf_rust(path: &Path) {
    black_box(NcFile::open(path).unwrap());
}

fn open_only_georust(path: &Path) {
    black_box(netcdf::open(path).unwrap());
}

fn open_and_read_netcdf_rust(path: &Path, case: &BenchCase) -> u64 {
    full_read_checksum_netcdf_rust(path, case)
}

fn open_and_read_georust(path: &Path, case: &BenchCase) -> u64 {
    full_read_checksum_georust(path, case)
}

fn parallel_open_and_read_netcdf_rust(path: &Path, case: &BenchCase, threads: usize) -> u64 {
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        for _ in 0..threads {
            handles.push(scope.spawn(|| open_and_read_netcdf_rust(path, case)));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .fold(0u64, |acc, value| acc ^ value)
    })
}

fn parallel_open_and_read_georust(path: &Path, case: &BenchCase, threads: usize) -> u64 {
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        for _ in 0..threads {
            handles.push(scope.spawn(|| open_and_read_georust(path, case)));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .fold(0u64, |acc, value| acc ^ value)
    })
}

fn parallel_read_shared_netcdf_rust(file: &NcFile, case: &BenchCase, threads: usize) -> u64 {
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        for _ in 0..threads {
            handles.push(scope.spawn(|| full_read_checksum_netcdf_rust_file(file, case)));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .fold(0u64, |acc, value| acc ^ value)
    })
}

fn metadata_batch_netcdf_rust_file(file: &NcFile, iterations: usize) -> usize {
    let mut total = 0usize;
    for _ in 0..iterations {
        total ^= metadata_netcdf_rust_file(file);
    }
    total
}

fn metadata_batch_georust_file(file: &netcdf::File, iterations: usize) -> usize {
    let mut total = 0usize;
    for _ in 0..iterations {
        total ^= metadata_georust_file(file);
    }
    total
}

fn parallel_metadata_batch_netcdf_rust(path: &Path, threads: usize, iterations: usize) -> usize {
    let barrier = Arc::new(Barrier::new(threads));
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        for _ in 0..threads {
            let barrier = barrier.clone();
            handles.push(scope.spawn(move || {
                let file = NcFile::open(path).unwrap();
                barrier.wait();
                metadata_batch_netcdf_rust_file(&file, iterations)
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .fold(0usize, |acc, value| acc ^ value)
    })
}

fn parallel_metadata_batch_georust(path: &Path, threads: usize, iterations: usize) -> usize {
    let barrier = Arc::new(Barrier::new(threads));
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        for _ in 0..threads {
            let barrier = barrier.clone();
            handles.push(scope.spawn(move || {
                let file = netcdf::open(path).unwrap();
                barrier.wait();
                metadata_batch_georust_file(&file, iterations)
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .fold(0usize, |acc, value| acc ^ value)
    })
}

fn slice_batch_netcdf_rust_dataset(
    dataset: &hdf5_reader::Dataset,
    case: &BenchCase,
    slice: SliceSpec,
    iterations: usize,
) -> u64 {
    let mut total = 0u64;
    for _ in 0..iterations {
        total ^= slice_checksum_netcdf_rust_dataset(dataset, case, slice);
    }
    total
}

fn slice_batch_georust_file(
    file: &netcdf::File,
    case: &BenchCase,
    slice: SliceSpec,
    iterations: usize,
) -> u64 {
    let mut total = 0u64;
    for _ in 0..iterations {
        total ^= slice_checksum_georust_file(file, case, slice);
    }
    total
}

fn parallel_slice_batch_netcdf_rust(
    path: &Path,
    case: &BenchCase,
    slice: SliceSpec,
    threads: usize,
    iterations: usize,
) -> u64 {
    let barrier = Arc::new(Barrier::new(threads));
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        for _ in 0..threads {
            let barrier = barrier.clone();
            handles.push(scope.spawn(move || {
                let file = Hdf5File::open(path).unwrap();
                let dataset = file.dataset(&variable_hdf5_path(case.variable)).unwrap();
                barrier.wait();
                slice_batch_netcdf_rust_dataset(&dataset, case, slice, iterations)
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .fold(0u64, |acc, value| acc ^ value)
    })
}

fn parallel_slice_batch_georust(
    path: &Path,
    case: &BenchCase,
    slice: SliceSpec,
    threads: usize,
    iterations: usize,
) -> u64 {
    let barrier = Arc::new(Barrier::new(threads));
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        for _ in 0..threads {
            let barrier = barrier.clone();
            handles.push(scope.spawn(move || {
                let file = netcdf::open(path).unwrap();
                barrier.wait();
                slice_batch_georust_file(&file, case, slice, iterations)
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .fold(0u64, |acc, value| acc ^ value)
    })
}

fn selected_case_filters() -> Vec<String> {
    let mut filters = Vec::new();
    let mut positional_mode = false;

    for arg in std::env::args().skip(1) {
        if positional_mode {
            filters.push(arg);
            continue;
        }

        if arg == "--" {
            positional_mode = true;
            continue;
        }

        if !arg.starts_with('-') {
            filters.push(arg);
        }
    }

    filters
}

fn benchmark_group_selected(group_name: &str) -> bool {
    let filters = selected_case_filters();
    if filters.is_empty() {
        return true;
    }

    const GROUP_NAMES: &[&str] = &[
        "open_only",
        "metadata_reuse_handle",
        "read_full_reuse_handle",
        "open_and_read_full",
        "slice_reuse_handle_hdf5_backend",
        "slice_reuse_handle_classic",
        "parallel_open_and_read",
        "parallel_read_shared_netcdf_rust",
        "parallel_metadata_batch",
        "sparse_huge_logical_slice",
        "parallel_slice_batch",
        "read_full_internal_parallel",
        "read_full_internal_parallel_classic",
        "read_full_internal_parallel_nocache",
        "slice_internal_parallel_classic",
        "cf_conventions_overhead",
        "slice_selectivity",
        "memory_profile",
    ];

    let has_group_filter = GROUP_NAMES
        .iter()
        .any(|candidate| filters.iter().any(|filter| filter.contains(candidate)));

    !has_group_filter || filters.iter().any(|filter| filter.contains(group_name))
}

fn validate_cases() {
    VALIDATION_ONCE.get_or_init(|| {
        let case_filters = selected_case_filters();
        let has_specific_case_filter = CASES
            .iter()
            .any(|case| case_filters.iter().any(|filter| filter.contains(case.id)));

        for case in CASES.iter().filter(|case| {
            !has_specific_case_filter || case_filters.iter().any(|filter| filter.contains(case.id))
        }) {
            let path = case_path(case);
            let netcdf_rust_full = full_read_checksum_netcdf_rust(&path, case);
            let georust_full = full_read_checksum_georust(&path, case);
            assert_eq!(
                netcdf_rust_full, georust_full,
                "full-read checksum mismatch for {}",
                case.id
            );

            let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
            let netcdf_rust_parallel = full_read_checksum_netcdf_rust_file_in_pool(
                &NcFile::open(&path).unwrap(),
                case,
                &pool,
            );
            assert_eq!(
                netcdf_rust_full, netcdf_rust_parallel,
                "parallel full-read checksum mismatch for {}",
                case.id
            );

            let netcdf_rust_metadata = metadata_netcdf_rust(&path);
            let georust_metadata = metadata_georust(&path);
            assert!(
                netcdf_rust_metadata > 0 && georust_metadata > 0,
                "metadata walk returned zero for {}",
                case.id
            );

            if let Some(slice) = case.slice {
                let netcdf_rust_slice =
                    slice_checksum_netcdf_rust_file(&NcFile::open(&path).unwrap(), case, slice);
                let georust_slice = slice_checksum_georust(&path, case, slice);
                assert_eq!(
                    netcdf_rust_slice, georust_slice,
                    "slice checksum mismatch for {}",
                    case.id
                );

                if case.is_netcdf4 {
                    let hdf5_slice = slice_checksum_netcdf_rust(&path, case, slice);
                    assert_eq!(
                        netcdf_rust_slice, hdf5_slice,
                        "HDF5 backend slice checksum mismatch for {}",
                        case.id
                    );
                }
            }
        }
    });
}

fn bench_open_only(c: &mut Criterion) {
    if !benchmark_group_selected("open_only") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("open_only");

    for case in CASES {
        let path = case_path(case);

        group.bench_with_input(
            BenchmarkId::new("netcdf_rust", case.id),
            &path,
            |b, path| {
                b.iter(|| open_only_netcdf_rust(path));
            },
        );

        group.bench_with_input(BenchmarkId::new("georust", case.id), &path, |b, path| {
            b.iter(|| open_only_georust(path));
        });
    }

    group.finish();
}

fn bench_metadata_reuse_handle(c: &mut Criterion) {
    if !benchmark_group_selected("metadata_reuse_handle") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("metadata_reuse_handle");

    for case in CASES {
        let path = case_path(case);
        let netcdf_rust = NcFile::open(&path).unwrap();
        let georust = netcdf::open(&path).unwrap();

        group.bench_function(BenchmarkId::new("netcdf_rust", case.id), |b| {
            b.iter(|| black_box(metadata_netcdf_rust_file(&netcdf_rust)));
        });

        group.bench_function(BenchmarkId::new("georust", case.id), |b| {
            b.iter(|| black_box(metadata_georust_file(&georust)));
        });
    }

    group.finish();
}

fn bench_read_full_reuse_handle(c: &mut Criterion) {
    if !benchmark_group_selected("read_full_reuse_handle") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("read_full_reuse_handle");

    for case in CASES {
        let path = case_path(case);
        let netcdf_rust = NcFile::open(&path).unwrap();
        let georust = netcdf::open(&path).unwrap();
        group.throughput(Throughput::Bytes(case_bytes(case) as u64));

        group.bench_function(BenchmarkId::new("netcdf_rust", case.id), |b| {
            b.iter(|| black_box(full_read_checksum_netcdf_rust_file(&netcdf_rust, case)));
        });

        group.bench_function(BenchmarkId::new("georust", case.id), |b| {
            b.iter(|| black_box(full_read_checksum_georust_file(&georust, case)));
        });
    }

    group.finish();
}

fn bench_open_and_read_full(c: &mut Criterion) {
    if !benchmark_group_selected("open_and_read_full") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("open_and_read_full");

    for case in CASES {
        let path = case_path(case);
        group.throughput(Throughput::Bytes(case_bytes(case) as u64));

        group.bench_with_input(
            BenchmarkId::new("netcdf_rust", case.id),
            &(path.clone(), case),
            |b, input| {
                b.iter(|| black_box(open_and_read_netcdf_rust(&input.0, input.1)));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("georust", case.id),
            &(path.clone(), case),
            |b, input| {
                b.iter(|| black_box(open_and_read_georust(&input.0, input.1)));
            },
        );
    }

    group.finish();
}

fn bench_slice_reuse_handle(c: &mut Criterion) {
    if !benchmark_group_selected("slice_reuse_handle_hdf5_backend") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("slice_reuse_handle_hdf5_backend");

    for case in CASES
        .iter()
        .filter(|case| case.is_netcdf4 && case.slice.is_some())
    {
        let path = case_path(case);
        let slice = case.slice.unwrap();
        let netcdf_rust_file = Hdf5File::open(&path).unwrap();
        let netcdf_rust_dataset = netcdf_rust_file
            .dataset(&variable_hdf5_path(case.variable))
            .unwrap();
        let georust = netcdf::open(&path).unwrap();
        group.throughput(Throughput::Bytes(slice_bytes(case).unwrap() as u64));

        group.bench_function(BenchmarkId::new("netcdf_rust", case.id), |b| {
            b.iter(|| {
                black_box(slice_checksum_netcdf_rust_dataset(
                    &netcdf_rust_dataset,
                    case,
                    slice,
                ))
            });
        });

        group.bench_function(BenchmarkId::new("georust", case.id), |b| {
            b.iter(|| black_box(slice_checksum_georust_file(&georust, case, slice)));
        });
    }

    group.finish();
}

fn bench_classic_slice_reuse_handle(c: &mut Criterion) {
    if !benchmark_group_selected("slice_reuse_handle_classic") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("slice_reuse_handle_classic");

    for case in CASES
        .iter()
        .filter(|case| !case.is_netcdf4 && case.slice.is_some())
    {
        let path = case_path(case);
        let slice = case.slice.unwrap();
        let netcdf_rust = NcFile::open(&path).unwrap();
        let georust = netcdf::open(&path).unwrap();
        group.throughput(Throughput::Bytes(slice_bytes(case).unwrap() as u64));

        group.bench_function(BenchmarkId::new("netcdf_rust", case.id), |b| {
            b.iter(|| black_box(slice_checksum_netcdf_rust_file(&netcdf_rust, case, slice)));
        });

        group.bench_function(BenchmarkId::new("georust", case.id), |b| {
            b.iter(|| black_box(slice_checksum_georust_file(&georust, case, slice)));
        });
    }

    group.finish();
}

fn bench_parallel_open_and_read(c: &mut Criterion) {
    if !benchmark_group_selected("parallel_open_and_read") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("parallel_open_and_read");

    for threads in thread_counts() {
        for case in CASES.iter().filter(|case| case.is_netcdf4) {
            let path = case_path(case);
            group.throughput(Throughput::Bytes((case_bytes(case) * threads) as u64));

            group.bench_with_input(
                BenchmarkId::new(format!("netcdf_rust_x{threads}"), case.id),
                &(path.clone(), case, threads),
                |b, input| {
                    b.iter(|| {
                        black_box(parallel_open_and_read_netcdf_rust(
                            &input.0, input.1, input.2,
                        ))
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new(format!("georust_x{threads}"), case.id),
                &(path.clone(), case, threads),
                |b, input| {
                    b.iter(|| {
                        black_box(parallel_open_and_read_georust(&input.0, input.1, input.2))
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_parallel_read_shared_netcdf_rust(c: &mut Criterion) {
    if !benchmark_group_selected("parallel_read_shared_netcdf_rust") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("parallel_read_shared_netcdf_rust");

    for threads in thread_counts().into_iter().filter(|threads| *threads > 1) {
        for case in CASES.iter().filter(|case| case.is_netcdf4) {
            let path = case_path(case);
            let netcdf_rust = NcFile::open(&path).unwrap();
            group.throughput(Throughput::Bytes((case_bytes(case) * threads) as u64));

            group.bench_function(
                BenchmarkId::new(format!("netcdf_rust_x{threads}"), case.id),
                |b| {
                    b.iter(|| {
                        black_box(parallel_read_shared_netcdf_rust(
                            &netcdf_rust,
                            case,
                            threads,
                        ))
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_parallel_metadata_batch(c: &mut Criterion) {
    if !benchmark_group_selected("parallel_metadata_batch") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("parallel_metadata_batch");
    let iterations = hot_ops_per_thread();

    for threads in thread_counts().into_iter().filter(|threads| *threads > 0) {
        for case in CASES
            .iter()
            .filter(|case| matches!(case.id, "nc4_basic" | "nc4_groups" | "nested_nc4_groups"))
        {
            let path = case_path(case);
            group.throughput(Throughput::Elements((iterations * threads) as u64));

            group.bench_with_input(
                BenchmarkId::new(format!("netcdf_rust_x{threads}"), case.id),
                &(path.clone(), threads),
                |b, input| {
                    b.iter(|| {
                        black_box(parallel_metadata_batch_netcdf_rust(
                            &input.0, input.1, iterations,
                        ))
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new(format!("georust_x{threads}"), case.id),
                &(path.clone(), threads),
                |b, input| {
                    b.iter(|| {
                        black_box(parallel_metadata_batch_georust(
                            &input.0, input.1, iterations,
                        ))
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_sparse_huge_logical_slice(c: &mut Criterion) {
    if !benchmark_group_selected("sparse_huge_logical_slice") {
        return;
    }
    let mut group = c.benchmark_group("sparse_huge_logical_slice");
    let path = generated_fixtures().sparse_huge_logical_nc4.clone();
    let netcdf_rust = NcFile::open(&path).unwrap();
    let georust = netcdf::open(&path).unwrap();
    let selection = netcdf_reader::NcSliceInfo {
        selections: vec![
            netcdf_reader::NcSliceInfoElem::Index(((1 << 20) - 1) as u64),
            netcdf_reader::NcSliceInfoElem::Index(((1 << 20) - 1) as u64),
        ],
    };
    let georust_start = &[(1 << 20) - 1, (1 << 20) - 1];
    let georust_count = &[1usize, 1usize];

    let rust_value = checksum_f32(
        netcdf_rust
            .read_variable_slice::<f32>("sparse", &selection)
            .unwrap()
            .iter(),
    );
    let georust_value = {
        let variable = georust.variable("sparse").unwrap();
        checksum_f32(
            variable
                .get_values::<f32, _>((georust_start, georust_count))
                .unwrap()
                .iter(),
        )
    };
    assert_eq!(rust_value, georust_value);

    group.throughput(Throughput::Bytes(4));

    group.bench_function("netcdf_rust", |b| {
        b.iter(|| {
            black_box(
                netcdf_rust
                    .read_variable_slice::<f32>("sparse", &selection)
                    .unwrap(),
            )
        });
    });

    group.bench_function("georust", |b| {
        b.iter(|| {
            let variable = georust.variable("sparse").unwrap();
            black_box(
                variable
                    .get::<f32, _>((georust_start, georust_count))
                    .unwrap(),
            )
        });
    });

    group.finish();
}

fn bench_parallel_slice_batch(c: &mut Criterion) {
    if !benchmark_group_selected("parallel_slice_batch") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("parallel_slice_batch");
    let iterations = hot_ops_per_thread();

    for threads in thread_counts().into_iter().filter(|threads| *threads > 0) {
        for case in CASES.iter().filter(|case| {
            matches!(
                case.id,
                "nc4_basic" | "nc4_compressed" | "large_nc4_compressed"
            )
        }) {
            let path = case_path(case);
            let slice = hot_slice_for_case(case).unwrap();
            group.throughput(Throughput::Elements((iterations * threads) as u64));

            group.bench_with_input(
                BenchmarkId::new(format!("netcdf_rust_x{threads}"), case.id),
                &(path.clone(), threads),
                |b, input| {
                    b.iter(|| {
                        black_box(parallel_slice_batch_netcdf_rust(
                            &input.0, case, slice, input.1, iterations,
                        ))
                    });
                },
            );

            group.bench_with_input(
                BenchmarkId::new(format!("georust_x{threads}"), case.id),
                &(path.clone(), threads),
                |b, input| {
                    b.iter(|| {
                        black_box(parallel_slice_batch_georust(
                            &input.0, case, slice, input.1, iterations,
                        ))
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_read_full_internal_parallel(c: &mut Criterion) {
    if !benchmark_group_selected("read_full_internal_parallel") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("read_full_internal_parallel");

    for case in CASES
        .iter()
        .filter(|case| matches!(case.id, "nc4_compressed" | "large_nc4_compressed"))
    {
        let path = case_path(case);
        let netcdf_rust_file = Hdf5File::open(&path).unwrap();
        let dataset = netcdf_rust_file
            .dataset(&variable_hdf5_path(case.variable))
            .unwrap();
        let georust = netcdf::open(&path).unwrap();
        group.throughput(Throughput::Bytes(case_bytes(case) as u64));

        group.bench_function(BenchmarkId::new("netcdf_rust_x1", case.id), |b| {
            b.iter(|| black_box(full_read_checksum_netcdf_rust_dataset(&dataset, case)));
        });

        group.bench_function(BenchmarkId::new("georust_x1", case.id), |b| {
            b.iter(|| black_box(full_read_checksum_georust_file(&georust, case)));
        });

        for threads in thread_counts().into_iter().filter(|threads| *threads > 1) {
            let pool = ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            group.bench_function(
                BenchmarkId::new(format!("netcdf_rust_x{threads}"), case.id),
                |b| {
                    b.iter(|| {
                        black_box(full_read_checksum_netcdf_rust_dataset_in_pool(
                            &dataset, case, &pool,
                        ))
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_read_full_internal_parallel_classic(c: &mut Criterion) {
    if !benchmark_group_selected("read_full_internal_parallel_classic") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("read_full_internal_parallel_classic");

    for case in CASES
        .iter()
        .filter(|case| matches!(case.id, "large_cdf5" | "large_record_cdf5"))
    {
        let path = case_path(case);
        let netcdf_rust = NcFile::open(&path).unwrap();
        let georust = netcdf::open(&path).unwrap();
        group.throughput(Throughput::Bytes(case_bytes(case) as u64));

        group.bench_function(BenchmarkId::new("netcdf_rust_x1", case.id), |b| {
            b.iter(|| black_box(full_read_checksum_netcdf_rust_file(&netcdf_rust, case)));
        });

        group.bench_function(BenchmarkId::new("georust_x1", case.id), |b| {
            b.iter(|| black_box(full_read_checksum_georust_file(&georust, case)));
        });

        for threads in thread_counts().into_iter().filter(|threads| *threads > 1) {
            let pool = ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            group.bench_function(
                BenchmarkId::new(format!("netcdf_rust_x{threads}"), case.id),
                |b| {
                    b.iter(|| {
                        black_box(full_read_checksum_netcdf_rust_file_in_pool(
                            &netcdf_rust,
                            case,
                            &pool,
                        ))
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_slice_internal_parallel_classic(c: &mut Criterion) {
    if !benchmark_group_selected("slice_internal_parallel_classic") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("slice_internal_parallel_classic");

    for case in CASES
        .iter()
        .filter(|case| !case.is_netcdf4 && case.slice.is_some())
    {
        let path = case_path(case);
        let slice = case.slice.unwrap();
        let netcdf_rust = NcFile::open(&path).unwrap();
        let georust = netcdf::open(&path).unwrap();
        group.throughput(Throughput::Bytes(slice_bytes(case).unwrap() as u64));

        group.bench_function(BenchmarkId::new("netcdf_rust_x1", case.id), |b| {
            b.iter(|| black_box(slice_checksum_netcdf_rust_file(&netcdf_rust, case, slice)));
        });

        group.bench_function(BenchmarkId::new("georust_x1", case.id), |b| {
            b.iter(|| black_box(slice_checksum_georust_file(&georust, case, slice)));
        });

        for threads in thread_counts().into_iter().filter(|threads| *threads > 1) {
            let pool = ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            group.bench_function(
                BenchmarkId::new(format!("netcdf_rust_x{threads}"), case.id),
                |b| {
                    b.iter(|| {
                        black_box(slice_checksum_netcdf_rust_file_parallel_in_pool(
                            &netcdf_rust,
                            case,
                            slice,
                            &pool,
                        ))
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_read_full_internal_parallel_nocache(c: &mut Criterion) {
    if !benchmark_group_selected("read_full_internal_parallel_nocache") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("read_full_internal_parallel_nocache");

    for case in CASES
        .iter()
        .filter(|case| matches!(case.id, "nc4_compressed" | "large_nc4_compressed"))
    {
        let path = case_path(case);
        let netcdf_rust_file = Hdf5File::open_with_options(
            &path,
            OpenOptions {
                chunk_cache_bytes: 0,
                chunk_cache_slots: 1,
                filter_registry: None,
                ..Default::default()
            },
        )
        .unwrap();
        let dataset = netcdf_rust_file
            .dataset(&variable_hdf5_path(case.variable))
            .unwrap();
        group.throughput(Throughput::Bytes(case_bytes(case) as u64));

        group.bench_function(BenchmarkId::new("netcdf_rust_x1", case.id), |b| {
            b.iter(|| black_box(full_read_checksum_netcdf_rust_dataset(&dataset, case)));
        });

        for threads in thread_counts().into_iter().filter(|threads| *threads > 1) {
            let pool = ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            group.bench_function(
                BenchmarkId::new(format!("netcdf_rust_x{threads}"), case.id),
                |b| {
                    b.iter(|| {
                        black_box(full_read_checksum_netcdf_rust_dataset_in_pool(
                            &dataset, case, &pool,
                        ))
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_cf_conventions_overhead(c: &mut Criterion) {
    if !benchmark_group_selected("cf_conventions_overhead") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("cf_conventions_overhead");

    // Use large_nc4_compressed — it has f32 data, good for measuring promotion overhead.
    for case in CASES
        .iter()
        .filter(|c| matches!(c.id, "large_nc4_compressed"))
    {
        let path = case_path(case);
        let netcdf_rust = NcFile::open(&path).unwrap();
        group.throughput(Throughput::Bytes(case_bytes(case) as u64));

        // Baseline: typed read (f32)
        group.bench_function(BenchmarkId::new("read_variable_f32", case.id), |b| {
            b.iter(|| black_box(netcdf_rust.read_variable::<f32>(case.variable).unwrap()));
        });

        // Type-promoting read (f32 → f64)
        group.bench_function(BenchmarkId::new("read_variable_as_f64", case.id), |b| {
            b.iter(|| black_box(netcdf_rust.read_variable_as_f64(case.variable).unwrap()));
        });

        // Unpacked (includes type promotion + scale/offset — no-op if absent)
        group.bench_function(BenchmarkId::new("read_variable_unpacked", case.id), |b| {
            b.iter(|| black_box(netcdf_rust.read_variable_unpacked(case.variable).unwrap()));
        });

        // Full CF pipeline (mask + unpack)
        group.bench_function(
            BenchmarkId::new("read_variable_unpacked_masked", case.id),
            |b| {
                b.iter(|| {
                    black_box(
                        netcdf_rust
                            .read_variable_unpacked_masked(case.variable)
                            .unwrap(),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_slice_selectivity(c: &mut Criterion) {
    if !benchmark_group_selected("slice_selectivity") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("slice_selectivity");

    // Test different selectivity levels on large_nc4_compressed (2048x1024).
    for case in CASES
        .iter()
        .filter(|c| matches!(c.id, "large_nc4_compressed"))
    {
        let path = case_path(case);
        let hdf5 = Hdf5File::open(&path).unwrap();
        let dataset = hdf5.dataset(&variable_hdf5_path(case.variable)).unwrap();

        let selectivities: &[(&str, SliceSpec)] = &[
            (
                "100pct",
                SliceSpec {
                    start: &[0, 0],
                    count: &[2048, 1024],
                    stride: UNIT_STRIDE_2D,
                },
            ),
            (
                "50pct",
                SliceSpec {
                    start: &[0, 0],
                    count: &[1024, 1024],
                    stride: UNIT_STRIDE_2D,
                },
            ),
            (
                "10pct",
                SliceSpec {
                    start: &[0, 0],
                    count: &[205, 1024],
                    stride: UNIT_STRIDE_2D,
                },
            ),
            (
                "1pct",
                SliceSpec {
                    start: &[512, 256],
                    count: &[20, 1024],
                    stride: UNIT_STRIDE_2D,
                },
            ),
        ];

        for (label, slice) in selectivities {
            let sel = slice_selection(*slice);
            let elem_count: usize = slice.count.iter().product();
            group.throughput(Throughput::Bytes((elem_count * 4) as u64)); // f32

            group.bench_function(BenchmarkId::new("netcdf_rust", label), |b| {
                b.iter(|| black_box(dataset.read_slice::<f32>(&sel).unwrap()));
            });
        }
    }

    group.finish();
}

#[cfg(feature = "bench-memory-profile")]
fn bench_memory_profile(c: &mut Criterion) {
    if !benchmark_group_selected("memory_profile") {
        return;
    }
    validate_cases();
    let mut group = c.benchmark_group("memory_profile");

    for case in CASES
        .iter()
        .filter(|c| matches!(c.id, "large_nc4_compressed"))
    {
        let path = case_path(case);

        // Full read memory
        group.bench_function(BenchmarkId::new("full_read", case.id), |b| {
            b.iter(|| {
                PEAK_ALLOC.reset_peak_usage();
                let file = NcFile::open(&path).unwrap();
                let _ = black_box(match case.kind {
                    NumericKind::F32 => {
                        checksum_f32(&file.read_variable::<f32>(case.variable).unwrap())
                    }
                    NumericKind::F64 => {
                        checksum_f64(&file.read_variable::<f64>(case.variable).unwrap())
                    }
                });
                black_box(PEAK_ALLOC.peak_usage_as_mb())
            });
        });

        // Slice read memory (10% selectivity)
        if case.is_netcdf4 {
            let hdf5 = Hdf5File::open(&path).unwrap();
            let dataset = hdf5.dataset(&variable_hdf5_path(case.variable)).unwrap();
            let sel = slice_selection(SliceSpec {
                start: &[0, 0],
                count: &[205, 1024],
                stride: UNIT_STRIDE_2D,
            });

            group.bench_function(BenchmarkId::new("slice_10pct", case.id), |b| {
                b.iter(|| {
                    PEAK_ALLOC.reset_peak_usage();
                    let _ = black_box(dataset.read_slice::<f32>(&sel).unwrap());
                    black_box(PEAK_ALLOC.peak_usage_as_mb())
                });
            });
        }
    }

    group.finish();
}

#[cfg(feature = "bench-memory-profile")]
criterion_group!(
    benches,
    bench_open_only,
    bench_metadata_reuse_handle,
    bench_read_full_reuse_handle,
    bench_read_full_internal_parallel,
    bench_read_full_internal_parallel_classic,
    bench_read_full_internal_parallel_nocache,
    bench_open_and_read_full,
    bench_slice_reuse_handle,
    bench_classic_slice_reuse_handle,
    bench_parallel_open_and_read,
    bench_parallel_read_shared_netcdf_rust,
    bench_parallel_metadata_batch,
    bench_parallel_slice_batch,
    bench_slice_internal_parallel_classic,
    bench_sparse_huge_logical_slice,
    bench_cf_conventions_overhead,
    bench_slice_selectivity,
    bench_memory_profile,
);
#[cfg(not(feature = "bench-memory-profile"))]
criterion_group!(
    benches,
    bench_open_only,
    bench_metadata_reuse_handle,
    bench_read_full_reuse_handle,
    bench_read_full_internal_parallel,
    bench_read_full_internal_parallel_classic,
    bench_read_full_internal_parallel_nocache,
    bench_open_and_read_full,
    bench_slice_reuse_handle,
    bench_classic_slice_reuse_handle,
    bench_parallel_open_and_read,
    bench_parallel_read_shared_netcdf_rust,
    bench_parallel_metadata_batch,
    bench_parallel_slice_batch,
    bench_slice_internal_parallel_classic,
    bench_sparse_huge_logical_slice,
    bench_cf_conventions_overhead,
    bench_slice_selectivity,
);
criterion_main!(benches);
