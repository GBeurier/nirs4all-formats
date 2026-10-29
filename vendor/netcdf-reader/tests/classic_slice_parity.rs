use std::path::Path;

use netcdf_reader::{Error, NcFile, NcSliceInfo, NcSliceInfoElem};

const CLASSIC_ROWS: usize = 32;
const CLASSIC_COLS: usize = 64;
const RECORD_ROWS: usize = 9;
const RECORD_COLS: usize = 12;

fn create_classic_slice_fixture(path: &Path) {
    let mut file = netcdf::create_with(path, netcdf::Options::_64BIT_DATA).unwrap();
    file.add_dimension("row", CLASSIC_ROWS).unwrap();
    file.add_dimension("col", CLASSIC_COLS).unwrap();
    file.add_variable::<f32>("data", &["row", "col"]).unwrap();
    file.enddef().unwrap();

    let mut variable = file.variable_mut("data").unwrap();
    for row in 0..CLASSIC_ROWS {
        let values: Vec<f32> = (0..CLASSIC_COLS)
            .map(|col| ((row * 131 + col * 17) % 997) as f32 * 0.5 + row as f32 * 0.25)
            .collect();
        variable.put_values(&values, (row, ..)).unwrap();
    }
}

fn classic_strided_inner_slice() -> NcSliceInfo {
    NcSliceInfo {
        selections: vec![
            NcSliceInfoElem::Slice {
                start: 3,
                end: CLASSIC_ROWS as u64,
                step: 3,
            },
            NcSliceInfoElem::Slice {
                start: 5,
                end: 60,
                step: 4,
            },
        ],
    }
}

fn create_record_slice_fixture(path: &Path) {
    let mut file = netcdf::create_with(path, netcdf::Options::_64BIT_DATA).unwrap();
    file.add_unlimited_dimension("time").unwrap();
    file.add_dimension("x", RECORD_COLS).unwrap();
    file.add_variable::<f32>("series", &["time", "x"]).unwrap();
    file.enddef().unwrap();

    let mut variable = file.variable_mut("series").unwrap();
    for record in 0..RECORD_ROWS {
        let values: Vec<f32> = (0..RECORD_COLS)
            .map(|col| ((record * 97 + col * 29) % 541) as f32 * 0.25 + col as f32 * 0.5)
            .collect();
        variable.put_values(&values, (record, ..)).unwrap();
    }
}

fn record_strided_slice() -> NcSliceInfo {
    NcSliceInfo {
        selections: vec![
            NcSliceInfoElem::Slice {
                start: 1,
                end: RECORD_ROWS as u64,
                step: 2,
            },
            NcSliceInfoElem::Slice {
                start: 2,
                end: 11,
                step: 3,
            },
        ],
    }
}

#[test]
fn classic_non_record_slice_matches_georust_for_strided_inner_selection() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("classic_slice_parity.nc");
    create_classic_slice_fixture(&path);

    let file = NcFile::open(&path).unwrap();
    let selection = classic_strided_inner_slice();
    let actual: ndarray::ArrayD<f32> = file.read_variable_slice("data", &selection).unwrap();

    let reference = netcdf::open(&path).unwrap();
    let expected = reference
        .variable("data")
        .unwrap()
        .get_values::<f32, _>((&[3usize, 5usize], &[10usize, 14usize], &[3isize, 4isize]))
        .unwrap();

    assert_eq!(actual.shape(), &[10, 14]);
    assert_eq!(actual.iter().copied().collect::<Vec<_>>(), expected);

    let promoted = file.read_variable_slice_as_f64("data", &selection).unwrap();
    assert_eq!(promoted.shape(), &[10, 14]);
    let promoted_expected: Vec<f64> = expected.iter().map(|&value| value as f64).collect();
    assert_eq!(
        promoted.iter().copied().collect::<Vec<_>>(),
        promoted_expected
    );
}

#[cfg(feature = "rayon")]
#[test]
fn classic_non_record_parallel_reads_match_serial() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("classic_parallel_read.nc");
    create_classic_slice_fixture(&path);

    let file = NcFile::open(&path).unwrap();
    let serial: ndarray::ArrayD<f32> = file.read_variable("data").unwrap();
    let parallel: ndarray::ArrayD<f32> = file.read_variable_parallel("data").unwrap();
    assert_eq!(parallel, serial);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();
    let in_pool: ndarray::ArrayD<f32> = file.read_variable_in_pool("data", &pool).unwrap();
    assert_eq!(in_pool, serial);
}

