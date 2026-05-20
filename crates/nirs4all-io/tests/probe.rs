use nirs4all_io::{probe_path, Confidence};

#[test]
fn probes_committed_jcamp_fixture() {
    let probes = probe_path(workspace_file("samples/jcamp_dx/TESTSPEC.DX")).expect("probe");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "jcamp-dx" && probe.confidence == Confidence::Definite));
}

#[test]
fn probes_committed_csv_fixture() {
    let probes = probe_path(workspace_file("samples/csv_tsv/synthetic_nirs.csv")).expect("probe");
    assert!(probes
        .iter()
        .any(|probe| probe.format == "delimited-text" && probe.confidence == Confidence::Likely));
}

#[test]
fn procspec_zip_does_not_probe_as_galactic_spc() {
    let probes = probe_path(workspace_file(
        "samples/ocean_optics/OceanOptics_Linux.ProcSpec",
    ))
    .expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "ocean-optics-procspec" && probe.confidence == Confidence::Definite
    }));
    assert!(!probes.iter().any(|probe| probe.format == "galactic-spc"));
}

#[test]
fn probes_committed_msa_fixture() {
    let probes = probe_path(workspace_file(
        "samples/msa_iso22029/ISO_22029_2022_compliance.msa",
    ))
    .expect("probe");
    assert!(probes.iter().any(|probe| {
        probe.format == "emsa-mas-msa" && probe.confidence == Confidence::Definite
    }));
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
