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
fn reads_nicolet_omnic_spa_single_spectrum() {
    let records =
        open_path(workspace_file("samples/nicolet_omnic/2-BaSO4_0.SPA")).expect("open spa");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "nicolet-omnic-spa");
    assert_eq!(
        record.metadata["spectrum_title"].as_str(),
        Some("2-BaSO4_0")
    );
    let absorbance = record.signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 11_098);
    assert_eq!(absorbance.axis.unit, "cm-1");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavenumber);
    assert_eq!(absorbance.axis.order, AxisOrder::Descending);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 6000.041015625).abs() < 0.000001);
    assert!((absorbance.axis.values[11_097] - 649.9039916992188).abs() < 0.000001);
    assert!((absorbance.values[0] - 2.2815363407).abs() < 0.000001);
    assert!((absorbance.values[11_097] - 6.0).abs() < 0.000001);
}

#[test]
fn reads_nicolet_omnic_spg_group_spectra() {
    let records = open_path(workspace_file("samples/nicolet_omnic/wodger.spg")).expect("open spg");

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].provenance.format, "nicolet-omnic-spg");
    assert!(records[0].metadata["spectrum_title"]
        .as_str()
        .expect("title")
        .starts_with("vz0470.spa"));
    let first = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(first.axis.values.len(), 5_549);
    assert_eq!(first.axis.unit, "cm-1");
    assert_eq!(first.axis.kind, AxisKind::Wavenumber);
    assert_eq!(first.signal_type, SignalType::Absorbance);
    assert!((first.values[0] - 1.9831526279).abs() < 0.000001);
    let second = records[1].signals.get("absorbance").expect("absorbance");
    assert!((second.values[0] - 2.0048975945).abs() < 0.000001);
}

#[test]
fn reads_nicolet_omnic_srs_tg_gc_series() {
    let records = open_path(workspace_file("samples/nicolet_omnic/GC_Demo.srs")).expect("open srs");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "nicolet-omnic-srs");
    assert_eq!(record.metadata["series_variant"].as_str(), Some("tg_gc"));
    assert_eq!(record.metadata["series_y_len"].as_u64(), Some(788));
    assert_eq!(
        record.metadata["omnic_srs_data_header_offset"].as_u64(),
        Some(5_584)
    );
    assert_eq!(
        record.metadata["omnic_srs_background_header_offset"].as_u64(),
        Some(7_044)
    );
    assert_eq!(
        record.metadata["omnic_srs_data_offset"].as_u64(),
        Some(20_616)
    );
    let transmittance = record.signals.get("transmittance").expect("transmittance");
    assert_eq!(transmittance.axis.values.len(), 1_738);
    assert_eq!(transmittance.values.len(), 1_369_544);
    assert_eq!(transmittance.dims, vec!["y".to_string(), "x".to_string()]);
    assert_eq!(transmittance.axis.unit, "cm-1");
    assert_eq!(transmittance.axis.kind, AxisKind::Wavenumber);
    assert_eq!(transmittance.axis.order, AxisOrder::Descending);
    assert_eq!(transmittance.signal_type, SignalType::Transmittance);
    assert!((transmittance.axis.values[0] - 3999.704346).abs() < 0.000001);
    assert!((transmittance.axis.values[1_737] - 649.903809).abs() < 0.000001);
    assert!((transmittance.values[0] - 99.701584).abs() < 0.000001);
    assert!((transmittance.values[1_369_543] - 100.124908).abs() < 0.000001);
    assert!((transmittance.values.iter().sum::<f64>() - 136_739_704.182004).abs() < 0.01);
}

