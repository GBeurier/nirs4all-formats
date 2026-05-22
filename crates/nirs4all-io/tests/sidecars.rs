//! End-to-end equivalence between `open_path` and `open_with_sidecars`
//! across every committed sidecar-bearing format. Each test loads the
//! primary file (and any companion sidecars) from disk, builds an
//! `InMemorySidecars`, and verifies the in-memory decode produces records
//! that match the path-mode decode signal-for-signal.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use nirs4all_io::{open_path, open_with_sidecars, InMemorySidecars, SidecarResolver};

fn workspace_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn assert_records_match(
    relative_primary: &str,
    sidecars_relative: &[&str],
    name_override: Option<&str>,
) {
    let primary_path = workspace_file(relative_primary);
    if !primary_path.exists() {
        return;
    }
    let primary_bytes = std::fs::read(&primary_path).expect("read primary");
    let from_path = open_path(&primary_path).expect("open_path");

    let mut resolver = InMemorySidecars::new();
    let base = primary_path.parent().expect("primary parent");
    for rel in sidecars_relative {
        let abs = base.join(rel);
        let bytes = std::fs::read(&abs).expect("read sidecar");
        resolver.insert(PathBuf::from(rel), bytes);
    }
    let resolver: Arc<dyn SidecarResolver> = Arc::new(resolver);

    let logical_name = name_override
        .map(PathBuf::from)
        .or_else(|| primary_path.file_name().map(PathBuf::from))
        .expect("primary file name");
    let from_bytes =
        open_with_sidecars(&logical_name, &primary_bytes, resolver).expect("open_with_sidecars");

    assert_eq!(
        from_bytes.len(),
        from_path.len(),
        "{relative_primary}: record count mismatch (bytes={}, path={})",
        from_bytes.len(),
        from_path.len()
    );
    for (a, b) in from_bytes.iter().zip(from_path.iter()) {
        assert_eq!(
            a.signal_type, b.signal_type,
            "{relative_primary}: signal_type"
        );
        assert_eq!(
            a.signals.keys().collect::<Vec<_>>(),
            b.signals.keys().collect::<Vec<_>>(),
            "{relative_primary}: signal keys"
        );
        for (key, signal_bytes) in &a.signals {
            let signal_path = &b.signals[key];
            assert_eq!(
                signal_bytes.values, signal_path.values,
                "{relative_primary}: signal '{key}' values"
            );
            assert_eq!(
                signal_bytes.axis.values, signal_path.axis.values,
                "{relative_primary}: signal '{key}' axis"
            );
        }
        assert_eq!(a.metadata, b.metadata, "{relative_primary}: metadata");
        assert_eq!(a.targets, b.targets, "{relative_primary}: targets");
    }
}

#[test]
fn envi_standard_cube_in_memory_matches_path() {
    assert_records_match(
        "samples/envi_sli/cubescope-mini-cube.img",
        &["cubescope-mini-cube.hdr"],
        None,
    );
}

#[test]
fn envi_sli_synthetic_in_memory_matches_path() {
    assert_records_match(
        "samples/envi_sli/synthetic_lib.sli",
        &["synthetic_lib.hdr"],
        None,
    );
}

#[test]
fn envi_sli_usgs_splib06a_in_memory_matches_path() {
    assert_records_match(
        "samples/envi_sli/usgs_splib06a_aviris95_envi.sli",
        &["usgs_splib06a_aviris95_envi.hdr"],
        None,
    );
}

#[test]
fn erdas_lan_aviris_in_memory_matches_path() {
    assert_records_match(
        "samples/hyperspectral_cubes/92AV3C.lan",
        &["92AV3C.spc", "92AV3GT.GIS"],
        None,
    );
}

#[test]
fn fgi_xml_hdf5_in_memory_matches_path() {
    assert_records_match("samples/fgi/synthetic_fgi.xml", &["synthetic_fgi.h5"], None);
}

#[test]
fn matlab_v73_in_memory_matches_path() {
    // MATLAB v7.3 is HDF5-backed; no external sidecar.
    assert_records_match("samples/matlab/synthetic_nirs_v73.mat", &[], None);
}

#[test]
fn generic_hdf5_in_memory_matches_path() {
    // Reuse FGI's HDF5 payload via the generic HDF5 reader path; the
    // logical name keeps a `.h5` extension so sniff picks the generic
    // reader instead of `fgi-hdf5-xml`.
    let primary_path = workspace_file("samples/fgi/synthetic_fgi.h5");
    if !primary_path.exists() {
        return;
    }
    let primary_bytes = std::fs::read(&primary_path).expect("read primary");
    let from_path = open_path(&primary_path).expect("open_path");
    let resolver: Arc<dyn SidecarResolver> = Arc::new(InMemorySidecars::new());
    let from_bytes = open_with_sidecars("synthetic_fgi.h5", &primary_bytes, resolver)
        .expect("open_with_sidecars");
    assert_eq!(from_bytes.len(), from_path.len());
}

#[test]
fn matlab_v5_nir_shootout_in_memory_matches_path() {
    // Regression: a path-only structured MAT v5 reader still works through
    // open_with_sidecars (no actual sidecar needed).
    assert_records_match(
        "samples/matlab/eigenvector_nir_shootout_2002.mat",
        &[],
        None,
    );
}

fn fixture_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/hdf5_external")
        .join(name)
}

const EXPECTED_BANDS: usize = 8;
const EXPECTED_SAMPLES: usize = 4;

fn build_expected_spectra() -> Vec<f64> {
    (0..EXPECTED_SAMPLES * EXPECTED_BANDS)
        .map(|i| (i as f64) + 1.0)
        .collect()
}

fn assert_external_resolver_payload(primary_name: &str, sidecar_names: &[&str]) {
    let primary_path = fixture_file(primary_name);
    if !primary_path.exists() {
        panic!(
            "missing fixture {}; regenerate via tests/fixtures/hdf5_external/build.py",
            primary_path.display()
        );
    }
    let primary_bytes = std::fs::read(&primary_path).expect("read primary");
    let mut resolver = InMemorySidecars::new();
    for name in sidecar_names {
        let bytes = std::fs::read(fixture_file(name)).expect("read sidecar");
        resolver.insert(PathBuf::from(*name), bytes);
    }
    let resolver: Arc<dyn SidecarResolver> = Arc::new(resolver);
    let records =
        open_with_sidecars(primary_name, &primary_bytes, resolver).expect("open_with_sidecars");
    assert_eq!(
        records.len(),
        EXPECTED_SAMPLES,
        "{primary_name}: record count"
    );
    let mut flat = Vec::with_capacity(EXPECTED_SAMPLES * EXPECTED_BANDS);
    for record in &records {
        let signal = record
            .signals
            .values()
            .next()
            .expect("at least one signal per record");
        assert_eq!(signal.axis.values.len(), EXPECTED_BANDS);
        assert_eq!(signal.values.len(), EXPECTED_BANDS);
        flat.extend_from_slice(&signal.values);
    }
    assert_eq!(
        flat,
        build_expected_spectra(),
        "{primary_name}: external resolver payload mismatch",
    );
}

#[test]
fn hdf5_external_link_resolver_serves_linked_dataset() {
    assert_external_resolver_payload("primary_link.h5", &["linked.h5"]);
}

#[test]
fn hdf5_external_file_resolver_serves_external_dataset() {
    assert_external_resolver_payload("primary_file.h5", &["external_dataset.h5"]);
}
