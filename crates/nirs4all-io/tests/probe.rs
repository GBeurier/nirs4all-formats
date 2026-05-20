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

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
