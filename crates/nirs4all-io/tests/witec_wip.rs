use std::path::Path;

use nirs4all_io::{builtin_probes, open_path, AxisKind, Confidence, SignalType};

#[test]
fn probes_wit_pr06_and_legacy_wit_magic() {
    let probes = builtin_probes(b"WIT_PR06\0\0\0\0", Path::new("sample.wip"));
    assert!(probes
        .iter()
        .any(|probe| probe.format == "witec-wip" && probe.confidence == Confidence::Definite));

    let probes = builtin_probes(b"WIT^\0\0\0\0", Path::new("legacy.wip"));
    assert!(probes
        .iter()
        .any(|probe| probe.format == "witec-wip" && probe.confidence == Confidence::Definite));
}

#[test]
fn reads_sa4_wit_pr06_tdgraph() {
    let records = open_path(workspace_file("samples/raman_witec/Sa4.wip")).expect("open Sa4.wip");
    assert_eq!(records.len(), 4410);

    let first = &records[0];
    assert_eq!(first.signal_type, SignalType::RawCounts);
    assert_eq!(first.metadata.get("x_index"), Some(&serde_json::json!(0)));
    assert_eq!(first.metadata.get("y_index"), Some(&serde_json::json!(0)));
    assert_eq!(
        first.metadata.get("witec_layout"),
        Some(&serde_json::json!("WIT_PR06_TDGraph_u16_Sa4"))
    );
    assert_eq!(
        first.metadata.get("physical_grid_slots"),
        Some(&serde_json::json!(4950))
    );
    assert_eq!(
        first.metadata.get("valid_line_count"),
        Some(&serde_json::json!(49))
    );
    assert_eq!(
        first.metadata.get("invalid_line_count"),
        Some(&serde_json::json!(6))
    );
    assert_eq!(
        first.metadata.get("valid_spectrum_count"),
        Some(&serde_json::json!(4410))
    );
    assert_eq!(
        first.metadata.get("line_valid_encoding"),
        Some(&serde_json::json!("u8_boolean"))
    );
    assert_eq!(
        first.metadata.get("axis_calibration"),
        Some(&serde_json::json!("FreePolynom"))
    );
    assert_eq!(
        first.metadata.get("free_polynom_order"),
        Some(&serde_json::json!(6))
    );
    assert_eq!(
        first.metadata.get("free_polynom_start_bin"),
        Some(&serde_json::json!(0.0))
    );
    assert_eq!(
        first.metadata.get("free_polynom_stop_bin"),
        Some(&serde_json::json!(1024.0))
    );
    assert!(first
        .provenance
        .warnings
        .iter()
        .any(|warning| warning == "witec_wip_experimental_parser"));

    let signal = first.signals.get("raw_counts").expect("raw_counts");
    assert_eq!(signal.signal_type, SignalType::RawCounts);
    assert_eq!(signal.unit.as_deref(), Some("counts"));
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.values.len(), 1024);
    assert_eq!(signal.values.len(), 1024);
    assert_eq!(signal.values[0], 700.0);
    assert!((signal.axis.values[0] - 531.5776716392156).abs() < 1e-9);
    assert!((signal.axis.values[1023] - 567.6329112855832).abs() < 1e-9);

    let last = records.last().expect("last record");
    assert_eq!(last.metadata.get("x_index"), Some(&serde_json::json!(89)));
    assert_eq!(last.metadata.get("y_index"), Some(&serde_json::json!(48)));
    assert_eq!(
        last.metadata.get("physical_spectrum_index"),
        Some(&serde_json::json!(4409))
    );
}

#[test]
fn rejects_unknown_wit_pr06_layouts_explicitly() {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "nirs4all-io-witec-wip-unknown-{}.wip",
        std::process::id()
    ));
    std::fs::write(&path, b"WIT_PR06\0\0\0\0synthetic").expect("write synthetic wip");

    let err = open_path(&path).expect_err("unknown WIT_PR06 layout must be refused");
    let _ = std::fs::remove_file(&path);

    let message = err.to_string();
    assert!(message.contains("unsupported WiTec WIP layout"));
    assert!(message.contains("TDGraph"));
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
