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
fn reads_avantes_legacy_transmittance_binary() {
    let records =
        open_path(workspace_file("samples/avantes/avantes2.TRM")).expect("open avantes trm");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "avantes-legacy-binary");
    assert!(records[0].signals.contains_key("sample"));
    assert!(records[0].signals.contains_key("white_reference"));
    assert!(records[0].signals.contains_key("dark_reference"));
    let transmittance = records[0]
        .signals
        .get("transmittance")
        .expect("transmittance");
    assert_eq!(transmittance.axis.values.len(), 1_442);
    assert_eq!(transmittance.axis.unit, "nm");
    assert_eq!(transmittance.signal_type, SignalType::Transmittance);
    assert!((transmittance.axis.values[0] - 275.271759).abs() < 0.000001);
    assert!((transmittance.axis.values[1_441] - 1100.133307).abs() < 0.000001);
    assert!((transmittance.values[0] - 11.840215).abs() < 0.000001);
    assert!((transmittance.values[1_441] + 127.179425).abs() < 0.000001);
}

#[test]
fn reads_avantes_legacy_raw_reference_binaries() {
    for (relative, signal_name, first_value) in [
        ("samples/avantes/avantes_reflect.ROH", "scope", 805.0),
        (
            "samples/avantes/1305084U1.DRK",
            "dark_reference",
            785.900024,
        ),
        ("samples/avantes/1305084U1.REF", "white_reference", 856.0),
    ] {
        let records = open_path(workspace_file(relative)).expect("open avantes legacy raw");
        assert_eq!(records.len(), 1);
        let signal = records[0].signals.get(signal_name).expect(signal_name);
        assert_eq!(signal.axis.values.len(), 1_442);
        assert_eq!(signal.signal_type, SignalType::RawCounts);
        assert!((signal.values[0] - first_value).abs() < 0.000001);
    }
}

#[test]
fn reads_avantes_avasoft8_raw_binary() {
    let records =
        open_path(workspace_file("samples/avantes/1904090M1_0003.Raw8")).expect("open raw8");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "avantes-avasoft8-binary");
    assert!(records[0].signals.contains_key("dark_reference"));
    assert!(records[0].signals.contains_key("white_reference"));
    let scope = records[0].signals.get("scope").expect("scope");
    assert_eq!(scope.axis.values.len(), 1_019);
    assert_eq!(scope.signal_type, SignalType::RawCounts);
    assert!((scope.axis.values[0] - 300.013855).abs() < 0.000001);
    assert!((scope.axis.values[1_018] - 899.874878).abs() < 0.000001);
    assert!((scope.values[0] - 267.155243).abs() < 0.000001);
    assert!((scope.values[1_018] - 360.127502).abs() < 0.000001);
}

#[test]
fn reads_avantes_avasoft8_irradiance_binary() {
    let records = open_path(workspace_file("samples/avantes/eg.IRR8")).expect("open irr8");

    assert_eq!(records.len(), 1);
    let irradiance = records[0].signals.get("irradiance").expect("irradiance");
    assert_eq!(irradiance.axis.values.len(), 1_620);
    assert_eq!(irradiance.signal_type, SignalType::Irradiance);
    assert!((irradiance.axis.values[0] - 144.942429).abs() < 0.000001);
    assert!((irradiance.axis.values[1_619] - 1100.441406).abs() < 0.000001);
    assert!((irradiance.values[0] - 1096.812012).abs() < 0.000001);
    assert!((irradiance.values[1_619] - 2009.875).abs() < 0.000001);
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"avantes_irr8_irradiance_calibration_not_applied".to_string()));
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
fn reads_ocean_optics_spectrasuite_text_export() {
    let records =
        open_path(workspace_file("samples/ocean_optics/OOusb4000.txt")).expect("open ocean text");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "ocean-optics-text");
    let signal = records[0].signals.get("processed").expect("processed");
    assert_eq!(signal.axis.values.len(), 3_648);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert!((signal.axis.values[0] - 178.65).abs() < 0.000001);
    assert!((signal.axis.values[3_647] - 888.37).abs() < 0.000001);
    assert!((signal.values[3_647] + 12.792).abs() < 0.000001);
}