#[test]
fn reads_nicolet_omnic_srs_tgair_series() {
    let records = open_path(workspace_file("samples/nicolet_omnic/TGAIR.srs")).expect("open srs");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "nicolet-omnic-srs");
    assert_eq!(record.metadata["series_y_len"].as_u64(), Some(335));
    assert_eq!(
        record.metadata["omnic_srs_data_header_offset"].as_u64(),
        Some(14_032)
    );
    assert_eq!(
        record.metadata["omnic_srs_background_header_offset"].as_u64(),
        Some(20_836)
    );
    assert_eq!(
        record.metadata["omnic_srs_data_offset"].as_u64(),
        Some(30_888)
    );
    let absorbance = record.signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 1_868);
    assert_eq!(absorbance.values.len(), 625_780);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 3999.706055).abs() < 0.000001);
    assert!((absorbance.axis.values[1_867] - 399.199188).abs() < 0.000001);
    assert!((absorbance.values[0] + 0.007524).abs() < 0.000001);
    assert!((absorbance.values[625_779] - 0.002916).abs() < 0.000001);
    assert!((absorbance.values.iter().sum::<f64>() - 4699.720344).abs() < 0.001);
}

#[test]
fn reads_perkin_elmer_sp_single_spectrum() {
    let records = open_path(workspace_file("samples/perkin_elmer/spectra.sp")).expect("open sp");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "perkin-elmer-sp");
    assert_eq!(record.metadata["sample_id"].as_str(), Some("strip01"));
    assert_eq!(record.metadata["instrument"].as_str(), Some("Spectrum One"));
    assert_eq!(record.metadata["detector"].as_str(), Some("MCT"));
    assert_eq!(
        record.metadata["scan_date"].as_str(),
        Some("Thu Mar 09 09:17:56 2006")
    );
    let absorbance = record.signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 3_301);
    assert_eq!(absorbance.axis.unit, "cm-1");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavenumber);
    assert_eq!(absorbance.axis.order, AxisOrder::Descending);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert_eq!(absorbance.unit.as_deref(), Some("A"));
    assert!((absorbance.axis.values[0] - 4000.0).abs() < 0.000001);
    assert!((absorbance.axis.values[3_300] - 700.0).abs() < 0.000001);
    assert!((absorbance.values[0] - 0.03723936007346753).abs() < 0.000001);
    assert!((absorbance.values[3_300] - 0.004175562077308466).abs() < 0.000001);
    assert!((absorbance.values.iter().sum::<f64>() - 117.16218877705974).abs() < 0.000001);
}

#[test]
fn reads_buchi_nircal_project_spectra() {
    let records = open_path(workspace_file(
        "samples/buchi_nircal/muestras-tejido-foliar_transfer.nir",
    ))
    .expect("open nircal");

    assert_eq!(records.len(), 20);
    assert_eq!(records[0].provenance.format, "buchi-nircal");
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("105/78G"));
    assert_eq!(records[19].metadata["sample_id"].as_str(), Some("105/59"));
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 1_501);
    assert_eq!(absorbance.axis.unit, "cm-1");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavenumber);
    assert_eq!(absorbance.axis.order, AxisOrder::Ascending);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 4000.0).abs() < 0.000001);
    assert!((absorbance.axis.values[1_500] - 10000.0).abs() < 0.000001);
    assert!((absorbance.values[0] - 0.1812854070008529).abs() < 0.000001);
    assert!((absorbance.values[1_500] - 0.667603536333019).abs() < 0.000001);
    assert!((absorbance.values.iter().sum::<f64>() - 787.4193555920597).abs() < 0.000001);
}

