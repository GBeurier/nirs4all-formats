use nirs4all_io::{open_path, AxisKind, AxisOrder, SignalType};

#[test]
fn reads_asd_fieldspec_revisions() {
    for (relative, signal_name, signal_type, first_value) in [
        (
            "samples/asd/3L9257.000",
            "reflectance",
            SignalType::Reflectance,
            0.026823,
        ),
        (
            "samples/asd/v6sample00000.asd",
            "raw",
            SignalType::RawCounts,
            29.311738,
        ),
        (
            "samples/asd/v7_field_44231B009.asd",
            "reflectance",
            SignalType::Reflectance,
            18.622284,
        ),
        (
            "samples/asd/v8sample00001.asd",
            "raw",
            SignalType::RawCounts,
            153.995245,
        ),
    ] {
        let records = open_path(workspace_file(relative)).expect("open asd");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].provenance.format, "asd-fieldspec");
        let signal = records[0].signals.get(signal_name).expect(signal_name);
        assert_eq!(signal.axis.values.len(), 2_151);
        assert_eq!(signal.axis.unit, "nm");
        assert_eq!(signal.axis.kind, AxisKind::Wavelength);
        assert_eq!(signal.signal_type, signal_type);
        assert!((signal.values[0] - first_value).abs() < 0.000001);
    }
}

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
fn reads_bruker_opus_native_absorbance_multisignal_file() {
    let records =
        open_path(workspace_file("samples/bruker_opus/617262_1TP_C-1_A5.0")).expect("open opus");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "bruker-opus");
    assert!(record.signals.contains_key("sample_spectrum"));
    assert!(record.signals.contains_key("reference_spectrum"));
    assert!(record.signals.contains_key("sample_interferogram"));
    let absorbance = record.signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 3_578);
    assert_eq!(absorbance.axis.unit, "cm-1");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavenumber);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.values[0] - 0.5524729490).abs() < 0.000001);
}

#[test]
fn reads_bruker_opus_native_reflectance_file() {
    let records =
        open_path(workspace_file("samples/bruker_opus/test_spectra.0")).expect("open opus");

    assert_eq!(records.len(), 1);
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert_eq!(reflectance.axis.values.len(), 4_819);
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert!((reflectance.values[0] - 0.5243431926).abs() < 0.000001);
}

#[test]
fn reads_bruker_opus_duplicate_absorbance_blocks() {
    let records =
        open_path(workspace_file("samples/bruker_opus/BF_lo_01_soil_cal.1")).expect("open opus");

    assert_eq!(records.len(), 1);
    assert!(records[0].signals.contains_key("absorbance"));
    assert!(records[0].signals.contains_key("absorbance_2"));
    let newest = records[0].signals.get("absorbance").expect("absorbance");
    assert!((newest.values[0] - 0.1239784658).abs() < 0.000001);
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
fn reads_envi_spectral_library_from_header() {
    let records =
        open_path(workspace_file("samples/envi_sli/synthetic_lib.hdr")).expect("open envi sli");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "envi-sli");
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("S000"));
    let signal = records[0].signals.get("spectrum").expect("spectrum");
    assert_eq!(signal.axis.values.len(), 200);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.signal_type, SignalType::Unknown);
    assert!((signal.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((signal.axis.values[199] - 2500.0).abs() < 0.000001);
    assert!((signal.values[0] - 0.0367427170).abs() < 0.000001);
}

#[test]
fn reads_envi_spectral_library_from_binary_sidecar() {
    let records =
        open_path(workspace_file("samples/envi_sli/synthetic_lib.sli")).expect("open envi sli");

    assert_eq!(records.len(), 50);
    assert_eq!(records[49].metadata["sample_id"].as_str(), Some("S049"));
    let signal = records[49].signals.get("spectrum").expect("spectrum");
    assert_eq!(signal.axis.values.len(), 200);
    assert!((signal.values[199] - 0.0608757548).abs() < 0.000001);
}

#[test]
fn rejects_envi_standard_image_cube_for_v1() {
    let err = open_path(workspace_file("samples/envi_sli/cubescope-mini-cube.hdr"))
        .expect_err("cube should be out of scope");

    assert!(err.to_string().contains("ENVI Standard"));
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

#[test]
fn reads_galactic_spc_single_even_axis() {
    let records = open_path(workspace_file("samples/galactic_spc/BENZENE.SPC")).expect("open spc");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "galactic-spc");
    let signal = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(signal.axis.values.len(), 1_842);
    assert_eq!(signal.axis.unit, "cm-1");
    assert_eq!(signal.axis.kind, AxisKind::Wavenumber);
    assert_eq!(signal.signal_type, SignalType::Absorbance);
    assert!((signal.values[0] - 0.1015599817).abs() < 0.000001);
}

#[test]
fn reads_galactic_spc_explicit_x_axis() {
    let records = open_path(workspace_file("samples/galactic_spc/s_xy.spc")).expect("open spc");

    assert_eq!(records.len(), 1);
    let signal = records[0]
        .signals
        .get("arbitrary_intensity")
        .expect("arbitrary intensity");
    assert_eq!(signal.axis.values.len(), 512);
    assert_eq!(signal.axis.unit, "min");
    assert!((signal.axis.values[0] - 1.0866667032).abs() < 0.000001);
    assert_eq!(signal.values[0], 45_333.0);
}

#[test]
fn reads_galactic_spc_multi_common_axis() {
    let records = open_path(workspace_file("samples/galactic_spc/nir.spc")).expect("open spc");

    assert_eq!(records.len(), 20);
    let signal = records[0].signals.get("kubelka_munk").expect("km");
    assert_eq!(signal.axis.values.len(), 700);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.signal_type, SignalType::KubelkaMunk);
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("1"));
}

#[test]
fn reads_galactic_spc_xyxy_directory() {
    let records = open_path(workspace_file("samples/galactic_spc/m_xyxy.spc")).expect("open spc");

    assert_eq!(records.len(), 512);
    let signal = records[0].signals.get("abundance").expect("abundance");
    assert_eq!(signal.axis.values.len(), 8);
    assert_eq!(signal.axis.unit, "m/z");
    assert!((signal.axis.values[0] - 16_943.600006).abs() < 0.000001);
    assert_eq!(signal.values[0], 6_823.0);
}

#[test]
fn reads_galactic_spc_old_lsb_header() {
    let records =
        open_path(workspace_file("samples/galactic_spc/LC_DIODE_ARRAY.SPC")).expect("open spc");

    assert!(!records.is_empty());
    let signal = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(signal.axis.values.len(), 181);
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.axis.unit, "nm");
    assert!((signal.values[0] - 0.0040779736).abs() < 0.000001);
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|warning| warning.contains("old_spc_header_limited")));
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