#[test]
fn reads_ocean_optics_master_transmission_export() {
    let records = open_path(workspace_file(
        "samples/ocean_optics/FMNH6834.00000001.Master.Transmission",
    ))
    .expect("open master transmission");

    assert_eq!(records.len(), 1);
    let transmittance = records[0]
        .signals
        .get("transmittance")
        .expect("transmittance");
    assert_eq!(transmittance.axis.values.len(), 3_648);
    assert_eq!(transmittance.signal_type, SignalType::Transmittance);
    assert!((transmittance.axis.values[0] - 178.53).abs() < 0.000001);
    assert!((transmittance.values[0] - 95.380).abs() < 0.000001);
    assert!((transmittance.values[3_647] - 25.753).abs() < 0.000001);
}

#[test]
fn reads_ocean_optics_craic_reflectance_export() {
    let records =
        open_path(workspace_file("samples/ocean_optics/CRAIC_export.txt")).expect("open craic");

    assert_eq!(records.len(), 1);
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert_eq!(reflectance.axis.values.len(), 3_761);
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert!((reflectance.axis.values[0] - 280.11).abs() < 0.000001);
    assert!((reflectance.values[0] - 13.3999).abs() < 0.000001);
    assert!((reflectance.values[3_760] - 169.6574).abs() < 0.000001);
}

#[test]
fn reads_ocean_optics_jaz_multichannel_export() {
    let records = open_path(workspace_file("samples/ocean_optics/jazspec.jaz")).expect("open jaz");

    assert_eq!(records.len(), 1);
    assert!(records[0].signals.contains_key("dark_reference"));
    assert!(records[0].signals.contains_key("white_reference"));
    assert!(records[0].signals.contains_key("sample"));
    let processed = records[0].signals.get("processed").expect("processed");
    assert_eq!(processed.axis.values.len(), 2_048);
    assert!((processed.axis.values[2_047] - 886.439331).abs() < 0.000001);
    assert!((processed.values[2_047] - 13.679238).abs() < 0.000001);
}

#[test]
fn reads_ocean_optics_jaz_irradiance_export() {
    let records =
        open_path(workspace_file("samples/ocean_optics/irrad.JazIrrad")).expect("open jaz irrad");

    assert_eq!(records.len(), 1);
    let irradiance = records[0].signals.get("irradiance").expect("irradiance");
    assert_eq!(irradiance.axis.values.len(), 2_048);
    assert_eq!(irradiance.signal_type, SignalType::Irradiance);
    assert!((irradiance.axis.values[2_047] - 891.915466).abs() < 0.000001);
    assert!((irradiance.values[2_047] - 3.643908).abs() < 0.000001);
}

#[test]
fn reads_ocean_optics_linux_procspec_archive() {
    let records = open_path(workspace_file(
        "samples/ocean_optics/OceanOptics_Linux.ProcSpec",
    ))
    .expect("open procspec");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "ocean-optics-procspec");
    assert!(records[0].provenance.warnings.is_empty());
    let processed = records[0].signals.get("processed").expect("processed");
    assert_eq!(processed.axis.values.len(), 3_648);
    assert_eq!(processed.axis.unit, "nm");
    assert_eq!(processed.axis.kind, AxisKind::Wavelength);
    assert!((processed.axis.values[0] - 176.3604183).abs() < 0.000001);
    assert!((processed.axis.values[3_647] - 893.6943397004063).abs() < 0.000001);
    assert_eq!(processed.signal_type, SignalType::Unknown);
    assert!((processed.values[0] - 0.0).abs() < 0.000001);
    assert!((processed.values[3_647] - 125.07433102081265).abs() < 0.000001);
    assert!(records[0].signals.contains_key("sample"));
    assert!(records[0].signals.contains_key("dark_reference"));
    assert!(records[0].signals.contains_key("white_reference"));
}