#[test]
fn reads_jasco_jws_single_channel_files() {
    let records = open_path(workspace_file("samples/jasco/243.jws")).expect("open jws");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "jasco-jws");
    assert_eq!(record.metadata["channel_count"].as_u64(), Some(1));
    assert_eq!(record.metadata["point_count"].as_u64(), Some(7_729));
    let signal = record.signals.get("signal").expect("signal");
    assert_eq!(signal.axis.values.len(), 7_729);
    assert_eq!(signal.axis.unit, "cm-1");
    assert_eq!(signal.axis.kind, AxisKind::Wavenumber);
    assert_eq!(signal.axis.order, AxisOrder::Ascending);
    assert_eq!(signal.signal_type, SignalType::Unknown);
    assert!((signal.axis.values[0] - 349.0525166555562).abs() < 0.000001);
    assert!((signal.axis.values[7_728] - 7800.6487838216835).abs() < 0.000001);
    assert!((signal.values[0] - 38.420169830322266).abs() < 0.000001);
    assert!((signal.values[7_728] - 35.47404479980469).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 316_675.31128692627).abs() < 0.000001);

    let records =
        open_path(workspace_file("samples/jasco/sample_fluorescence.jws")).expect("open jws");
    assert_eq!(records.len(), 1);
    let signal = records[0].signals.get("signal").expect("signal");
    assert_eq!(signal.axis.values.len(), 301);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.axis.order, AxisOrder::Ascending);
    assert!((signal.axis.values[0] - 400.0).abs() < 0.000001);
    assert!((signal.axis.values[300] - 700.0).abs() < 0.000001);
    assert!((signal.values[0] - 18.799175262451172).abs() < 0.000001);
    assert!((signal.values[300] - 5.624600887298584).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 75_506.5075211525).abs() < 0.000001);
}

#[test]
fn reads_jasco_jws_multichannel_file() {
    let records =
        open_path(workspace_file("samples/jasco/sample_CD_HT_Abs.jws")).expect("open jws");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "jasco-jws");
    assert_eq!(record.metadata["channel_count"].as_u64(), Some(3));
    assert_eq!(record.metadata["point_count"].as_u64(), Some(1_501));
    assert_eq!(record.signal_type, SignalType::Unknown);

    let channel_1 = record.signals.get("channel_1").expect("channel_1");
    assert_eq!(channel_1.axis.values.len(), 1_501);
    assert_eq!(channel_1.axis.unit, "nm");
    assert_eq!(channel_1.axis.kind, AxisKind::Wavelength);
    assert_eq!(channel_1.axis.order, AxisOrder::Descending);
    assert_eq!(channel_1.signal_type, SignalType::Unknown);
    assert!((channel_1.axis.values[0] - 350.0).abs() < 0.000001);
    assert!((channel_1.axis.values[1_500] - 200.0).abs() < 0.000001);
    assert!((channel_1.values[0] - 0.3416369557380676).abs() < 0.000001);
    assert!((channel_1.values[1_500] - 6.220218658447266).abs() < 0.000001);
    assert!((channel_1.values.iter().sum::<f64>() - 3706.048405816895).abs() < 0.000001);

    let channel_2 = record.signals.get("channel_2").expect("channel_2");
    assert!((channel_2.values[0] - 250.94847106933594).abs() < 0.000001);
    assert!((channel_2.values[1_500] - 364.5225830078125).abs() < 0.000001);
    assert!((channel_2.values.iter().sum::<f64>() - 401_403.0902252197).abs() < 0.000001);

    let channel_3 = record.signals.get("channel_3").expect("channel_3");
    assert!((channel_3.values[0] - 0.7128385901451111).abs() < 0.000001);
    assert!((channel_3.values[1_500] - 1.899193286895752).abs() < 0.000001);
    assert!((channel_3.values.iter().sum::<f64>() - 1356.2173843979836).abs() < 0.000001);
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
fn reads_row_oriented_spectral_tables() {
    for (relative, signal_name, axis_unit, axis_len, signal_type, first_x, first_y) in [
        (
            "samples/siware_neospectra/synthetic_neospectra.csv",
            "absorbance",
            "nm",
            200,
            SignalType::Absorbance,
            1100.0,
            0.036743,
        ),
        (
            "samples/modtran/synthetic_albedo.dat",
            "albedo",
            "um",
            200,
            SignalType::Reflectance,
            1.1,
            0.3891,
        ),
        (
            "samples/envi_sli/ecostress_b.spectrum.txt",
            "reflectance",
            "um",
            2_151,
            SignalType::Reflectance,
            0.35,
            1.471,
        ),
        (
            "samples/shimadzu/synthetic_uvprobe.txt",
            "sample_s000",
            "nm",
            200,
            SignalType::Unknown,
            1100.0,
            0.036743,
        ),
        (
            "samples/raman_witec/Si-wafer-Raman-Spectrum-1.txt",
            "spectrum__000__spec_data_1",
            "nm",
            1_600,
            SignalType::RawCounts,
            530.7816803,
            356.8500061,
        ),
    ] {
        let records = open_path(workspace_file(relative)).expect("open row spectral table");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].provenance.format, "row-spectral-table");
        let signal = records[0].signals.get(signal_name).expect(signal_name);
        assert_eq!(signal.axis.unit, axis_unit);
        assert_eq!(signal.axis.kind, AxisKind::Wavelength);
        assert_eq!(signal.axis.values.len(), axis_len);
        assert_eq!(signal.signal_type, signal_type);
        assert!((signal.axis.values[0] - first_x).abs() < 0.000001);
        assert!((signal.values[0] - first_y).abs() < 0.000001);
    }
}

