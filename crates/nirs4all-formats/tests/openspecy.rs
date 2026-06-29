//! OpenSpecy `.rds` reader integration tests (network-free; uses the committed
//! synthetic fixture).
#![cfg(feature = "fmt-openspecy")]

use std::path::{Path, PathBuf};

use nirs4all_formats::{open_bytes, open_path, probe_path, AxisKind, Confidence, SignalType};

const FIXTURE: &str = "samples/openspecy/synthetic_minilib.rds";

fn workspace_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

#[test]
fn probes_rds_as_openspecy() {
    let probes = probe_path(workspace_file(FIXTURE)).expect("probe");
    let probe = probes
        .iter()
        .find(|probe| probe.format == "openspecy-rds")
        .unwrap_or_else(|| panic!("expected openspecy-rds probe, got {probes:?}"));
    assert_eq!(probe.confidence, Confidence::Likely);
}

#[test]
fn reads_canonical_object_from_path() {
    let records = open_path(workspace_file(FIXTURE)).expect("open OpenSpecy rds");
    assert_eq!(records.len(), 3, "one record per spectrum column");

    // Shared wavenumber axis (cm^-1) for every spectrum.
    for record in &records {
        assert_eq!(record.signals.len(), 1);
        let signal = record.signals.get("intensity").expect("intensity signal");
        assert_eq!(signal.axis.kind, AxisKind::Wavenumber);
        assert_eq!(signal.axis.unit, "cm-1");
        assert_eq!(signal.axis.values.len(), 6);
        assert_eq!(signal.values.len(), 6);
        assert_eq!(record.provenance.format, "openspecy-rds");
        assert_eq!(record.metadata.get("container").unwrap(), "rds_gzip");
    }

    // FTIR / Raman modality comes from the per-spectrum metadata row.
    let modalities: Vec<String> = records
        .iter()
        .map(|record| {
            record
                .metadata
                .get("modality")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string()
        })
        .collect();
    assert_eq!(modalities, vec!["ftir", "ftir", "raman"]);

    // The polymer identity is surfaced as a modelling target.
    let identities: Vec<String> = records
        .iter()
        .map(|record| {
            record
                .targets
                .get("spectrum_identity")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string()
        })
        .collect();
    assert_eq!(
        identities,
        vec![
            "Polystyrene",
            "Polyethylene",
            "Poly(ethylene terephthalate)"
        ]
    );

    // Intensity semantics are left Unknown unless the metadata names them.
    assert_eq!(records[0].signal_type, SignalType::Unknown);

    // The full metadata row is preserved under `fields`.
    let fields = records[2]
        .metadata
        .get("fields")
        .and_then(|value| value.as_object())
        .expect("metadata fields object");
    assert_eq!(fields.get("sample_name").unwrap(), "PET reference");
    assert_eq!(fields.get("col_id").unwrap(), "pet_r03");
}

#[test]
fn reads_canonical_object_from_bytes() {
    // Filesystem-free path (the one bindings / wasm consumers exercise).
    let bytes = std::fs::read(workspace_file(FIXTURE)).expect("read fixture bytes");
    let records = open_bytes("synthetic_minilib.rds", &bytes).expect("open_bytes OpenSpecy rds");
    assert_eq!(records.len(), 3);
    assert_eq!(
        records[0].metadata.get("spectrum_column").unwrap(),
        "ps_001"
    );
}