#[test]
fn reads_ocean_optics_windows_and_reference_procspec_archives() {
    let windows = open_path(workspace_file(
        "samples/ocean_optics/OceanOptics_Windows.ProcSpec",
    ))
    .expect("open windows procspec");
    let windows_processed = windows[0].signals.get("processed").expect("processed");
    assert_eq!(windows_processed.axis.values.len(), 2_048);
    assert!((windows_processed.values[0] - 282.8571428571289).abs() < 0.000001);
    assert!((windows_processed.values[2_047] - 40.05032131664623).abs() < 0.000001);

    let whiteref =
        open_path(workspace_file("samples/ocean_optics/whiteref.ProcSpec")).expect("open whiteref");
    let whiteref_processed = whiteref[0].signals.get("processed").expect("processed");
    assert_eq!(whiteref_processed.axis.values.len(), 3_648);
    assert!((whiteref_processed.values[0] - 0.0).abs() < 0.000001);
    assert!((whiteref_processed.values[3_647] - 97.29425028184893).abs() < 0.000001);
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
fn reads_msa_iso22029_xy_variants() {
    let records = open_path(workspace_file(
        "samples/msa_iso22029/ISO_22029_2022_compliance.msa",
    ))
    .expect("open msa");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "emsa-mas-msa");
    let signal = records[0].signals.get("counts").expect("counts");
    assert_eq!(signal.axis.values.len(), 21);
    assert_eq!(signal.axis.unit, "eV");
    assert_eq!(signal.signal_type, SignalType::RawCounts);
    assert!((signal.axis.values[0] - 520.13).abs() < 0.000001);
    assert!((signal.axis.values[20] - 580.50).abs() < 0.000001);
    assert_eq!(signal.values[0], 4_066.0);
    assert_eq!(signal.values[20], 4_217.0);

    let records = open_path(workspace_file(
        "samples/msa_iso22029/ISO_22029_2022_compliance_XY_NCOLUMNS2.msa",
    ))
    .expect("open msa ncolumns");
    let signal = records[0].signals.get("counts").expect("counts");
    assert_eq!(signal.axis.values.len(), 21);
    assert_eq!(signal.values[20], 4_217.0);
}

#[test]
fn reads_msa_iso22029_y_axis_reconstruction() {
    let records = open_path(workspace_file(
        "samples/msa_iso22029/example2_NCOLUMNS5.msa",
    ))
    .expect("open msa y");

    assert_eq!(records.len(), 1);
    let signal = records[0]
        .signals
        .get("x_ray_intensity")
        .expect("x-ray intensity");
    assert_eq!(signal.axis.values.len(), 80);
    assert_eq!(signal.axis.unit, "eV");
    assert!((signal.axis.values[0] - 0.0).abs() < 0.000001);
    assert!((signal.axis.values[79] - 790.0).abs() < 0.000001);
    assert!((signal.values[0] - 65.820).abs() < 0.000001);
    assert!((signal.values[79] - 49.442).abs() < 0.000001);
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
fn reads_jcamp_sqz_packed_xydata() {
    let records = open_path(workspace_file("samples/jcamp_dx/BRUKSQZ.DX")).expect("open sqz");

    assert_eq!(records.len(), 1);
    let signal = records[0].signals.get("signal").expect("signal");
    assert_eq!(signal.axis.values.len(), 16_384);
    assert_eq!(signal.axis.unit, "hz");
    assert!((signal.axis.values[0] - 24_038.5).abs() < 0.000001);
    assert!((signal.axis.values[16_383] - 0.0).abs() < 0.000001);
    assert_eq!(signal.values[0], 2_259_260.0);
    assert_eq!(signal.values[16_383], 1_505_988.0);
}

#[test]
fn reads_jcamp_dif_dup_packed_xydata() {
    let records = open_path(workspace_file("samples/jcamp_dx/BRUKDIF.DX")).expect("open dif");

    assert_eq!(records.len(), 1);
    let signal = records[0].signals.get("signal").expect("signal");
    assert_eq!(signal.axis.values.len(), 16_384);
    assert_eq!(signal.values[0], 2_254_931.0);
    assert_eq!(signal.values[16_383], 1_513_177.0);
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|warning| warning.contains("npoints_truncated")));
}

#[test]
fn reads_jcamp_mixed_squeeze_difference_file() {
    let records = open_path(workspace_file("samples/jcamp_dx/SPECFILE.DX")).expect("open specfile");

    assert_eq!(records.len(), 1);
    let signal = records[0].signals.get("signal").expect("signal");
    assert_eq!(signal.axis.values.len(), 1_801);
    assert_eq!(signal.signal_type, SignalType::Transmittance);
    assert!((signal.axis.values[0] - 400.0).abs() < 0.000001);
    assert!((signal.axis.values[1_800] - 4_000.0).abs() < 0.000001);
    assert!((signal.values[0] - 97.737187).abs() < 0.000001);
    assert!((signal.values[1_800] - 82.830985).abs() < 0.000001);
}