#[test]
fn reads_jasco_and_idl_text_exports_as_row_tables() {
    let records =
        open_path(workspace_file("samples/jasco/synthetic_jws_export.txt")).expect("open jasco");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "row-spectral-table");
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 200);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((absorbance.values[0] - 0.036743).abs() < 0.000001);

    let records =
        open_path(workspace_file("samples/csv_tsv/idl_envi_output.txt")).expect("open idl");
    assert_eq!(records.len(), 1);
    let s000 = records[0].signals.get("s000").expect("s000");
    assert_eq!(records[0].signals.len(), 5);
    assert_eq!(s000.axis.values.len(), 200);
    assert_eq!(s000.axis.unit, "nm");
    assert!((s000.values[0] - 0.0367).abs() < 0.000001);
}

#[test]
fn reads_siware_api_json_measurement() {
    let records = open_path(workspace_file(
        "samples/siware_api/synthetic_siware_api.json",
    ))
    .expect("open siware json");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "siware-api-json");
    assert_eq!(
        records[0].metadata["measurement_id"].as_str(),
        Some("meas-2026-05-18-001")
    );
    assert_eq!(
        records[0].metadata["instrument_model"].as_str(),
        Some("NeoSpectra Cloud")
    );
    assert_eq!(records[0].targets["protein"].as_f64(), Some(13.7));
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 200);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.values[0] - 0.024870592439159966).abs() < 0.000001);
}

#[test]
fn reads_synthetic_nirs_netcdf_dataset() {
    let records =
        open_path(workspace_file("samples/netcdf/synthetic_nirs.nc")).expect("open netcdf");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "netcdf-nirs");
    assert_eq!(records[0].metadata["sample_index"].as_u64(), Some(0));
    assert!(records[0].targets.contains_key("protein"));
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 200);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavelength);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((absorbance.axis.values[199] - 2500.0).abs() < 0.000001);
    assert!((absorbance.values[0] - 0.036742717027664185).abs() < 0.000001);
}

#[test]
fn rejects_non_nirs_netcdf_containers() {
    for relative in [
        "samples/andi_ms/gc01_0812_066.cdf",
        "samples/netcdf/air_temperature.nc",
    ] {
        let err = open_path(workspace_file(relative)).expect_err("non-NIRS NetCDF");
        assert!(err.to_string().contains("no spectra variable"));
    }
}

#[test]
fn reads_synthetic_nirs_hdf5_dataset() {
    let records = open_path(workspace_file("samples/hdf5/synthetic_nirs.h5")).expect("open hdf5");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "hdf5-nirs");
    assert_eq!(records[0].metadata["container"].as_str(), Some("hdf5"));
    assert_eq!(records[0].metadata["group_path"].as_str(), Some("/"));
    assert_eq!(records[0].metadata["sample_index"].as_u64(), Some(0));
    assert!(records[0].targets.contains_key("protein"));
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 200);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavelength);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((absorbance.axis.values[199] - 2500.0).abs() < 0.000001);
    assert!((absorbance.values[0] - 0.036742717027664185).abs() < 0.000001);
}

