use nirs4all_io::{walk_path, WalkOptions, WalkOutcome, WalkStats};

#[test]
fn walks_committed_asd_directory() {
    let entries = walk_path(workspace_file("samples/asd"), &WalkOptions::default())
        .expect("walk samples/asd");
    let stats = WalkStats::collect(&entries);
    assert!(stats.parsed >= 5, "expected at least 5 parsed ASD fixtures");
    assert_eq!(stats.errored, 0);
    for entry in &entries {
        match &entry.outcome {
            WalkOutcome::Parsed { format, records } => {
                assert_eq!(format, "asd-fieldspec");
                assert_eq!(records.len(), 1);
            }
            WalkOutcome::Error { message, .. } => {
                panic!("unexpected error: {message}");
            }
            WalkOutcome::Unsupported => {}
        }
    }
}

#[test]
fn includes_unsupported_when_requested() {
    let with_unsupported = walk_path(
        workspace_file("samples/hyperspectral_cubes"),
        &WalkOptions {
            skip_unsupported: false,
            ..WalkOptions::default()
        },
    )
    .expect("walk hyperspectral_cubes");
    let without = walk_path(
        workspace_file("samples/hyperspectral_cubes"),
        &WalkOptions::default(),
    )
    .expect("walk hyperspectral_cubes");
    assert!(
        with_unsupported.len() > without.len(),
        "include_unsupported should surface extra entries"
    );
    assert!(with_unsupported
        .iter()
        .any(|entry| matches!(entry.outcome, WalkOutcome::Unsupported)));
}

#[test]
fn walks_single_file() {
    let entries = walk_path(
        workspace_file("samples/csv_tsv/synthetic_nirs.csv"),
        &WalkOptions::default(),
    )
    .expect("walk single file");
    assert_eq!(entries.len(), 1);
    assert!(entries[0].outcome.is_parsed());
}

#[test]
fn refuses_designed_refusal_files_with_error_outcome() {
    let entries = walk_path(
        workspace_file("samples/foss_winisi/synthetic_ds3_report.csv"),
        &WalkOptions::default(),
    )
    .expect("walk refusal");
    assert_eq!(entries.len(), 1);
    let WalkOutcome::Error { message, .. } = &entries[0].outcome else {
        panic!("expected error outcome");
    };
    assert!(message.contains("no numeric spectral headers found"));
}

#[test]
fn max_depth_zero_limits_to_root_children() {
    let shallow = walk_path(
        workspace_file("samples"),
        &WalkOptions {
            max_depth: Some(0),
            ..WalkOptions::default()
        },
    )
    .expect("walk samples max_depth=0");
    let deep = walk_path(workspace_file("samples"), &WalkOptions::default())
        .expect("walk samples unlimited");
    assert!(deep.len() > shallow.len());
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