#[test]
fn reads_jcamp_ntuples_spectrum_real_imag_pages() {
    let records = open_path(workspace_file("samples/jcamp_dx/BRUKNTUP.DX")).expect("open ntuples");

    assert_eq!(records.len(), 1);
    let real = records[0].signals.get("real").expect("real");
    let imaginary = records[0].signals.get("imaginary").expect("imaginary");
    assert_eq!(real.axis.values.len(), 16_384);
    assert_eq!(imaginary.axis.values.len(), 16_384);
    assert_eq!(real.axis.unit, "hz");
    assert_eq!(real.axis.kind, AxisKind::Frequency);
    assert!((real.axis.values[0] - 24_038.5).abs() < 0.000001);
    assert!((real.axis.values[16_383] - 0.0).abs() < 0.000001);
    assert_eq!(real.values[0], 2_254_931.0);
    assert_eq!(real.values[16_383], 1_513_177.0);
    assert_eq!(imaginary.values[0], -6_966_283.0);
    assert_eq!(imaginary.values[16_383], -7_303_022.0);
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|warning| warning.contains("jcamp_ntuples_npoints_truncated")));
}

#[test]
fn reads_jcamp_ntuples_fid_real_imag_pages() {
    let records = open_path(workspace_file("samples/jcamp_dx/TESTFID.DX")).expect("open fid");

    assert_eq!(records.len(), 1);
    let real = records[0].signals.get("real").expect("real");
    let imaginary = records[0].signals.get("imaginary").expect("imaginary");
    assert_eq!(real.axis.values.len(), 16_384);
    assert_eq!(imaginary.axis.values.len(), 16_384);
    assert_eq!(real.axis.unit, "s");
    assert_eq!(real.axis.kind, AxisKind::Index);
    assert!((real.axis.values[0] - 0.0).abs() < 0.000001);
    assert!((real.axis.values[16_383] - 0.6815317).abs() < 0.000001);
    assert!((real.values[0] - 2_979.837824796).abs() < 0.000001);
    assert!((real.values[16_383] + 60_241.607962368005).abs() < 0.000001);
    assert!((imaginary.values[0] - 6_214.555863824).abs() < 0.000001);
    assert!((imaginary.values[16_383] + 6_063.227393114).abs() < 0.000001);
}

#[test]
fn reads_jcamp_link_xypoints_ocean_optics_blocks() {
    let records = open_path(workspace_file(
        "samples/ocean_optics/OceanOptics_period.jdx",
    ))
    .expect("open link jcamp");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "jcamp-dx");
    assert!(records[0].signals.contains_key("sample"));
    assert!(records[0].signals.contains_key("dark_reference"));
    assert!(records[0].signals.contains_key("white_reference"));
    let processed = records[0].signals.get("processed").expect("processed");
    assert_eq!(processed.axis.values.len(), 3_648);
    assert_eq!(processed.axis.unit, "nm");
    assert_eq!(processed.signal_type, SignalType::Transmittance);
    assert!((processed.axis.values[0] - 176.36).abs() < 0.000001);
    assert!((processed.axis.values[3_647] - 893.69).abs() < 0.000001);
    assert!((processed.values[0] - 0.0).abs() < 0.000001);
    assert!((processed.values[3_647] - 171.97706959107845).abs() < 0.000001);
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|warning| warning.contains("jcamp_link_processed_zero_denominator")));
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
fn reads_ocean_optics_spc_with_galactic_spc_layout() {
    let records =
        open_path(workspace_file("samples/ocean_optics/OceanOptics.spc")).expect("open spc");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "galactic-spc");
    let signal = records[0]
        .signals
        .get("transmittance")
        .expect("transmittance");
    assert_eq!(signal.axis.values.len(), 3_648);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.signal_type, SignalType::Transmittance);
    assert!((signal.axis.values[0] - 176.36041259765625).abs() < 0.000001);
    assert!((signal.axis.values[3_647] - 893.6943359375).abs() < 0.000001);
    assert!((signal.values[0] - 0.0).abs() < 0.000001);
    assert!((signal.values[3_647] - 119.4251708984375).abs() < 0.000001);
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