#[test]
fn reads_nested_fgi_hdf5_payload() {
    let records = open_path(workspace_file("samples/fgi/synthetic_fgi.h5")).expect("open fgi hdf5");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "hdf5-nirs");
    assert_eq!(
        records[0].metadata["group_path"].as_str(),
        Some("/Measurement1")
    );
    assert_eq!(
        records[0].metadata["group_attributes"]["instrument"].as_str(),
        Some("FGI-mock")
    );
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 200);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
}

#[test]
fn rejects_non_nirs_hdf5_containers() {
    let err =
        open_path(workspace_file("samples/hdf5/vlen_string_dset.h5")).expect_err("non-NIRS HDF5");
    assert!(err.to_string().contains("no spectra dataset"));
}

#[test]
fn reads_synthetic_matlab_v5_dataset() {
    let records =
        open_path(workspace_file("samples/matlab/synthetic_nirs_v5.mat")).expect("open matlab v5");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "matlab-mat-v5");
    assert_eq!(records[0].metadata["container"].as_str(), Some("matlab_v5"));
    assert_eq!(
        records[0].metadata["matrix_orientation"].as_str(),
        Some("samples_by_bands")
    );
    assert_eq!(records[0].metadata["sample_index"].as_u64(), Some(0));
    assert_eq!(records[0].targets["y"].as_f64(), Some(10.53211185428271));
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 200);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavelength);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((absorbance.axis.values[199] - 2500.0).abs() < 0.000001);
    assert!((absorbance.values[0] - 0.03674271524932157).abs() < 0.000001);
    assert!((absorbance.values[199] + 0.1465858247257086).abs() < 0.000001);
}

#[test]
fn reads_synthetic_matlab_v73_dataset() {
    let records = open_path(workspace_file("samples/matlab/synthetic_nirs_v73.mat"))
        .expect("open matlab v73");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "matlab-mat-v73");
    assert_eq!(
        records[0].metadata["container"].as_str(),
        Some("matlab_v73_hdf5")
    );
    assert_eq!(
        records[0].metadata["matrix_orientation"].as_str(),
        Some("bands_by_samples")
    );
    assert_eq!(records[0].targets["y"].as_f64(), Some(10.53211185428271));
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 200);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.values[0] - 0.03674271524932157).abs() < 0.000001);
    assert!((absorbance.values[199] + 0.1465858247257086).abs() < 0.000001);
}