#[cfg(feature = "rayon")]
#[test]
fn classic_non_record_parallel_slice_matches_serial() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("classic_parallel_slice.nc");
    create_classic_slice_fixture(&path);

    let file = NcFile::open(&path).unwrap();
    let selection = classic_strided_inner_slice();
    let serial: ndarray::ArrayD<f32> = file.read_variable_slice("data", &selection).unwrap();
    let parallel: ndarray::ArrayD<f32> = file
        .read_variable_slice_parallel("data", &selection)
        .unwrap();
    assert_eq!(parallel, serial);
}

#[test]
fn classic_non_record_slice_allows_empty_results() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("classic_slice_empty.nc");
    create_classic_slice_fixture(&path);

    let file = NcFile::open(&path).unwrap();
    let selection = NcSliceInfo {
        selections: vec![
            NcSliceInfoElem::Slice {
                start: CLASSIC_ROWS as u64,
                end: u64::MAX,
                step: 1,
            },
            NcSliceInfoElem::Slice {
                start: 0,
                end: u64::MAX,
                step: 1,
            },
        ],
    };

    let actual: ndarray::ArrayD<f32> = file.read_variable_slice("data", &selection).unwrap();
    assert_eq!(actual.shape(), &[0, CLASSIC_COLS]);
    assert!(actual.is_empty());
}

#[test]
fn classic_non_record_slice_rejects_start_past_dimension_end() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("classic_slice_oob.nc");
    create_classic_slice_fixture(&path);

    let file = NcFile::open(&path).unwrap();
    let selection = NcSliceInfo {
        selections: vec![
            NcSliceInfoElem::Slice {
                start: CLASSIC_ROWS as u64 + 1,
                end: u64::MAX,
                step: 1,
            },
            NcSliceInfoElem::Slice {
                start: 0,
                end: u64::MAX,
                step: 1,
            },
        ],
    };

    let err = file
        .read_variable_slice::<f32>("data", &selection)
        .unwrap_err();
    assert!(matches!(err, Error::InvalidData(message) if message.contains("slice start")));
}

#[test]
fn classic_record_slice_matches_georust_for_strided_selection() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("record_slice_parity.nc");
    create_record_slice_fixture(&path);

    let file = NcFile::open(&path).unwrap();
    let selection = record_strided_slice();
    let actual: ndarray::ArrayD<f32> = file.read_variable_slice("series", &selection).unwrap();

    let reference = netcdf::open(&path).unwrap();
    let expected = reference
        .variable("series")
        .unwrap()
        .get_values::<f32, _>((&[1usize, 2usize], &[4usize, 3usize], &[2isize, 3isize]))
        .unwrap();

    assert_eq!(actual.shape(), &[4, 3]);
    assert_eq!(actual.iter().copied().collect::<Vec<_>>(), expected);

    let promoted = file
        .read_variable_slice_as_f64("series", &selection)
        .unwrap();
    let promoted_expected: Vec<f64> = expected.iter().map(|&value| value as f64).collect();
    assert_eq!(
        promoted.iter().copied().collect::<Vec<_>>(),
        promoted_expected
    );
}

#[cfg(feature = "rayon")]
#[test]
fn classic_record_parallel_reads_match_serial() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("record_parallel_read.nc");
    create_record_slice_fixture(&path);

    let file = NcFile::open(&path).unwrap();
    let serial: ndarray::ArrayD<f32> = file.read_variable("series").unwrap();
    let parallel: ndarray::ArrayD<f32> = file.read_variable_parallel("series").unwrap();
    assert_eq!(parallel, serial);
}

#[cfg(feature = "rayon")]
#[test]
fn classic_record_parallel_slice_matches_serial() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("record_parallel_slice.nc");
    create_record_slice_fixture(&path);

    let file = NcFile::open(&path).unwrap();
    let selection = record_strided_slice();
    let serial: ndarray::ArrayD<f32> = file.read_variable_slice("series", &selection).unwrap();
    let parallel: ndarray::ArrayD<f32> = file
        .read_variable_slice_parallel("series", &selection)
        .unwrap();
    assert_eq!(parallel, serial);
}

#[test]
fn classic_record_slice_index_collapses_record_axis() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("record_slice_index.nc");
    create_record_slice_fixture(&path);

    let file = NcFile::open(&path).unwrap();
    let selection = NcSliceInfo {
        selections: vec![
            NcSliceInfoElem::Index(4),
            NcSliceInfoElem::Slice {
                start: 1,
                end: 10,
                step: 2,
            },
        ],
    };

    let actual: ndarray::ArrayD<f32> = file.read_variable_slice("series", &selection).unwrap();
    let reference = netcdf::open(&path).unwrap();
    let expected = reference
        .variable("series")
        .unwrap()
        .get_values::<f32, _>((&[4usize, 1usize], &[1usize, 5usize], &[1isize, 2isize]))
        .unwrap();

    assert_eq!(actual.shape(), &[5]);
    assert_eq!(actual.iter().copied().collect::<Vec<_>>(), expected);
}
