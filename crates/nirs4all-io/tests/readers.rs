use nirs4all_io::{open_path, AxisKind, AxisOrder, SignalType};

#[test]
fn reads_synthetic_delimited_nirs_table() {
    let records =
        open_path(workspace_file("samples/csv_tsv/synthetic_nirs.csv")).expect("open csv");

    assert_eq!(records.len(), 50);
    let first = &records[0];
    let signal = first.signals.get("signal").expect("signal");
    assert_eq!(signal.axis.values.len(), 200);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(first.metadata["sample_id"].as_str(), Some("S000"));
    assert!(first.targets.contains_key("protein"));
}

#[test]
fn reads_bruker_dpt_export() {
    let records = open_path(workspace_file("samples/bruker_dpt/synthetic.dpt")).expect("open dpt");

    assert_eq!(records.len(), 1);
    let signal = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(signal.axis.values.len(), 200);
    assert_eq!(signal.axis.unit, "cm-1");
    assert_eq!(signal.axis.kind, AxisKind::Wavenumber);
    assert_eq!(signal.axis.order, AxisOrder::Descending);
    assert_eq!(signal.signal_type, SignalType::Absorbance);
}

#[test]
fn reads_avantes_wave_table() {
    let records = open_path(workspace_file("samples/avantes/avantes_export.ttt"))
        .expect("open avantes table");

    assert_eq!(records.len(), 1);
    let signal = records[0]
        .signals
        .get("transmittance")
        .expect("transmittance");
    assert!(signal.axis.values.len() >= 300);
    assert_eq!(signal.signal_type, SignalType::Transmittance);
}

#[test]
fn reads_avantes_irradiance_export() {
    let records =
        open_path(workspace_file("samples/avantes/irr_820_1941.IRR")).expect("open avantes irr");

    assert_eq!(records.len(), 1);
    let signal = records[0].signals.get("irradiance").expect("irradiance");
    assert!(signal.axis.values.len() > 1_000);
    assert_eq!(signal.signal_type, SignalType::Irradiance);
}

#[test]
fn reads_spectral_evolution_sed() {
    let records = open_path(workspace_file(
        "samples/spectral_evolution/1566060_09506_working.sed",
    ))
    .expect("open sed");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "spectral-evolution-sed");
    assert!(records[0].signals.keys().any(|key| key.contains("reflect")));
    let reflectance = records[0]
        .signals
        .iter()
        .find(|(key, _)| key.contains("reflect"))
        .map(|(_, value)| value)
        .expect("reflectance");
    assert_eq!(reflectance.axis.values.len(), 2_151);
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
}

#[test]
fn reads_svc_sig_with_overlap_quality_flag() {
    let records =
        open_path(workspace_file("samples/svc_ger/BNL13001_000_moc.sig")).expect("open sig");

    assert_eq!(records.len(), 1);
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert!(reflectance.axis.values.len() > 900);
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert!(records[0]
        .quality_flags
        .contains(&"matched_overlap_corrected".to_string()));
}

#[test]
fn reads_plain_affn_jcamp_dx() {
    let records =
        open_path(workspace_file("samples/jcamp_dx/nist_water_ir.jdx")).expect("open jcamp");

    assert_eq!(records.len(), 1);
    let signal = records[0].signals.get("signal").expect("signal");
    assert_eq!(signal.axis.values.len(), 3_917);
    assert_eq!(signal.axis.unit, "cm-1");
    assert_eq!(signal.signal_type, SignalType::Transmittance);
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