#[test]
fn reads_eigenvector_corn_matlab_dso_dataset() {
    let records =
        open_path(workspace_file("samples/matlab/eigenvector_corn.mat")).expect("open corn mat");

    assert_eq!(records.len(), 80);
    assert_eq!(records[0].provenance.format, "matlab-eigenvector-corn");
    assert_eq!(
        records[0].metadata["dataset"].as_str(),
        Some("eigenvector_corn")
    );
    assert_eq!(records[0].targets["moisture"].as_f64(), Some(10.448));
    assert_eq!(records[0].targets["oil"].as_f64(), Some(3.687));
    assert_eq!(records[0].targets["protein"].as_f64(), Some(8.746));
    assert_eq!(records[0].targets["starch"].as_f64(), Some(64.838));
    let m5 = records[0].signals.get("m5spec").expect("m5spec");
    assert_eq!(m5.axis.values.len(), 700);
    assert_eq!(m5.axis.unit, "nm");
    assert_eq!(m5.axis.kind, AxisKind::Wavelength);
    assert_eq!(m5.signal_type, SignalType::Absorbance);
    assert!((m5.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((m5.axis.values[699] - 2498.0).abs() < 0.000001);
    assert!((m5.values[0] - 0.0444948).abs() < 0.000001);
    assert!((m5.values[699] - 0.730594).abs() < 0.000001);
    assert!((records[79].signals["m5spec"].values[699] - 0.728245).abs() < 0.000001);
    assert_eq!(records[79].targets["starch"].as_f64(), Some(64.853));
}

#[test]
fn reads_eigenvector_nir_shootout_matlab_dso_dataset() {
    let records = open_path(workspace_file(
        "samples/matlab/eigenvector_nir_shootout_2002.mat",
    ))
    .expect("open shootout mat");

    assert_eq!(records.len(), 655);
    assert_eq!(
        records[0].provenance.format,
        "matlab-eigenvector-nir-shootout"
    );
    assert_eq!(records[0].metadata["split"].as_str(), Some("calibrate"));
    assert_eq!(
        records[0].targets["weight"].as_f64(),
        Some(378.0199890136719)
    );
    assert_eq!(
        records[0].targets["hardness"].as_f64(),
        Some(20.899999618530273)
    );
    assert_eq!(
        records[0].targets["assay"].as_f64(),
        Some(200.10000610351562)
    );
    let instrument_1 = records[0]
        .signals
        .get("instrument_1")
        .expect("instrument_1");
    assert_eq!(instrument_1.axis.values.len(), 650);
    assert_eq!(instrument_1.axis.unit, "nm");
    assert_eq!(instrument_1.axis.kind, AxisKind::Wavelength);
    assert!((instrument_1.axis.values[0] - 600.0).abs() < 0.000001);
    assert!((instrument_1.axis.values[649] - 1898.0).abs() < 0.000001);
    assert!((instrument_1.values[0] - 3.222009).abs() < 0.000001);
    assert!((instrument_1.values[649] - 4.131819).abs() < 0.000001);
    assert_eq!(records[654].metadata["split"].as_str(), Some("validate"));
    assert!((records[654].signals["instrument_1"].values[649] - 4.089232).abs() < 0.000001);
    assert_eq!(records[654].targets["assay"].as_f64(), Some(197.5));
}

#[test]
fn reads_spectrochempy_dso_matlab_dataset() {
    let records =
        open_path(workspace_file("samples/matlab/scpdata_dso.mat")).expect("open DSO mat");

    assert_eq!(records.len(), 20);
    assert_eq!(records[0].provenance.format, "matlab-spectrochempy-dso");
    assert_eq!(
        records[0].metadata["dso_name"].as_str(),
        Some("Group sust_base line withoutEQU.SPG")
    );
    assert!(
        (records[0].metadata["pressure_bar"]
            .as_f64()
            .expect("pressure")
            - 7.3072899386666675e-06)
            .abs()
            < 1e-12
    );
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 426);
    assert_eq!(absorbance.axis.unit, "cm-1");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavenumber);
    assert_eq!(absorbance.axis.order, AxisOrder::Descending);
    assert!((absorbance.axis.values[0] - 2210.059237650298).abs() < 0.000001);
    assert!((absorbance.axis.values[425] - 1800.2533144385284).abs() < 0.000001);
    assert!((absorbance.values[0] - 9.120255708694458e-05).abs() < 0.000001);
    assert!((absorbance.values[425] - 0.0006451588124036789).abs() < 0.000001);
    assert!(
        (records[19].signals["absorbance"].values[425] - 0.0012210346758365631).abs() < 0.000001
    );
}

#[test]
fn reads_spectrochempy_als2004_matlab_dataset() {
    let records = open_path(workspace_file("samples/matlab/scpdata_als2004dataset.MAT"))
        .expect("open ALS mat");

    assert_eq!(records.len(), 204);
    assert_eq!(records[0].provenance.format, "matlab-als2004");
    let signal = records[0].signals.get("signal").expect("signal");
    assert_eq!(signal.axis.values.len(), 96);
    assert_eq!(signal.axis.unit, "index");
    assert_eq!(signal.axis.kind, AxisKind::Index);
    assert_eq!(signal.signal_type, SignalType::Unknown);
    assert!((signal.values[0] - 0.015245458206131416).abs() < 0.000001);
    assert!((signal.values[95] + 0.00026308635228991425).abs() < 0.000001);
    assert_eq!(
        records[0].targets["component_3"].as_f64(),
        Some(0.027500394939256604)
    );
    assert!((records[203].signals["signal"].values[0] - 0.001698897211274964).abs() < 0.000001);
    assert_eq!(
        records[203].targets["component_4"].as_f64(),
        Some(0.008705362013575775)
    );
}

#[test]
fn reads_synthetic_excel_workbook() {
    let records =
        open_path(workspace_file("samples/excel/synthetic_nirs.xlsx")).expect("open xlsx");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "excel-xlsx");
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("S000"));
    assert_eq!(records[0].metadata["sheet"].as_str(), Some("spectra"));
    assert_eq!(
        records[0].targets["protein"].as_f64(),
        Some(10.53211185428271)
    );
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 200);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavelength);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((absorbance.axis.values[199] - 2500.0).abs() < 0.000001);
    assert!((absorbance.values[0] - 0.03674271524932157).abs() < 0.000001);
}

#[test]
fn reads_pp_systems_row_tables_with_multiple_signals() {
    let records =
        open_path(workspace_file("samples/pp_systems/synthetic_unispec.SPT")).expect("open spt");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "row-spectral-table");
    assert!(records[0].signals.contains_key("dn_white"));
    assert!(records[0].signals.contains_key("dn_target"));
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert_eq!(reflectance.axis.values.len(), 200);
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert!((reflectance.values[0] - 0.6787).abs() < 0.000001);

    let records = open_path(workspace_file(
        "samples/pp_systems/synthetic_unispec_dc.SPU",
    ))
    .expect("open spu");
    assert!(records[0].signals.contains_key("channel_a_dn"));
    assert!(records[0].signals.contains_key("channel_b_dn"));
    assert!((records[0].signals["reflectance"].values[0] - 1.2646).abs() < 0.000001);
}

#[test]
fn reads_usgs_specpr_ascii_spectrum() {
    let records = open_path(workspace_file("samples/specpr/asphalt_gds366.27407.asc"))
        .expect("open usgs asc");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "row-spectral-table");
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    let stddev = records[0]
        .signals
        .get("standard_deviation")
        .expect("standard deviation");
    assert_eq!(reflectance.axis.unit, "um");
    assert_eq!(reflectance.axis.values.len(), 2_151);
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert_eq!(stddev.signal_type, SignalType::Unknown);
    assert!((reflectance.values[0] - 0.042736).abs() < 0.000001);
}

#[test]
fn reads_one_spectrum_per_row_matrix_exports() {
    for (relative, records_len, first_target) in [
        (
            "samples/foss_winisi/synthetic_winisi_export.txt",
            50,
            Some("protein"),
        ),
        (
            "samples/metrohm/synthetic_visionair.csv",
            50,
            Some("protein"),
        ),
        ("samples/viavi_micronir/synthetic_micronir.csv", 20, None),
    ] {
        let records = open_path(workspace_file(relative)).expect("open spectral matrix");

        assert_eq!(records.len(), records_len);
        assert_eq!(records[0].provenance.format, "spectral-matrix");
        assert_eq!(records[0].metadata["sample_id"].as_str(), Some("S000"));
        if let Some(target) = first_target {
            assert!(records[0].targets.contains_key(target));
        }
        let signal = records[0].signals.get("absorbance").expect("absorbance");
        assert_eq!(signal.axis.values.len(), 200);
        assert_eq!(signal.axis.unit, "nm");
        assert_eq!(signal.axis.kind, AxisKind::Wavelength);
        assert_eq!(signal.signal_type, SignalType::Absorbance);
        assert!((signal.axis.values[0] - 1100.0).abs() < 0.000001);
        assert!((signal.axis.values[199] - 2500.0).abs() < 0.000001);
        assert!((signal.values[0] - 0.03674).abs() < 0.00001);
    }
}

#[test]
fn reads_sun_photometer_channel_exports() {
    let records = open_path(workspace_file("samples/mfr/synthetic_mfr.OUT")).expect("open mfr");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "mfr-sun-photometer");
    let channels = records[0].signals.get("channels").expect("channels");
    assert_eq!(
        channels.axis.values,
        vec![415.0, 500.0, 614.0, 673.0, 870.0, 940.0]
    );
    assert_eq!(channels.signal_type, SignalType::RawCounts);
    assert_eq!(channels.values[0], 500.0);
    assert_eq!(channels.values[5], 620.0);

    let records = open_path(workspace_file("samples/microtops/synthetic_microtops.TXT"))
        .expect("open microtops");
    assert_eq!(records.len(), 20);
    assert_eq!(records[0].provenance.format, "microtops-sun-photometer");
    let aot = records[0].signals.get("aot").expect("aot");
    assert_eq!(aot.axis.values, vec![1020.0, 870.0, 675.0]);
    assert_eq!(aot.axis.unit, "nm");
    assert!((aot.values[0] - 0.124).abs() < 0.000001);
    assert!((aot.values[2] - 0.211).abs() < 0.000001);
}

#[test]
fn reads_animl_synthetic_nirs_spectrum() {
    let records =
        open_path(workspace_file("samples/animl/synthetic_nirs.animl")).expect("open animl");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "animl");
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("S001"));
    assert_eq!(records[0].targets["protein"].as_f64(), Some(10.53));

    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 200);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavelength);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((absorbance.axis.values[199] - 2500.0).abs() < 0.000001);
    assert!((absorbance.values[0] - 0.03674).abs() < 0.000001);
    assert!((absorbance.values[199] + 0.14659).abs() < 0.000001);
}

#[test]
fn rejects_non_spectral_animl_result_documents() {
    let err = open_path(workspace_file("samples/animl/Example3.animl"))
        .expect_err("non-spectral AnIML has no axis series");

    assert!(err.to_string().contains("no supported axis series"));
}

#[test]
fn reads_allotrope_asm_spectrum_cubes_and_endpoints() {
    let records = open_path(workspace_file(
        "samples/allotrope_asm/ACSINS_absorbance_spectrum.json",
    ))
    .expect("open ASM absorbance spectrum");

    assert_eq!(records.len(), 360);
    assert_eq!(records[0].provenance.format, "allotrope-asm-json");
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("plate A1"));
    assert_eq!(records[0].metadata["location_id"].as_str(), Some("A1"));
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 51);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavelength);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert_eq!(absorbance.unit.as_deref(), Some("mAU"));
    assert!((absorbance.axis.values[0] - 520.0).abs() < 0.000001);
    assert!((absorbance.axis.values[50] - 570.0).abs() < 0.000001);
    assert!((absorbance.values[0] - 2.672).abs() < 0.000001);

    let records = open_path(workspace_file(
        "samples/allotrope_asm/spectrum_emission_data.json",
    ))
    .expect("open ASM emission spectrum");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].metadata["detection_type"].as_str(),
        Some("Fluorescence")
    );
    assert!(records[0].metadata.contains_key("asm_errors"));
    let emission = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(emission.axis.values, vec![300.0, 310.0, 320.0]);
    assert!((emission.values[0] - 0.123).abs() < 0.000001);

    let records = open_path(workspace_file(
        "samples/allotrope_asm/MD_SMP_absorbance_example.json",
    ))
    .expect("open ASM endpoint absorbance");
    assert_eq!(records.len(), 192);
    assert_eq!(records[0].metadata["group_id"].as_str(), Some("Standards"));
    let endpoint = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(endpoint.axis.values, vec![450.0]);
    assert_eq!(endpoint.unit.as_deref(), Some("mAU"));
    assert!((endpoint.values[0] - 3.41797666666667).abs() < 0.000001);
}

#[test]
fn rejects_target_only_reports_without_spectra() {
    for relative in [
        "samples/foss_winisi/synthetic_ds3_report.csv",
        "samples/perten/synthetic_perten.csv",
    ] {
        let err = open_path(workspace_file(relative)).expect_err("report has no spectrum");
        assert!(err.to_string().contains("unsupported format"));
    }
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
