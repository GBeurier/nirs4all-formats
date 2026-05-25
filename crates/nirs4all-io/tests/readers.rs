use nirs4all_io::{
    open_path, open_path_with_options, AxisKind, AxisOrder, CubeMask, CubeWindow, Error,
    ReadOptions, SignalType,
};
use serde_json::Value;

#[test]
fn reads_asd_fieldspec_revisions() {
    for (
        relative,
        version,
        data_format,
        data_type,
        signal_name,
        signal_type,
        first_value,
        trailing_block_bytes,
        secondary_warning,
    ) in [
        (
            "samples/asd/3L9257.000",
            1,
            "float32",
            "reflectance",
            "reflectance",
            SignalType::Reflectance,
            0.026823,
            0,
            None,
        ),
        (
            "samples/asd/v6sample00000.asd",
            6,
            "float64",
            "raw",
            "raw",
            SignalType::RawCounts,
            29.311738,
            17_274,
            Some("asd_secondary_spectra_not_emitted: reference_spectrum=1"),
        ),
        (
            "samples/asd/v7_field_44231B009.asd",
            7,
            "float64",
            "reflectance",
            "reflectance",
            SignalType::Reflectance,
            18.622284,
            34_523,
            Some("asd_secondary_spectra_not_emitted: reference_spectrum=1, calibration_spectrum=1"),
        ),
        (
            "samples/asd/v7sample00000.asd",
            7,
            "float64",
            "radiance",
            "radiance",
            SignalType::Radiance,
            30.425934,
            68_994,
            Some("asd_secondary_spectra_not_emitted: reference_spectrum=1, calibration_spectrum=3"),
        ),
        (
            "samples/asd/soil.asd",
            8,
            "float64",
            "raw",
            "raw",
            SignalType::RawCounts,
            15.700499,
            17_440,
            Some("asd_secondary_spectra_not_emitted: reference_spectrum=1"),
        ),
        (
            "samples/asd/v8sample00001.asd",
            8,
            "float64",
            "raw",
            "raw",
            SignalType::RawCounts,
            153.995245,
            18_699,
            Some("asd_secondary_spectra_not_emitted: reference_spectrum=1"),
        ),
    ] {
        let records = open_path(workspace_file(relative)).expect("open asd");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].provenance.format, "asd-fieldspec");
        let signal = records[0].signals.get(signal_name).expect(signal_name);
        assert_eq!(signal.axis.values.len(), 2_151);
        assert!((signal.axis.values[0] - 350.0).abs() < 0.000001);
        assert!((signal.axis.values[2_150] - 2500.0).abs() < 0.000001);
        assert_eq!(signal.axis.unit, "nm");
        assert_eq!(signal.axis.kind, AxisKind::Wavelength);
        assert_eq!(signal.signal_type, signal_type);
        assert!((signal.values[0] - first_value).abs() < 0.000001);
        let asd = &records[0].metadata["asd"];
        assert_eq!(asd["version"].as_u64(), Some(version));
        assert_eq!(asd["channels"].as_u64(), Some(2_151));
        assert_eq!(asd["data_format"].as_str(), Some(data_format));
        assert_eq!(asd["data_type"].as_str(), Some(data_type));
        assert_eq!(
            asd["trailing_block_bytes"].as_u64(),
            Some(trailing_block_bytes)
        );
        assert_eq!(
            asd["decoded_trailing_block_bytes"].as_u64(),
            Some(trailing_block_bytes)
        );
        assert_eq!(asd["undecoded_trailing_block_bytes"].as_u64(), Some(0));
        assert!(!records[0]
            .provenance
            .warnings
            .iter()
            .any(|warning| warning.starts_with("trailing_asd_blocks_not_decoded")));
        match secondary_warning {
            Some(expected) => assert!(records[0]
                .provenance
                .warnings
                .iter()
                .any(|warning| warning == expected)),
            None => assert!(records[0].provenance.warnings.is_empty()),
        }
    }
}

#[test]
fn exposes_asd_header_metadata_from_committed_fixtures() {
    let asd = asd_metadata("samples/asd/v7_field_44231B009.asd");

    assert_eq!(asd["program_version"].as_str(), Some("6.4"));
    assert_eq!(asd["file_version"].as_str(), Some("7.0"));
    assert_eq!(
        asd["acquisition_time"]["local"].as_str(),
        Some("2024-10-23T16:58:54")
    );
    assert_eq!(asd["dark_corrected"].as_bool(), Some(true));
    assert_eq!(
        asd["instrument_type"].as_str(),
        Some("fieldspec_full_range")
    );
    assert_eq!(asd["instrument_number"].as_u64(), Some(19_082));
    assert_eq!(asd["integration_time_ms"].as_u64(), Some(17));
    assert_eq!(asd["sample_count"].as_u64(), Some(10));
    assert_eq!(asd["app_data_nonzero_bytes"].as_u64(), Some(30));
    assert!((asd["splice1_wavelength"].as_f64().unwrap() - 1000.0).abs() < 0.000001);
    assert!((asd["splice2_wavelength"].as_f64().unwrap() - 1800.0).abs() < 0.000001);
}

#[test]
fn inventories_asd_secondary_classifier_calibration_and_audit_blocks() {
    let asd = asd_metadata("samples/asd/v7sample00000.asd");
    let blocks = asd["secondary_blocks"].as_array().expect("blocks");
    assert_eq!(count_asd_blocks(blocks, "reference_spectrum"), 1);
    assert_eq!(count_asd_blocks(blocks, "calibration_spectrum"), 3);
    let calibration_header = find_asd_block(blocks, "calibration_header");
    assert_eq!(calibration_header["count"].as_i64(), Some(3));
    let series = calibration_header["series"].as_array().expect("series");
    assert_eq!(series[0]["name"].as_str(), Some("bse63554.ref"));
    assert_eq!(series[1]["calibration_type"].as_str(), Some("lamp"));
    assert_eq!(series[2]["name"].as_str(), Some("ni63554.raw"));

    let asd = asd_metadata("samples/asd/v8sample00001.asd");
    let blocks = asd["secondary_blocks"].as_array().expect("blocks");
    let classifier = find_asd_block(blocks, "classifier_data");
    assert_eq!(classifier["constituent_count"].as_u64(), Some(1));
    assert_eq!(
        classifier["strings"]["display_mode"].as_str(),
        Some("REFLECTANCE")
    );
    assert_eq!(
        classifier["constituents"][0]["name"].as_str(),
        Some("Polystryrene.41D")
    );
    let dependents = find_asd_block(blocks, "dependent_variables");
    assert_eq!(dependents["count"].as_i64(), Some(3));
    assert_eq!(dependents["labels"][0].as_str(), Some("Dep1"));
    assert_eq!(dependents["values"][2].as_f64(), Some(3.0));
    let audit_log = find_asd_block(blocks, "audit_log");
    assert_eq!(audit_log["event_count"].as_i64(), Some(1));
    assert_eq!(
        audit_log["events"][0]["application"].as_str(),
        Some("Indico Pro")
    );
    let signature = find_asd_block(blocks, "signature");
    assert_eq!(signature["signed"].as_str(), Some("signed"));
    assert_eq!(signature["signature_nonzero_bytes"].as_u64(), Some(128));
}

#[test]
fn reads_synthetic_delimited_nirs_table() {
    for relative in [
        "samples/csv_tsv/synthetic_nirs.csv",
        "samples/csv_tsv/synthetic_nirs.tsv",
        "samples/csv_tsv/synthetic_nirs_semicolon.csv",
    ] {
        let records = open_path(workspace_file(relative)).expect("open delimited table");

        assert_eq!(records.len(), 50);
        let first = &records[0];
        let signal = first.signals.get("signal").expect("signal");
        assert_eq!(signal.axis.values.len(), 200);
        assert_eq!(signal.axis.unit, "nm");
        assert_eq!(signal.axis.kind, AxisKind::Wavelength);
        assert_eq!(first.metadata["sample_id"].as_str(), Some("S000"));
        assert!(first.targets.contains_key("protein"));
    }
}

#[test]
fn reads_real_handheld_and_reference_csv_matrices() {
    for (relative, expected_len, sample_id, axis_len, first_axis, last_axis, first_value) in [
        (
            "samples/csv_tsv/auroranir_handheld_barley_sensAIfood.csv",
            86,
            "1",
            351,
            950.0,
            1650.0,
            0.17570436,
        ),
        (
            "samples/foss_winisi/foss_xds_barleyground_sensAIfood.csv",
            7,
            "7693",
            1050,
            400.0,
            2498.0,
            0.249042,
        ),
        (
            "samples/foss_winisi/foss_xds_wheat2_sensAIfood.csv",
            2,
            "11329",
            1050,
            400.0,
            2498.0,
            0.2466762,
        ),
    ] {
        let records = open_path(workspace_file(relative)).expect("open real csv matrix");

        assert_eq!(records.len(), expected_len);
        assert_eq!(records[0].provenance.format, "delimited-text");
        assert_eq!(records[0].metadata["sample_id"].as_str(), Some(sample_id));
        assert!(records[0].targets.contains_key("Protein"));
        let signal = records[0].signals.get("signal").expect("signal");
        assert_eq!(signal.axis.values.len(), axis_len);
        assert_eq!(signal.axis.unit, "nm");
        assert_eq!(signal.axis.kind, AxisKind::Wavelength);
        assert!((signal.axis.values[0] - first_axis).abs() < 0.000001);
        assert!((signal.axis.values[axis_len - 1] - last_axis).abs() < 0.000001);
        assert!((signal.values[0] - first_value).abs() < 0.000001);
    }
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

    let records = open_path(workspace_file("samples/bruker_dpt/RS-1.dpt")).expect("open real dpt");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "bruker-dpt");
    let signal = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(signal.axis.values.len(), 3_439);
    assert_eq!(signal.axis.unit, "cm-1");
    assert_eq!(signal.axis.kind, AxisKind::Wavenumber);
    assert_eq!(signal.axis.order, AxisOrder::Ascending);
    assert_eq!(signal.signal_type, SignalType::Absorbance);
    assert!((signal.axis.values[0] - 800.44463).abs() < 0.000001);
    assert!((signal.axis.values[3_438] - 2_500.135_16).abs() < 0.000001);
    assert!((signal.values[0] - 1.11622).abs() < 0.000001);
    assert!((signal.values[3_438] - 2.20339).abs() < 0.000001);
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
fn reads_bruker_opus_cross_reader_fixture_set() {
    struct BrukerFixtureCheck<'a> {
        path: &'a str,
        signal_count: usize,
        expected_signals: &'a [&'a str],
        primary_signal: &'a str,
        axis_len: usize,
        first_axis: f64,
        last_axis: f64,
        first_value: f64,
        last_value: f64,
        warnings: &'a [&'a str],
    }

    for check in [
        BrukerFixtureCheck {
            path: "samples/bruker_opus/MMP_2107_Test1.001",
            signal_count: 7,
            expected_signals: &[
                "absorbance",
                "match",
                "match_2ch",
                "reference_interferogram",
                "reference_spectrum",
                "sample_interferogram",
                "sample_spectrum",
            ],
            primary_signal: "absorbance",
            axis_len: 1_899,
            first_axis: 11_540.0,
            last_axis: 3_948.0,
            first_value: 0.0714,
            last_value: 0.797975,
            warnings: &["opus_data_block_26_has_no_matching_status_block"],
        },
        BrukerFixtureCheck {
            path: "samples/bruker_opus/brukeropus_file.0",
            signal_count: 6,
            expected_signals: &[
                "absorbance",
                "reference_interferogram",
                "reference_spectrum",
                "sample_interferogram",
                "sample_phase",
                "sample_spectrum",
            ],
            primary_signal: "absorbance",
            axis_len: 4_927,
            first_axis: 9_997.720923,
            last_axis: 499.403996,
            first_value: 0.008769,
            last_value: 0.023399,
            warnings: &[],
        },
        BrukerFixtureCheck {
            path: "samples/bruker_opus/issue82_Opus_test.0",
            signal_count: 5,
            expected_signals: &[
                "absorbance",
                "match",
                "match_2ch",
                "reference_spectrum",
                "sample_spectrum",
            ],
            primary_signal: "absorbance",
            axis_len: 1_112,
            first_axis: 12_488.0,
            last_axis: 3_600.0,
            first_value: 0.998422,
            last_value: 1.991414,
            warnings: &[],
        },
        BrukerFixtureCheck {
            path: "samples/bruker_opus/opusreader_test_spectra.0",
            signal_count: 3,
            expected_signals: &["reference_spectrum", "reflectance", "sample_spectrum"],
            primary_signal: "reflectance",
            axis_len: 4_819,
            first_axis: 7_498.291691,
            last_axis: 599.920607,
            first_value: 0.524343,
            last_value: 0.033849,
            warnings: &["opus_data_block_19_has_no_matching_status_block"],
        },
        BrukerFixtureCheck {
            path: "samples/bruker_opus/scpdata_background.0",
            signal_count: 2,
            expected_signals: &["reference_interferogram", "reference_spectrum"],
            primary_signal: "reference_spectrum",
            axis_len: 4_096,
            first_axis: 5_264.701776,
            last_axis: 0.0,
            first_value: 0.012417,
            last_value: 0.012608,
            warnings: &[],
        },
        BrukerFixtureCheck {
            path: "samples/bruker_opus/scpdata_test.0000",
            signal_count: 6,
            expected_signals: &[
                "absorbance",
                "reference_interferogram",
                "reference_spectrum",
                "sample_interferogram",
                "sample_phase",
                "sample_spectrum",
            ],
            primary_signal: "absorbance",
            axis_len: 2_567,
            first_axis: 3_998.344938,
            last_axis: 699.388954,
            first_value: 0.000459,
            last_value: 0.632425,
            warnings: &[],
        },
    ] {
        let records = open_path(workspace_file(check.path)).expect("open opus fixture");
        assert_eq!(records.len(), 1, "{}", check.path);
        let record = &records[0];
        assert_eq!(record.provenance.format, "bruker-opus", "{}", check.path);
        assert_eq!(record.signals.len(), check.signal_count, "{}", check.path);
        for signal_name in check.expected_signals {
            assert!(
                record.signals.contains_key(*signal_name),
                "{} missing {signal_name}",
                check.path
            );
        }
        let signal = record
            .signals
            .get(check.primary_signal)
            .expect(check.primary_signal);
        assert_eq!(signal.axis.values.len(), check.axis_len, "{}", check.path);
        assert_eq!(signal.axis.unit, "cm-1", "{}", check.path);
        assert_eq!(signal.axis.kind, AxisKind::Wavenumber, "{}", check.path);
        assert_eq!(signal.axis.order, AxisOrder::Descending, "{}", check.path);
        assert!(
            (signal.axis.values[0] - check.first_axis).abs() < 0.000001,
            "{}",
            check.path
        );
        assert!(
            (signal.axis.values[check.axis_len - 1] - check.last_axis).abs() < 0.000001,
            "{}",
            check.path
        );
        assert!(
            (signal.values[0] - check.first_value).abs() < 0.000001,
            "{}",
            check.path
        );
        assert!(
            (signal.values[check.axis_len - 1] - check.last_value).abs() < 0.000001,
            "{}",
            check.path
        );
        let warnings = record
            .provenance
            .warnings
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        assert_eq!(warnings, check.warnings, "{}", check.path);
    }
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
    assert!(record
        .provenance
        .warnings
        .contains(&"nicolet_omnic_srs_tg_gc_reverse_engineered".to_string()));
    assert_eq!(record.metadata["series_variant"].as_str(), Some("tg_gc"));
    assert_eq!(record.metadata["series_y_len"].as_u64(), Some(788));
    assert!(
        (record.metadata["series_y_first_min"].as_f64().unwrap() - 0.025440828874707222).abs()
            < 0.000001
    );
    assert!(
        (record.metadata["series_y_last_min"].as_f64().unwrap() - 20.047266006469727).abs()
            < 0.000001
    );
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
    assert_eq!(transmittance.unit.as_deref(), Some("%"));
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
    assert!(record
        .provenance
        .warnings
        .contains(&"nicolet_omnic_srs_tg_gc_reverse_engineered".to_string()));
    assert_eq!(record.metadata["series_variant"].as_str(), Some("tg_gc"));
    assert_eq!(record.metadata["series_y_len"].as_u64(), Some(335));
    assert!(
        (record.metadata["series_y_first_min"].as_f64().unwrap() - 0.25975024700164795).abs()
            < 0.000001
    );
    assert!(
        (record.metadata["series_y_last_min"].as_f64().unwrap() - 87.01625061035156).abs()
            < 0.000001
    );
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
    assert_eq!(absorbance.dims, vec!["y".to_string(), "x".to_string()]);
    assert_eq!(absorbance.axis.unit, "cm-1");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavenumber);
    assert_eq!(absorbance.axis.order, AxisOrder::Descending);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 3999.706055).abs() < 0.000001);
    assert!((absorbance.axis.values[1_867] - 399.199188).abs() < 0.000001);
    assert!((absorbance.values[0] + 0.007524).abs() < 0.000001);
    assert!((absorbance.values[625_779] - 0.002916).abs() < 0.000001);
    assert!((absorbance.values.iter().sum::<f64>() - 4699.720344).abs() < 0.001);
}

#[test]
fn reads_local_nicolet_omnic_srs_variants_when_present() {
    let tga_demo = workspace_file("samples_local/nicolet_omnic/spectrochempy_TGA_demo.srs");
    let rapid_scan = workspace_file("samples_local/nicolet_omnic/spectrochempy_rapid_scan.srs");
    let reprocessed =
        workspace_file("samples_local/nicolet_omnic/spectrochempy_rapid_scan_reprocessed.srs");
    if !tga_demo.exists() || !rapid_scan.exists() || !reprocessed.exists() {
        eprintln!("skipping local-only OMNIC SRS variant samples");
        return;
    }

    let records = open_path(tga_demo).expect("open local OMNIC TGA demo");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].metadata["series_variant"].as_str(),
        Some("tg_gc")
    );
    assert_eq!(records[0].metadata["series_y_len"].as_u64(), Some(485));
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 3_630);
    assert_eq!(absorbance.values.len(), 1_760_550);
    assert_eq!(absorbance.axis.kind, AxisKind::Wavenumber);
    assert!((absorbance.axis.values[0] - 3999.7041015625).abs() < 0.000001);
    assert!((absorbance.axis.values[3_629] - 500.4451599121094).abs() < 0.000001);

    let records = open_path(rapid_scan).expect("open local OMNIC rapid scan");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].metadata["series_variant"].as_str(),
        Some("rapid_scan_raw")
    );
    assert_eq!(records[0].metadata["series_y_len"].as_u64(), Some(643));
    let detector_signal = records[0]
        .signals
        .get("detector_signal")
        .expect("detector signal");
    assert_eq!(detector_signal.axis.values.len(), 4_160);
    assert_eq!(detector_signal.values.len(), 2_674_880);
    assert_eq!(detector_signal.axis.kind, AxisKind::Index);
    assert_eq!(detector_signal.signal_type, SignalType::Interferogram);
    assert_eq!(detector_signal.unit.as_deref(), Some("V"));
    assert!((detector_signal.values[0] + 0.05086925998330116).abs() < 0.000001);
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"nicolet_omnic_srs_rapid_scan_reverse_engineered".to_string()));

    let records = open_path(reprocessed).expect("open local OMNIC rapid scan reprocessed");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].metadata["series_variant"].as_str(),
        Some("rapid_scan_reprocessed")
    );
    assert_eq!(records[0].metadata["series_y_len"].as_u64(), Some(643));
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 3_734);
    assert_eq!(absorbance.values.len(), 2_400_962);
    assert_eq!(absorbance.axis.kind, AxisKind::Wavenumber);
    assert!((absorbance.values[0] - 0.0832669734954834).abs() < 0.000001);
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
    assert_eq!(
        records[0].metadata["target_property_count"].as_u64(),
        Some(20)
    );
    assert_eq!(
        records[0].metadata["project_file_version"].as_str(),
        Some("2.23")
    );
    assert_eq!(
        records[0].metadata["project_guid"].as_str(),
        Some("56A081B2-5301-40A1-9194-93BB6CCA1C9F")
    );
    assert_eq!(
        records[0].metadata["sample_guid"].as_str(),
        Some("BEA5FE92-4DBA-46DA-93D5-41C4DE159811")
    );
    assert_eq!(records[0].metadata["scans"].as_u64(), Some(32));
    assert_eq!(records[0].metadata["resolution"].as_u64(), Some(8));
    assert_eq!(
        records[0].metadata["declared_wavenumber_count"].as_u64(),
        Some(1_501)
    );
    assert_eq!(
        records[0].metadata["declared_wavenumber_step"].as_f64(),
        Some(4.0)
    );
    assert_eq!(
        records[0].metadata["declared_wavenumber_start"].as_f64(),
        Some(4_000.0)
    );
    assert_eq!(records[0].metadata["device"].as_str(), Some("NIRFlex N500"));
    assert_eq!(
        records[0].metadata["software_version"].as_str(),
        Some("5.6")
    );
    assert_eq!(
        records[0].metadata["created"].as_str(),
        Some("2023/10/10 22:43:58")
    );
    assert_eq!(
        records[0].metadata["modified"].as_str(),
        Some("2023/10/10 22:43:58")
    );
    assert_eq!(
        records[0].metadata["creator"].as_str(),
        Some("Customer System Maintenance")
    );
    assert_eq!(
        records[0].metadata["creator_login"].as_str(),
        Some("Customer System Maintenance")
    );
    assert_eq!(
        records[0].metadata["instrument_serial"].as_str(),
        Some("1000074244")
    );
    assert_eq!(
        records[0].metadata["measurement_cell"].as_str(),
        Some("NIRMaster")
    );
    assert_eq!(
        records[0].metadata["option_serial"].as_str(),
        Some("1000073234")
    );
    assert_eq!(
        records[0].metadata["description"].as_str(),
        Some("Reflectance")
    );
    assert!(!records[0].metadata.contains_key("comment"));
    assert!((records[0].metadata["gain_factor"].as_f64().unwrap() - 18.7143).abs() < 1e-9);
    assert_eq!(records[0].metadata["gain"].as_f64(), Some(3.0));
    assert!(
        (records[0].metadata["instrument_temperature_c"]
            .as_f64()
            .unwrap()
            - 29.812)
            .abs()
            < 1e-9
    );
    assert_eq!(
        records[0].metadata["sample_temperature_c"].as_f64(),
        Some(0.0)
    );
    assert_eq!(
        records[0].metadata["sample_replicate_index"].as_u64(),
        Some(1)
    );
    assert_eq!(
        records[0].metadata["sample_replicate_count"].as_u64(),
        Some(1)
    );
    assert_eq!(records[0].targets.len(), 20);
    let target_keys = records[0].targets.keys().cloned().collect::<Vec<_>>();
    for record in &records {
        assert_eq!(
            record.targets.keys().cloned().collect::<Vec<_>>(),
            target_keys
        );
        assert_eq!(record.targets.len(), 20);
        assert!(record.targets.values().all(|value| value.is_null()));
        assert!(record
            .provenance
            .warnings
            .contains(&"buchi_nircal_zero_property_values_as_missing".to_string()));
    }
    assert!(records[0].targets["K"].is_null());
    assert!(records[0].targets["S_1"].is_null());
    assert!(records[0].targets["S_2"].is_null());
    assert!(records[0].targets["Mn_1"].is_null());
    assert!(records[0].targets["Mn_2"].is_null());
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"buchi_nircal_duplicate_property_names_normalized".to_string()));
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"buchi_nircal_zero_property_values_as_missing".to_string()));
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
fn reads_local_buchi_nircal_non_null_targets_when_present() {
    let path = workspace_file("samples_local/buchi_nircal/transpec_DEMO_cannabis.nir");
    if !path.exists() {
        eprintln!("skipping local-only BUCHI NIRCal cannabis sample");
        return;
    }

    let records = open_path(path).expect("open local BUCHI NIRCal sample");

    assert_eq!(records.len(), 105);
    assert_eq!(records[0].provenance.format, "buchi-nircal");
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("Sample_1"));
    assert_eq!(
        records[0].metadata["target_property_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        records[0].metadata["project_guid"].as_str(),
        Some("986677DA-714F-444E-88C8-EA6C0865C17D")
    );
    assert_eq!(
        records[0].metadata["sample_guid"].as_str(),
        Some("F8A6EBC3-CE55-4942-BAA0-983777C5C285")
    );
    assert_eq!(records[0].metadata["scans"].as_u64(), Some(32));
    assert_eq!(records[0].metadata["resolution"].as_u64(), Some(8));
    assert_eq!(records[0].metadata["device"].as_str(), Some("NIRFlex N500"));
    assert_eq!(
        records[0].metadata["software_version"].as_str(),
        Some("R version 3.6.0 (2019-04-26)")
    );
    assert_eq!(
        records[0].metadata["created"].as_str(),
        Some("2018/10/29 14:31:16")
    );
    assert_eq!(
        records[0].metadata["modified"].as_str(),
        Some("2019/07/17 15:27:11")
    );
    assert_eq!(records[0].metadata["creator"].as_str(), Some("BUCHI"));
    assert_eq!(
        records[0].metadata["creator_login"].as_str(),
        Some("Exported from R")
    );
    assert_eq!(
        records[0].metadata["modified_by"].as_str(),
        Some("Administrator")
    );
    assert_eq!(
        records[0].metadata["modifier_login"].as_str(),
        Some("Administrator")
    );
    assert_eq!(
        records[0].metadata["instrument_serial"].as_str(),
        Some("1000283339")
    );
    assert_eq!(
        records[0].metadata["measurement_cell"].as_str(),
        Some("Solids, XL")
    );
    assert_eq!(
        records[0].metadata["option_serial"].as_str(),
        Some("1000288264")
    );
    assert_eq!(records[0].metadata["comment"].as_str(), Some("DEMO"));
    assert_eq!(
        records[0].metadata["description"].as_str(),
        Some("Reflectance")
    );
    assert!(!records[0].metadata.contains_key("gain_factor"));
    assert!(!records[0].metadata.contains_key("instrument_temperature_c"));
    assert_eq!(
        records[0].metadata["sample_replicate_index"].as_u64(),
        Some(1)
    );
    assert_eq!(
        records[0].metadata["sample_replicate_count"].as_u64(),
        Some(3)
    );
    assert_eq!(
        records[2].metadata["sample_replicate_index"].as_u64(),
        Some(3)
    );
    assert_eq!(
        records[3].metadata["sample_replicate_index"].as_u64(),
        Some(1)
    );
    assert!(!records[0]
        .provenance
        .warnings
        .contains(&"buchi_nircal_zero_property_values_as_missing".to_string()));
    assert!((records[0].targets["CBDA"].as_f64().expect("CBDA") - 5.958436124).abs() < 1e-9);
    assert!((records[0].targets["THCA"].as_f64().expect("THCA") - 0.174373006).abs() < 1e-9);
    assert!((records[104].targets["CBDA"].as_f64().expect("CBDA") - 14.35).abs() < 1e-9);
    assert!((records[104].targets["THCA"].as_f64().expect("THCA") - 0.6).abs() < 1e-9);
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 1_501);
    assert_eq!(absorbance.axis.unit, "cm-1");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavenumber);
    assert!((absorbance.values[0] - 0.16346853997656646).abs() < 1e-12);
    assert!((absorbance.values.iter().sum::<f64>() - 702.4047647373825).abs() < 1e-9);
}

#[test]
fn reads_jasco_jws_single_channel_files() {
    let records = open_path(workspace_file("samples/jasco/243.jws")).expect("open jws");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "jasco-jws");
    assert_eq!(record.metadata["channel_count"].as_u64(), Some(1));
    assert_eq!(record.metadata["point_count"].as_u64(), Some(7_729));
    assert_eq!(
        record.metadata["channel_labels"]
            .as_array()
            .expect("channel labels")[0]
            .as_str(),
        Some("transmittance")
    );
    assert_eq!(
        record.metadata["instrument_model"].as_str(),
        Some("FT/IR-4100typeA")
    );
    assert_eq!(
        record.metadata["measurement_mode"].as_str(),
        Some("ftir_transmittance")
    );
    assert_eq!(
        record.metadata["source_path"].as_str(),
        Some(r"Z:\Instruments\IR\YCD\243.jws")
    );
    assert!(record
        .provenance
        .warnings
        .contains(&"jasco_jws_semantic_channels_inferred".to_string()));
    let signal = record.signals.get("transmittance").expect("transmittance");
    assert_eq!(signal.axis.values.len(), 7_729);
    assert_eq!(signal.axis.unit, "cm-1");
    assert_eq!(signal.axis.kind, AxisKind::Wavenumber);
    assert_eq!(signal.axis.order, AxisOrder::Ascending);
    assert_eq!(signal.signal_type, SignalType::Transmittance);
    assert_eq!(signal.unit.as_deref(), Some("%T"));
    assert!((signal.axis.values[0] - 349.0525166555562).abs() < 0.000001);
    assert!((signal.axis.values[7_728] - 7800.6487838216835).abs() < 0.000001);
    assert!((signal.values[0] - 38.420169830322266).abs() < 0.000001);
    assert!((signal.values[7_728] - 35.47404479980469).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 316_675.31128692627).abs() < 0.000001);

    let records =
        open_path(workspace_file("samples/jasco/sample_fluorescence.jws")).expect("open jws");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].metadata["channel_labels"]
            .as_array()
            .expect("channel labels")[0]
            .as_str(),
        Some("fluorescence")
    );
    assert_eq!(
        records[0].metadata["instrument_model"].as_str(),
        Some("FP-8300")
    );
    assert_eq!(
        records[0].metadata["measurement_mode"].as_str(),
        Some("fluorescence")
    );
    assert_eq!(
        records[0].metadata["sample_label"].as_str(),
        Some("photonic wire")
    );
    let signal = records[0]
        .signals
        .get("fluorescence")
        .expect("fluorescence");
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
    let channel_labels = record.metadata["channel_labels"]
        .as_array()
        .expect("channel labels")
        .iter()
        .map(|value| value.as_str().expect("channel label"))
        .collect::<Vec<_>>();
    assert_eq!(channel_labels, vec!["cd", "ht", "absorbance"]);
    assert_eq!(
        record.metadata["instrument_model"].as_str(),
        Some("CD-1500")
    );
    assert_eq!(
        record.metadata["measurement_mode"].as_str(),
        Some("circular_dichroism")
    );
    assert_eq!(
        record.metadata["source_path"].as_str(),
        Some(r"F:\CD1500\1A-1.jws")
    );
    assert_eq!(record.signal_type, SignalType::Unknown);

    let cd = record.signals.get("cd").expect("cd");
    assert_eq!(cd.axis.values.len(), 1_501);
    assert_eq!(cd.axis.unit, "nm");
    assert_eq!(cd.axis.kind, AxisKind::Wavelength);
    assert_eq!(cd.axis.order, AxisOrder::Descending);
    assert_eq!(cd.signal_type, SignalType::Unknown);
    assert_eq!(cd.unit.as_deref(), Some("mdeg"));
    assert!((cd.axis.values[0] - 350.0).abs() < 0.000001);
    assert!((cd.axis.values[1_500] - 200.0).abs() < 0.000001);
    assert!((cd.values[0] - 0.3416369557380676).abs() < 0.000001);
    assert!((cd.values[1_500] - 6.220218658447266).abs() < 0.000001);
    assert!((cd.values.iter().sum::<f64>() - 3706.048405816895).abs() < 0.000001);

    let ht = record.signals.get("ht").expect("ht");
    assert_eq!(ht.unit.as_deref(), Some("V"));
    assert!((ht.values[0] - 250.94847106933594).abs() < 0.000001);
    assert!((ht.values[1_500] - 364.5225830078125).abs() < 0.000001);
    assert!((ht.values.iter().sum::<f64>() - 401_403.0902252197).abs() < 0.000001);

    let absorbance = record.signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert_eq!(absorbance.unit.as_deref(), Some("dOD"));
    assert!((absorbance.values[0] - 0.7128385901451111).abs() < 0.000001);
    assert!((absorbance.values[1_500] - 1.899193286895752).abs() < 0.000001);
    assert!((absorbance.values.iter().sum::<f64>() - 1356.2173843979836).abs() < 0.000001);
}

#[test]
fn reads_horiba_jobinyvon_xml_exports() {
    let records = open_path(workspace_file(
        "samples/raman_horiba/jobinyvon_test_spec.xml",
    ))
    .expect("open xml");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "horiba-jobinyvon-xml");
    assert_eq!(record.metadata["dataset_type"].as_str(), Some("Spectrum"));
    assert_eq!(
        record.metadata["instrument"].as_str(),
        Some("LabRAM HR Evol")
    );
    let signal = record.signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 34);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.axis.order, AxisOrder::Descending);
    assert_eq!(signal.signal_type, SignalType::RawCounts);
    assert_eq!(signal.unit.as_deref(), Some("Cnt/sec"));
    assert!((signal.axis.values[0] - 537.361).abs() < 0.000001);
    assert!((signal.axis.values[33] - 522.574).abs() < 0.000001);
    assert!((signal.values[0] - 1496.0).abs() < 0.000001);
    assert!((signal.values[33] - 760.0).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 28624.0).abs() < 0.000001);

    let records = open_path(workspace_file(
        "samples/raman_horiba/jobinyvon_test_map_x3-y2.xml",
    ))
    .expect("open map xml");
    assert_eq!(records.len(), 6);
    assert_eq!(records[0].metadata["dataset_type"].as_str(), Some("SpIm"));
    assert_eq!(records[0].metadata["spatial_x"].as_f64(), Some(-2.0));
    assert_eq!(records[0].metadata["spatial_y"].as_f64(), Some(-1.0));
    assert_eq!(records[5].metadata["spatial_x"].as_f64(), Some(2.0));
    assert_eq!(records[5].metadata["spatial_y"].as_f64(), Some(1.0));
    let first = records[0].signals.get("intensity").expect("intensity");
    assert!((first.values[0] - 275.5).abs() < 0.000001);
    let all_sum = records
        .iter()
        .map(|record| record.signals["intensity"].values.iter().sum::<f64>())
        .sum::<f64>();
    assert!((all_sum - 30224.0).abs() < 0.000001);

    let records = open_path(workspace_file(
        "samples/raman_horiba/jobinyvon_test_linescan.xml",
    ))
    .expect("open linescan xml");
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].metadata["dataset_type"].as_str(), Some("SpIm"));
    assert_eq!(records[0].metadata["spatial_x"].as_f64(), Some(0.0));
    assert_eq!(records[0].metadata["spatial_y"].as_f64(), Some(0.0));
    assert_eq!(records[1].metadata["spatial_y"].as_f64(), Some(0.5));
    assert_eq!(records[2].metadata["spatial_y"].as_f64(), Some(1.0));
    assert_eq!(records[0].metadata["spatial_x_unit"].as_str(), Some("um"));
    assert_eq!(records[0].metadata["spatial_y_unit"].as_str(), Some("um"));
    let first = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(first.axis.values.len(), 34);
    assert_eq!(first.axis.unit, "nm");
    assert_eq!(first.axis.kind, AxisKind::Wavelength);
    assert!((first.values[0] - 1614.0).abs() < 0.000001);
    assert!((first.values.iter().sum::<f64>() - 29666.0).abs() < 0.000001);

    let records = open_path(workspace_file(
        "samples/raman_horiba/jobinyvon_test_spec_range.xml",
    ))
    .expect("open range xml");
    assert_eq!(records.len(), 1);
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 105);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.axis.order, AxisOrder::Ascending);
    assert!((signal.axis.values[0] - 720.924).abs() < 0.000001);
    assert!((signal.axis.values[104] - 879.318).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 25303.558).abs() < 0.000001);

    let records = open_path(workspace_file(
        "samples/raman_horiba/jobinyvon_test_spec_3s_eV.xml",
    ))
    .expect("open eV xml");
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.unit, "eV");
    assert_eq!(signal.axis.kind, AxisKind::Energy);
    assert!(!records[0]
        .provenance
        .warnings
        .contains(&"horiba_unsupported_axis_kind_energy".to_string()));
}

#[test]
fn reads_horiba_labspec_text_exports() {
    let records =
        open_path(workspace_file("samples/raman_horiba/labspec_532nm_Si.txt")).expect("open text");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "horiba-labspec-text");
    assert_eq!(record.metadata["axis_layout"].as_str(), Some("two_column"));
    let signal = record.signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 1024);
    assert_eq!(signal.axis.unit, "cm-1");
    assert_eq!(signal.axis.kind, AxisKind::Wavenumber);
    assert!((signal.axis.values[0] - 46.6417).abs() < 0.000001);
    assert!((signal.axis.values[1023] - 1754.52).abs() < 0.000001);
    assert!((signal.values[0] - 37.0).abs() < 0.000001);
    assert!((signal.values[1023] - 19.0).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 127584.0).abs() < 0.000001);
    assert!(record
        .provenance
        .warnings
        .contains(&"horiba_labspec_text_axis_unit_inferred".to_string()));

    let records = open_path(workspace_file(
        "samples/raman_horiba/labspec_lasertest1.txt",
    ))
    .expect("open series text");
    assert_eq!(records.len(), 3);
    assert_eq!(
        records[0].metadata["axis_layout"].as_str(),
        Some("series_rows")
    );
    assert_eq!(records[0].metadata["point_index"].as_f64(), Some(1.0));
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 1024);
    assert!((signal.values[0] + 4.05818).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 31010.572498).abs() < 0.000001);

    let records = open_path(workspace_file(
        "samples/raman_horiba/labspec6_Gd2O3_AlN_map.txt",
    ))
    .expect("open map text");
    assert_eq!(records.len(), 72);
    assert_eq!(
        records[0].metadata["axis_layout"].as_str(),
        Some("map_rows")
    );
    assert_eq!(records[0].metadata["spatial_x"].as_f64(), Some(-209.871));
    assert_eq!(records[0].metadata["spatial_y"].as_f64(), Some(-204.081));
    assert_eq!(records[71].metadata["spatial_x"].as_f64(), Some(183.819));
    assert_eq!(records[71].metadata["spatial_y"].as_f64(), Some(204.317));
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 498);
    assert_eq!(signal.axis.unit, "cm-1");
    assert_eq!(signal.unit.as_deref(), Some("Cnt"));
    assert!((signal.values.iter().sum::<f64>() - 72757.0).abs() < 0.000001);
}

#[test]
fn reads_horiba_labspec6_binary_map() {
    let records = open_path(workspace_file("samples/raman_horiba/AlN_Gd2O3_indepth.l6m"))
        .expect("open labspec6 binary");
    let text_records = open_path(workspace_file(
        "samples/raman_horiba/labspec6_Gd2O3_AlN_map.txt",
    ))
    .expect("open paired labspec6 text export");

    assert_eq!(records.len(), 72);
    assert_eq!(text_records.len(), records.len());
    assert_eq!(records[0].provenance.format, "horiba-labspec6-binary");
    assert_eq!(
        records[0].metadata["axis_layout"].as_str(),
        Some("labspec6_binary_map")
    );
    assert_eq!(
        records[0].metadata["spatial_axis_order"].as_str(),
        Some("x_slowest_y_fastest")
    );
    assert!((records[0].metadata["spatial_x"].as_f64().expect("x") + 209.87088).abs() < 0.00001);
    assert!((records[0].metadata["spatial_y"].as_f64().expect("y") + 204.08078).abs() < 0.00001);
    assert!((records[71].metadata["spatial_x"].as_f64().expect("x") - 183.81874).abs() < 0.00001);
    assert!((records[71].metadata["spatial_y"].as_f64().expect("y") - 204.31718).abs() < 0.00001);
    assert!((records[1].metadata["spatial_x"].as_f64().expect("x") + 209.87088).abs() < 0.00001);
    assert!((records[1].metadata["spatial_y"].as_f64().expect("y") + 166.9537).abs() < 0.0001);
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"horiba_labspec6_binary_experimental".to_string()));
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 498);
    assert_eq!(signal.axis.unit, "cm-1");
    assert_eq!(signal.axis.kind, AxisKind::Wavenumber);
    assert_eq!(signal.unit.as_deref(), Some("Cnt"));
    assert!((signal.axis.values[0] - 100.166382).abs() < 0.000001);
    assert!((signal.axis.values[497] - 1198.541748).abs() < 0.000001);
    assert!((signal.values[0] - 57.0).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 72757.0).abs() < 0.000001);

    for (binary, text) in records.iter().zip(&text_records) {
        assert!(
            (binary.metadata["spatial_x"].as_f64().expect("binary x")
                - text.metadata["spatial_x"].as_f64().expect("text x"))
            .abs()
                < 0.001
        );
        assert!(
            (binary.metadata["spatial_y"].as_f64().expect("binary y")
                - text.metadata["spatial_y"].as_f64().expect("text y"))
            .abs()
                < 0.001
        );
        let binary_signal = binary.signals.get("intensity").expect("binary intensity");
        let text_signal = text.signals.get("intensity").expect("text intensity");
        assert_eq!(binary_signal.values, text_signal.values);
        assert_eq!(
            binary_signal.axis.values.len(),
            text_signal.axis.values.len()
        );
        for (binary_axis, text_axis) in binary_signal
            .axis
            .values
            .iter()
            .zip(&text_signal.axis.values)
        {
            assert!((binary_axis - text_axis).abs() < 0.005);
        }
    }
}

#[test]
fn reads_renishaw_wdf_single_spectra() {
    let records = open_path(workspace_file(
        "samples/raman_renishaw/renishaw_test_spectrum.wdf",
    ))
    .expect("open wdf");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "renishaw-wdf");
    assert_eq!(record.metadata["application_name"].as_str(), Some("WiRE"));
    assert_eq!(
        record.metadata["title"].as_str(),
        Some("Single scan measurement 7")
    );
    assert!(!record.metadata.contains_key("map_analysis_values"));
    assert!(record
        .provenance
        .warnings
        .contains(&"renishaw_wdf_reverse_engineered_chunks".to_string()));
    let signal = record.signals.get("raw_counts").expect("raw counts");
    assert_eq!(signal.axis.values.len(), 36);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.axis.order, AxisOrder::Descending);
    assert_eq!(signal.signal_type, SignalType::RawCounts);
    assert_eq!(signal.unit.as_deref(), Some("counts"));
    assert!((signal.axis.values[0] - 328.98077392578125).abs() < 0.000001);
    assert!((signal.axis.values[35] - 326.0163269042969).abs() < 0.000001);
    assert!((signal.values[0] - 68.10285186767578).abs() < 0.000001);
    assert!((signal.values[35] - 65.36617279052734).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 2606.160828).abs() < 0.000001);

    let records =
        open_path(workspace_file("samples/raman_renishaw/wire_sp.wdf")).expect("open wire wdf");
    let signal = records[0].signals.get("raw_counts").expect("raw counts");
    assert_eq!(signal.axis.values.len(), 1015);
    assert_eq!(signal.axis.unit, "cm-1");
    assert_eq!(signal.axis.kind, AxisKind::Wavenumber);
    assert_eq!(signal.axis.order, AxisOrder::Descending);
    assert!((signal.axis.values[0] - 2787.514404296875).abs() < 0.000001);
    assert!((signal.axis.values[1014] - 1226.2752685546875).abs() < 0.000001);
    assert!((signal.values[0] - 47.092708587646484).abs() < 0.000001);
    assert!((signal.values[1014] - 21.815458297729492).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 107421.227566).abs() < 0.000001);
}

#[test]
fn reads_renishaw_wdf_multi_spectrum_payloads() {
    let records = open_path(workspace_file(
        "samples/raman_renishaw/renishaw_test_linescan.wdf",
    ))
    .expect("open WDF linescan");
    assert_eq!(records.len(), 5);
    assert_eq!(
        records[0].metadata["measurement_type_label"].as_str(),
        Some("mapping")
    );
    assert_eq!(
        records[0].metadata["map_type_label"].as_str(),
        Some("xyline")
    );
    assert_eq!(records[0].metadata["map_width"].as_u64(), Some(5));
    assert_eq!(records[0].metadata["spatial_x"].as_f64(), Some(-50.0));
    assert_eq!(records[0].metadata["spatial_y"].as_f64(), Some(-50.0));
    assert_eq!(records[0].metadata["spatial_x_unit"].as_str(), Some("um"));
    let white_light = &records[0].metadata["white_light_image"];
    assert_eq!(white_light["format"].as_str(), Some("jpeg"));
    assert_eq!(white_light["mime_type"].as_str(), Some("image/jpeg"));
    assert_eq!(white_light["width_px"].as_u64(), Some(752));
    assert_eq!(white_light["height_px"].as_u64(), Some(480));
    assert_eq!(white_light["precision_bits"].as_u64(), Some(8));
    assert_eq!(white_light["components"].as_u64(), Some(3));
    assert_eq!(white_light["jfif_x_density"].as_u64(), Some(96));
    assert_eq!(white_light["jfif_y_density"].as_u64(), Some(96));
    assert_eq!(
        white_light["exif_description"].as_str(),
        Some("white-light image")
    );
    assert_eq!(white_light["exif_make"].as_str(), Some("Renishaw"));
    assert_eq!(white_light["byte_len"].as_u64(), Some(8797));
    assert_eq!(records[0].metadata["map_x_index"].as_u64(), Some(0));
    assert_eq!(records[0].metadata["map_y_index"].as_u64(), Some(0));
    assert!((records[0].metadata["spatial_distance"].as_f64().unwrap() - 0.0).abs() < 0.000001);
    assert_eq!(records[4].metadata["spectrum_index"].as_u64(), Some(4));
    assert_eq!(records[4].metadata["map_x_index"].as_u64(), Some(4));
    assert!(
        (records[4].metadata["spatial_x"].as_f64().unwrap() - 34.85281374238569).abs() < 0.000001
    );
    assert!((records[4].metadata["spatial_distance"].as_f64().unwrap() - 120.0).abs() < 0.00001);
    assert!(!records[0]
        .provenance
        .warnings
        .contains(&"renishaw_wdf_navigation_axes_pending".to_string()));
    let signal = records[0].signals.get("raw_counts").expect("raw counts");
    assert_eq!(signal.axis.values.len(), 40);
    assert_eq!(signal.axis.unit, "nm");
    assert!((signal.axis.values[0] - 364.6417541503906).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 26.666167).abs() < 0.000001);

    let records = open_path(workspace_file(
        "samples/raman_renishaw/interrupted_acquisition.wdf",
    ))
    .expect("open interrupted WDF");
    assert_eq!(records.len(), 12);
    assert_eq!(records[0].metadata["map_width"].as_u64(), Some(4));
    assert_eq!(records[11].metadata["map_x_index"].as_u64(), Some(3));
    assert_eq!(records[11].metadata["map_y_index"].as_u64(), Some(2));
    assert!(
        (records[0].metadata["spatial_x"].as_f64().unwrap() - 9250.073496942934).abs() < 0.000001
    );
    assert!(
        (records[11].metadata["spatial_y"].as_f64().unwrap() - 3354.234361049107).abs() < 0.000001
    );
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"renishaw_wdf_interrupted_acquisition_truncated_to_count".to_string()));
    let white_light = &records[0].metadata["white_light_image"];
    assert_eq!(white_light["width_px"].as_u64(), Some(479));
    assert_eq!(white_light["height_px"].as_u64(), Some(445));
    let signal = records[0].signals.get("raw_counts").expect("raw counts");
    assert_eq!(signal.axis.values.len(), 1010);
    assert_eq!(signal.axis.unit, "cm-1");
    assert!((signal.values[0] - 73.42675018310547).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 168272.582141).abs() < 0.000001);

    let records = open_path(workspace_file(
        "samples/raman_renishaw/renishaw_test_map.wdf",
    ))
    .expect("open WDF map");
    assert_eq!(records.len(), 9);
    assert_eq!(
        records[0].metadata["map_type_label"].as_str(),
        Some("unspecified")
    );
    assert_eq!(records[0].metadata["map_width"].as_u64(), Some(3));
    assert_eq!(records[0].metadata["map_height"].as_u64(), Some(3));
    assert_eq!(records[0].metadata["spatial_x"].as_f64(), Some(-100.0));
    assert_eq!(records[0].metadata["spatial_y"].as_f64(), Some(-100.0));
    assert_eq!(records[8].metadata["spatial_x"].as_f64(), Some(100.0));
    assert_eq!(records[8].metadata["spatial_y"].as_f64(), Some(100.0));
    assert_eq!(records[8].metadata["map_x_index"].as_u64(), Some(2));
    assert_eq!(records[8].metadata["map_y_index"].as_u64(), Some(2));

    let records = open_path(workspace_file(
        "samples/raman_renishaw/renishaw_test_map2.wdf",
    ))
    .expect("open WDF map2");
    assert_eq!(records.len(), 400);
    assert_eq!(
        records[0].metadata["map_analysis_block_count"].as_u64(),
        Some(2)
    );
    let map_blocks = records[0].metadata["map_analysis_blocks"]
        .as_array()
        .expect("map analysis blocks");
    assert_eq!(map_blocks[0]["payload_kind"].as_str(), Some("pset"));
    assert_eq!(map_blocks[0]["pset_declared_len"].as_u64(), Some(272));
    assert!(map_blocks[0]["ascii_preview"]
        .as_array()
        .expect("ascii preview")
        .iter()
        .any(|value| value.as_str() == Some("Intensity At Point 357u")));
    assert_eq!(
        map_blocks[0]["data_range_encoding"].as_str(),
        Some("f32le_tail_after_pset")
    );
    assert_eq!(
        map_blocks[0]["data_range_indexed_by"].as_str(),
        Some("spectrum_index")
    );
    assert_eq!(map_blocks[0]["data_range_value_count"].as_u64(), Some(400));
    assert_eq!(map_blocks[1]["data_range_value_count"].as_u64(), Some(400));
    let first_map_values = records[0].metadata["map_analysis_values"]
        .as_array()
        .expect("map analysis values");
    assert_eq!(first_map_values.len(), 2);
    assert!(first_map_values[0]["label"]
        .as_str()
        .is_some_and(|label| label.contains("Intensity At Point 357u")));
    assert!(
        (first_map_values[0]["value"].as_f64().expect("map value") - 66.674965).abs() < 0.000001
    );
    assert!(
        (first_map_values[1]["value"].as_f64().expect("map value") - 53.033577).abs() < 0.000001
    );
    let last_map_values = records[399].metadata["map_analysis_values"]
        .as_array()
        .expect("map analysis values");
    assert!(
        (last_map_values[0]["value"].as_f64().expect("map value") - 431.030426).abs() < 0.000001
    );
    assert!(
        (last_map_values[1]["value"].as_f64().expect("map value") - 261.486084).abs() < 0.000001
    );

    let records =
        open_path(workspace_file("samples/raman_renishaw/wire_depth.wdf")).expect("open WDF depth");
    assert_eq!(records.len(), 40);
    assert_eq!(records[0].metadata["spatial_z"].as_f64(), Some(-10.0));
    assert_eq!(records[39].metadata["spatial_z"].as_f64(), Some(9.5));
    assert_eq!(
        records[0].metadata["elapsed_time_seconds"].as_f64(),
        Some(0.0)
    );
    let map_blocks = records[0].metadata["map_analysis_blocks"]
        .as_array()
        .expect("map analysis blocks");
    assert_eq!(map_blocks.len(), 2);
    assert!(map_blocks[0]["ascii_preview"]
        .as_array()
        .expect("ascii preview")
        .iter()
        .any(|value| value
            .as_str()
            .is_some_and(|text| text.contains("Signal To Baseline from 1550.00"))));
    assert_eq!(map_blocks[0]["data_range_value_count"].as_u64(), Some(40));
    assert_eq!(map_blocks[1]["data_range_value_count"].as_u64(), Some(40));
    let second_depth_values = records[1].metadata["map_analysis_values"]
        .as_array()
        .expect("map analysis values");
    assert!(second_depth_values[0]["label"]
        .as_str()
        .is_some_and(|label| label.contains("Signal To Baseline from 1550.00")));
    assert!(
        (second_depth_values[0]["value"]
            .as_f64()
            .expect("depth value")
            - 332.154236)
            .abs()
            < 0.000001
    );
    assert!(
        (second_depth_values[1]["value"]
            .as_f64()
            .expect("depth value")
            - 1392.108521)
            .abs()
            < 0.000001
    );
    let last_depth_values = records[39].metadata["map_analysis_values"]
        .as_array()
        .expect("map analysis values");
    assert!(
        (last_depth_values[0]["value"].as_f64().expect("depth value") - 640.086609).abs()
            < 0.000001
    );
    assert!(
        (last_depth_values[1]["value"].as_f64().expect("depth value") - 1232.547852).abs()
            < 0.000001
    );

    let records = open_path(workspace_file(
        "samples/raman_renishaw/renishaw_test_focustrack.wdf",
    ))
    .expect("open WDF focustrack");
    assert_eq!(records.len(), 3);
    assert!(
        (records[0].metadata["focus_track_z"].as_f64().unwrap() - 31.599992786938856).abs()
            < 0.000001
    );
    assert_eq!(
        records[0].metadata["focus_track_z_unit"].as_str(),
        Some("um")
    );
}

#[test]
fn opens_supported_renishaw_wdf_acquisition_counts() {
    for (relative, expected_count) in [
        ("samples/raman_renishaw/interrupted_acquisition.wdf", 12),
        ("samples/raman_renishaw/renishaw_test_exptime10_acc1.wdf", 1),
        ("samples/raman_renishaw/renishaw_test_focustrack.wdf", 3),
        (
            "samples/raman_renishaw/renishaw_test_focustrack_invariant.wdf",
            10,
        ),
        ("samples/raman_renishaw/renishaw_test_linescan.wdf", 5),
        ("samples/raman_renishaw/renishaw_test_map.wdf", 9),
        ("samples/raman_renishaw/renishaw_test_map2.wdf", 400),
        ("samples/raman_renishaw/renishaw_test_spectrum.wdf", 1),
        ("samples/raman_renishaw/renishaw_test_streamline.wdf", 2205),
        ("samples/raman_renishaw/renishaw_test_timeseries.wdf", 3),
        ("samples/raman_renishaw/renishaw_test_zscan.wdf", 40),
        ("samples/raman_renishaw/wire_Streamline.wdf", 2205),
        ("samples/raman_renishaw/wire_depth.wdf", 40),
        ("samples/raman_renishaw/wire_line.wdf", 235),
        ("samples/raman_renishaw/wire_sp.wdf", 1),
    ] {
        let records = open_path(workspace_file(relative)).expect("open supported WDF");
        assert_eq!(records.len(), expected_count, "{relative}");
        assert!(records[0].signals.contains_key("raw_counts"), "{relative}");
    }
}

#[test]
fn reads_additional_renishaw_wdf_navigation_modes() {
    let records = open_path(workspace_file(
        "samples/raman_renishaw/renishaw_test_timeseries.wdf",
    ))
    .expect("open WDF time series");
    assert_eq!(records.len(), 3);
    assert_eq!(
        records[0].metadata["elapsed_time_seconds"].as_f64(),
        Some(0.0)
    );
    assert!(
        (records[1].metadata["elapsed_time_seconds"]
            .as_f64()
            .unwrap()
            - 3.0884593)
            .abs()
            < 0.000001
    );
    assert!(
        (records[2].metadata["elapsed_time_seconds"]
            .as_f64()
            .unwrap()
            - 6.133009)
            .abs()
            < 0.000001
    );

    let records = open_path(workspace_file(
        "samples/raman_renishaw/renishaw_test_zscan.wdf",
    ))
    .expect("open WDF z scan");
    assert_eq!(records.len(), 40);
    assert_eq!(records[0].metadata["spatial_z"].as_f64(), Some(-10.0));
    assert_eq!(records[39].metadata["spatial_z"].as_f64(), Some(9.5));
    assert!(
        (records[39].metadata["elapsed_time_seconds"]
            .as_f64()
            .unwrap()
            - 40.5323183)
            .abs()
            < 0.000001
    );

    let records =
        open_path(workspace_file("samples/raman_renishaw/wire_line.wdf")).expect("open wire line");
    assert_eq!(records.len(), 235);
    assert_eq!(
        records[0].metadata["map_type_label"].as_str(),
        Some("xyline")
    );
    assert_eq!(records[0].metadata["map_width"].as_u64(), Some(235));
    assert_eq!(records[234].metadata["map_x_index"].as_u64(), Some(234));
    assert!(
        (records[234].metadata["spatial_distance"].as_f64().unwrap() - 58.499996748737075).abs()
            < 0.000001
    );
    assert!(
        (records[234].metadata["spatial_x"].as_f64().unwrap() - 134.21802758544098).abs()
            < 0.000001
    );
    assert!(
        (records[234].metadata["spatial_y"].as_f64().unwrap() + 140.44237383913503).abs()
            < 0.000001
    );
}

#[test]
fn rejects_renishaw_wdf_undefined_modes_for_now() {
    for relative in [
        "samples/raman_renishaw/renishaw_test_undefined.wdf",
        "samples/raman_renishaw/wire_undefined.wdf",
    ] {
        let err = open_path(workspace_file(relative)).expect_err("undefined WDF should fail");
        assert!(err.to_string().contains("undefined measurement_type=0"));
    }
}

#[test]
fn reads_trivista_tvf_modes() {
    let records = open_path(workspace_file(
        "samples/raman_trivista/spec_1s_1acc_1frame_average.tvf",
    ))
    .expect("open TriVista single spectrum");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "trivista-tvf");
    assert_eq!(
        records[0].metadata["document_role"].as_str(),
        Some("primary")
    );
    assert_eq!(
        records[0].metadata["record_time"].as_str(),
        Some("06/14/2022 13:34:27.453")
    );
    assert_eq!(records[0].metadata["xdim_length"].as_u64(), Some(1024));
    assert_eq!(
        records[0].metadata["spectral_axis_label"].as_str(),
        Some("Wavelength")
    );
    assert_eq!(
        records[0].metadata["spectral_axis_unit"].as_str(),
        Some("nm")
    );
    assert_eq!(
        records[0].metadata["spectral_axis_display_unit"].as_str(),
        Some("Nanometer")
    );
    assert_eq!(
        records[0].metadata["spectral_axis_calibration_type"].as_str(),
        Some("ValueArray")
    );
    assert_eq!(
        records[0].metadata["spectral_axis_laser_wave"].as_f64(),
        Some(0.0)
    );
    assert_eq!(
        records[0].metadata["detector_name"].as_str(),
        Some("Camera1")
    );
    assert_eq!(
        records[0].metadata["detector_size"].as_str(),
        Some("1024;1")
    );
    assert_eq!(
        records[0].metadata["detector_adc_readout_port"].as_str(),
        Some("Normal")
    );
    assert_eq!(
        records[0].metadata["detector_adc_rate_resolution"].as_str(),
        Some("1 MHz")
    );
    assert_eq!(records[0].metadata["detector_adc_gain"].as_f64(), Some(2.0));
    assert_eq!(
        records[0].metadata["detector_temperature_c"].as_f64(),
        Some(-25.0)
    );
    assert!(!records[0].metadata.contains_key("time_index"));
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 1024);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.signal_type, SignalType::RawCounts);
    assert_eq!(signal.unit.as_deref(), Some("counts"));
    assert!((signal.axis.values[0] - 794.220731002166).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 28479097.0).abs() < 0.000001);

    let records = open_path(workspace_file("samples/raman_trivista/linescan.tvf"))
        .expect("open TriVista linescan");
    assert_eq!(records.len(), 21);
    assert_eq!(
        records[0].metadata["experiment_stage_mode"].as_str(),
        Some("LineScanX")
    );
    assert_eq!(records[0].metadata["spatial_x"].as_f64(), Some(-0.010));
    assert_eq!(
        records[0].metadata["spatial_x_unit"].as_str(),
        Some("unknown")
    );
    assert_eq!(records[20].metadata["spatial_x"].as_f64(), Some(0.010));
    assert_eq!(records[20].metadata["spatial_x_index"].as_u64(), Some(20));
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 97);
    assert!((signal.values.iter().sum::<f64>() - 44011.0).abs() < 0.000001);

    let records =
        open_path(workspace_file("samples/raman_trivista/map.tvf")).expect("open TriVista map");
    assert_eq!(records.len(), 81);
    assert_eq!(
        records[0].metadata["experiment_stage_mode"].as_str(),
        Some("MappingXY")
    );
    assert_eq!(records[0].metadata["spatial_x"].as_f64(), Some(-0.100));
    assert_eq!(records[0].metadata["spatial_y"].as_f64(), Some(-0.100));
    assert_eq!(
        records[0].metadata["spatial_x_unit"].as_str(),
        Some("unknown")
    );
    assert_eq!(
        records[0].metadata["spatial_y_unit"].as_str(),
        Some("unknown")
    );
    assert_eq!(records[80].metadata["spatial_x"].as_f64(), Some(0.100));
    assert_eq!(records[80].metadata["spatial_y"].as_f64(), Some(0.100));
    assert_eq!(records[80].metadata["spatial_x_index"].as_u64(), Some(8));
    assert_eq!(records[80].metadata["spatial_y_index"].as_u64(), Some(8));

    let records = open_path(workspace_file(
        "samples/raman_trivista/spec_multiple_spectrometers.tvf",
    ))
    .expect("open TriVista multi-spectrometer");
    assert_eq!(records.len(), 1);
    assert!(!records[0].metadata.contains_key("time_index"));
    assert_eq!(records[0].metadata["spectrometer_count"].as_u64(), Some(3));
    assert_eq!(
        records[0].metadata["spectrometer_serial_numbers"]
            .as_array()
            .and_then(|values| values.first())
            .and_then(|value| value.as_str()),
        Some("25580419")
    );
    assert_eq!(
        records[0].metadata["spectrometer_models"]
            .as_array()
            .and_then(|values| values.get(2))
            .and_then(|value| value.as_str()),
        Some("SP-2-750i")
    );
    assert_eq!(
        records[0].metadata["spectrometer_stage_numbers"]
            .as_array()
            .and_then(|values| values.get(2))
            .and_then(|value| value.as_f64()),
        Some(3.0)
    );

    let records = open_path(workspace_file(
        "samples/raman_trivista/spec_timeseries_2x1s_delta3s.tvf",
    ))
    .expect("open TriVista time series");
    assert_eq!(records.len(), 2);
    assert_eq!(records[1].metadata["time_index"].as_u64(), Some(1));
    assert!(
        (records[1].metadata["elapsed_time_seconds"]
            .as_f64()
            .unwrap()
            - 4.0442314)
            .abs()
            < 0.000001
    );

    let records = open_path(workspace_file(
        "samples/raman_trivista/spec_step_and_glue.tvf",
    ))
    .expect("open TriVista step-and-glue");
    assert_eq!(records.len(), 20);
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 18000);
    assert_eq!(records[0].metadata["xdim_length"].as_u64(), Some(18000));
    assert!(!records[0].metadata.contains_key("time_index"));
    assert_eq!(
        records[0].metadata["child_document_count"].as_u64(),
        Some(19)
    );
    assert_eq!(records[1].metadata["document_role"].as_str(), Some("child"));
    assert_eq!(records[1].metadata["xdim_length"].as_u64(), Some(1024));
    let signal = records[1]
        .signals
        .get("intensity")
        .expect("child intensity");
    assert_eq!(signal.axis.values.len(), 1024);
}

#[test]
fn reads_digitalsurf_sur_pro_modes() {
    let records =
        open_path(workspace_file("samples/digitalsurf/test_spectrum.pro")).expect("open spectrum");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "digitalsurf-sur-pro");
    assert_eq!(
        records[0].metadata["object_type_label"].as_str(),
        Some("_SPECTRUM")
    );
    assert_eq!(
        records[0].metadata["signal_axis_original_unit"].as_str(),
        Some("mm")
    );
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 512);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.signal_type, SignalType::RawCounts);
    assert!((signal.axis.values[0] - 172.84281784668565).abs() < 0.000001);
    assert!((signal.axis.values[511] - 726.7669435577773).abs() < 0.000001);
    assert!((signal.values[0] - 2438.830136228884).abs() < 0.000001);
    assert!((signal.values[511] - 2671.460130352156).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 1377533.5414941004).abs() < 0.000001);

    let records =
        open_path(workspace_file("samples/digitalsurf/test_spectra.pro")).expect("open spectra");
    assert_eq!(records.len(), 65);
    assert_eq!(
        records[64].metadata["spectrum_position"].as_f64().unwrap(),
        0.0073336162604391575
    );
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 512);
    assert!((signal.values.iter().sum::<f64>() - 207561.0).abs() < 0.000001);
    let signal = records[64].signals.get("intensity").expect("intensity");
    assert!((signal.values.iter().sum::<f64>() - 221920.0).abs() < 0.000001);

    let records = open_path(workspace_file("samples/digitalsurf/test_spectral_map.sur"))
        .expect("open spectral map");
    assert_eq!(records.len(), 120);
    assert_eq!(
        records[0].metadata["object_type_label"].as_str(),
        Some("_HYPCARD")
    );
    assert_eq!(records[0].metadata["map_width"].as_u64(), Some(10));
    assert_eq!(records[0].metadata["map_height"].as_u64(), Some(12));
    assert_eq!(
        records[0].metadata["map_axis_order"].as_str(),
        Some("y_slowest_x_fastest")
    );
    assert_eq!(records[0].metadata["map_x_index"].as_u64(), Some(0));
    assert_eq!(records[0].metadata["map_y_index"].as_u64(), Some(0));
    assert_eq!(records[119].metadata["map_x_index"].as_u64(), Some(9));
    assert_eq!(records[119].metadata["map_y_index"].as_u64(), Some(11));
    assert!(
        (records[119].metadata["spatial_x"].as_f64().unwrap() - 0.007757065512123518).abs()
            < 0.000001
    );
    assert!(
        (records[119].metadata["spatial_y"].as_f64().unwrap() - 0.003961054855608381).abs()
            < 0.000001
    );
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 310);
    assert_eq!(signal.axis.unit, "nm");
    assert!((signal.axis.values[0] - 333.2748601678759).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 115284.0).abs() < 0.000001);
    let signal = records[119].signals.get("intensity").expect("intensity");
    assert!((signal.values.iter().sum::<f64>() - 127121.0).abs() < 0.000001);

    let records = open_path(workspace_file(
        "samples/digitalsurf/test_spectral_map_compressed.sur",
    ))
    .expect("open compressed spectral map");
    assert_eq!(records.len(), 120);
    assert_eq!(records[0].metadata["map_width"].as_u64(), Some(10));
    assert_eq!(records[0].metadata["map_height"].as_u64(), Some(12));
    assert_eq!(
        records[0].metadata["map_axis_order"].as_str(),
        Some("y_slowest_x_fastest")
    );
    assert_eq!(records[119].metadata["map_x_index"].as_u64(), Some(9));
    assert_eq!(records[119].metadata["map_y_index"].as_u64(), Some(11));
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"digitalsurf_zlib_stream_decompressed".to_string()));
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 281);
    assert!((signal.axis.values[0] - 344.11484375596046).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 118502.0).abs() < 0.000001);
    let signal = records[119].signals.get("intensity").expect("intensity");
    assert!((signal.values.iter().sum::<f64>() - 112712.0).abs() < 0.000001);

    let records =
        open_path(workspace_file("samples/digitalsurf/test_surface.sur")).expect("open surface");
    assert_eq!(records.len(), 128);
    assert_eq!(
        records[0].metadata["object_type_label"].as_str(),
        Some("_SURFACE")
    );
    assert_eq!(records[0].metadata["surface_width"].as_u64(), Some(128));
    assert_eq!(records[0].metadata["surface_height"].as_u64(), Some(128));
    assert_eq!(
        records[0].metadata["surface_axis_order"].as_str(),
        Some("row_profiles_y_slowest_x_fastest")
    );
    assert_eq!(records[0].metadata["spatial_y_index"].as_u64(), Some(0));
    assert_eq!(records[127].metadata["spatial_y_index"].as_u64(), Some(127));
    assert_eq!(records[0].metadata["spatial_x_unit"].as_str(), Some("mm"));
    assert_eq!(records[0].metadata["spatial_y_unit"].as_str(), Some("mm"));
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 128);
    assert_eq!(signal.axis.kind, AxisKind::Index);
    assert_eq!(signal.axis.unit, "mm");
    assert!((signal.values.iter().sum::<f64>() - 56206.743748958834).abs() < 0.000001);
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"digitalsurf_surface_rows_exported_as_profiles".to_string()));
}

#[test]
fn reads_hamamatsu_img_streak_camera_modes() {
    let records = open_path(workspace_file("samples/hamamatsu/operate_mode.img"))
        .expect("open Hamamatsu operate");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "hamamatsu-img");
    assert_eq!(
        records[0].metadata["acquisition_mode_label"].as_str(),
        Some("analog_integration")
    );
    assert_eq!(records[0].metadata["image_width"].as_u64(), Some(672));
    assert_eq!(records[0].metadata["image_height"].as_u64(), Some(512));
    assert_eq!(records[0].metadata["y_axis_name"].as_str(), Some("Time"));
    assert_eq!(records[0].metadata["y_axis_unit"].as_str(), Some("us"));
    assert_eq!(records[0].metadata["y_axis_kind"].as_str(), Some("time"));
    assert!((records[0].metadata["y_axis_first"].as_f64().unwrap() - 0.0).abs() < 0.000001);
    assert!(
        (records[0].metadata["y_axis_last"].as_f64().unwrap() - 16.009395599365234).abs()
            < 0.000001
    );
    let y_values = records[0].metadata["y_axis_values"].as_array().unwrap();
    assert!((y_values[1].as_f64().unwrap() - 0.031080815941095352).abs() < 0.000001);
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 672);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.dims, vec!["y".to_string(), "x".to_string()]);
    assert_eq!(signal.values.len(), 512 * 672);
    assert_eq!(signal.values[0], 0.0);
    assert_eq!(signal.values[2], 715.0);
    assert_eq!(signal.values[672], 246.0);
    assert!((signal.axis.values[0] - 472.25201416015625).abs() < 0.000001);
    assert!((signal.axis.values[671] - 526.844482421875).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 7061710453.0).abs() < 0.000001);
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"hamamatsu_img_secondary_time_axis_in_metadata".to_string()));

    let records = open_path(workspace_file("samples/hamamatsu/focus_mode.img"))
        .expect("open Hamamatsu focus");
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].metadata["y_axis_name"].as_str(),
        Some("Vertical CCD Position")
    );
    assert_eq!(records[0].metadata["y_axis_kind"].as_str(), Some("index"));
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.values[signal.values.len() - 3], 21.0);
    assert!((signal.values.iter().sum::<f64>() - 59743889.0).abs() < 0.000001);
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"hamamatsu_img_y_axis_is_detector_position".to_string()));

    let records = open_path(workspace_file("samples/hamamatsu/photon_counting.img"))
        .expect("open Hamamatsu photon counting");
    assert_eq!(
        records[0].metadata["acquisition_mode_label"].as_str(),
        Some("photon_counting")
    );
    assert_eq!(records[0].metadata["y_axis_kind"].as_str(), Some("time"));
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.values.len(), 672);
    assert_eq!(signal.values.len(), 512 * 672);
    assert!((signal.values.iter().sum::<f64>() - 110996.0).abs() < 0.000001);

    let records = open_path(workspace_file("samples/hamamatsu/shading_file.img"))
        .expect("open Hamamatsu shading");
    assert_eq!(records[0].metadata["y_axis_kind"].as_str(), Some("time"));
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.values[0], 9385.0);
    assert_eq!(signal.values[1], 8354.0);
    assert!((signal.values.iter().sum::<f64>() - 182917341484.0).abs() < 0.000001);

    let records = open_path(workspace_file("samples/hamamatsu/xaxis_other.img"))
        .expect("open Hamamatsu uncalibrated");
    let signal = records[0].signals.get("intensity").expect("intensity");
    assert_eq!(signal.axis.kind, AxisKind::Index);
    assert_eq!(signal.axis.unit, "px");
    assert_eq!(records[0].metadata["y_axis_kind"].as_str(), Some("index"));
    assert_eq!(signal.values.len(), 508 * 672);
    assert_eq!(signal.values[0], 406.0);
    assert!((signal.values.iter().sum::<f64>() - 137039886.0).abs() < 0.000001);
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"hamamatsu_img_uncalibrated_x_axis".to_string()));
}

#[test]
fn refuses_mzml_mass_spectrometry_containers() {
    for relative in [
        "samples/mzml/example.mzML",
        "samples/mzml/mini.chrom.mzML",
        "samples/mzml/mini_numpress.chrom.mzML",
    ] {
        let err = open_path(workspace_file(relative)).expect_err("mzML should be refused");
        let message = err.to_string();
        assert!(
            message.contains("mass-spectrometry data"),
            "{relative}: {message}"
        );
        assert!(message.contains("pyteomics"), "{relative}: {message}");
    }
}

#[test]
fn reads_avantes_wave_table() {
    let records = open_path(workspace_file("samples/avantes/avantes_export.ttt"))
        .expect("open avantes table");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "avantes-ascii");
    let signal = records[0]
        .signals
        .get("transmittance")
        .expect("transmittance");
    assert_eq!(signal.axis.values.len(), 401);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.signal_type, SignalType::Transmittance);
    assert!((signal.axis.values[0] - 300.0).abs() < 0.000001);
    assert!((signal.axis.values[400] - 700.0).abs() < 0.000001);
    assert!((signal.values[0] - 3.1487).abs() < 0.000001);
    assert!((signal.values[400] - 31.4912).abs() < 0.000001);
}

#[test]
fn reads_avantes_wave_table_sample_counts() {
    let records = open_path(workspace_file("samples/avantes/avantes_export2.trt"))
        .expect("open avantes sample-count table");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "avantes-ascii");
    let signal = records[0].signals.get("sample").expect("sample");
    assert_eq!(signal.axis.values.len(), 1_442);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.signal_type, SignalType::RawCounts);
    assert!((signal.axis.values[0] - 275.27).abs() < 0.000001);
    assert!((signal.axis.values[1_441] - 1100.13).abs() < 0.000001);
    assert!((signal.values[0] - 805.0).abs() < 0.000001);
    assert!((signal.values[1_441] - 774.3).abs() < 0.000001);
}

#[test]
fn reads_avantes_wave_table_long_multi_signal() {
    let records = open_path(workspace_file("samples/avantes/avantes_export_long.ttt"))
        .expect("open avantes long table");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "avantes-ascii");
    for name in ["dark", "ref", "sample", "transmittance"] {
        assert!(records[0].signals.contains_key(name), "missing {name}");
    }
    let transmittance = records[0]
        .signals
        .get("transmittance")
        .expect("transmittance");
    assert_eq!(transmittance.axis.values.len(), 1_442);
    assert_eq!(transmittance.axis.unit, "nm");
    assert_eq!(transmittance.axis.kind, AxisKind::Wavelength);
    assert_eq!(transmittance.signal_type, SignalType::Transmittance);
    assert!((transmittance.axis.values[0] - 275.27).abs() < 0.000001);
    assert!((transmittance.axis.values[1_441] - 1100.13).abs() < 0.000001);
    assert!((transmittance.values[0] - 23.333).abs() < 0.000001);
    assert!((transmittance.values[1_441] - 75.393).abs() < 0.000001);
    let sample = records[0].signals.get("sample").expect("sample");
    assert_eq!(sample.signal_type, SignalType::RawCounts);
    let dark = records[0].signals.get("dark").expect("dark");
    assert_eq!(dark.signal_type, SignalType::RawCounts);
    let reference = records[0].signals.get("ref").expect("ref");
    assert_eq!(reference.signal_type, SignalType::RawCounts);
}

#[test]
fn reads_avantes_irradiance_export() {
    let records =
        open_path(workspace_file("samples/avantes/irr_820_1941.IRR")).expect("open avantes irr");

    assert_eq!(records.len(), 1);
    let signal = records[0].signals.get("irradiance").expect("irradiance");
    assert_eq!(signal.axis.values.len(), 1_922);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.signal_type, SignalType::Irradiance);
    assert!((signal.axis.values[0] - 173.0).abs() < 0.000001);
    assert!((signal.axis.values[1_921] - 1133.5).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 1.416468).abs() < 0.000001);
}

#[test]
fn reads_avantes_avasoft8_ascii_export() {
    let records =
        open_path(workspace_file("samples/avantes/avasoft8.txt")).expect("open avasoft8 txt");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "avantes-ascii");
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert_eq!(reflectance.axis.values.len(), 401);
    assert_eq!(reflectance.axis.unit, "nm");
    assert_eq!(reflectance.axis.kind, AxisKind::Wavelength);
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert!((reflectance.axis.values[0] - 300.0).abs() < 0.000001);
    assert!((reflectance.axis.values[400] - 700.0).abs() < 0.000001);
    assert!((reflectance.values[0] - 252.16336).abs() < 0.000001);
    assert!((reflectance.values[400] - 96.33834).abs() < 0.000001);
    let dark = records[0].signals.get("dark").expect("dark");
    assert_eq!(dark.signal_type, SignalType::RawCounts);
    let reference = records[0].signals.get("reference").expect("reference");
    assert_eq!(reference.signal_type, SignalType::RawCounts);
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

    let metadata = &records[0].metadata;
    assert_eq!(metadata["measurement_mode"].as_str(), Some("transmittance"));
    assert_eq!(metadata["point_count"].as_u64(), Some(1_442));
    assert_eq!(metadata["first_pixel"].as_u64(), Some(0));
    assert_eq!(metadata["last_pixel"].as_u64(), Some(1_441));
    assert!((metadata["version_id"].as_f64().expect("version_id") - 70.0).abs() < 0.001);
    assert!(metadata.contains_key("integration_time_ms"));
    assert!(metadata.contains_key("averages_count"));
    let raw = &metadata["avantes"];
    assert_eq!(raw["family"].as_str(), Some("AvaSoft legacy"));
    assert_eq!(raw["mode"].as_str(), Some("Transmittance"));
}

#[test]
fn reads_avantes_legacy_alternate_transmittance_binary() {
    let records =
        open_path(workspace_file("samples/avantes/avantes_trans.TRM")).expect("open alt trm");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "avantes-legacy-binary");
    let transmittance = records[0]
        .signals
        .get("transmittance")
        .expect("transmittance");
    assert_eq!(transmittance.axis.values.len(), 1_623);
    assert_eq!(transmittance.axis.unit, "nm");
    assert_eq!(transmittance.axis.kind, AxisKind::Wavelength);
    assert_eq!(transmittance.signal_type, SignalType::Transmittance);
    assert!((transmittance.axis.values[0] - 179.100616).abs() < 0.000001);
    assert!((transmittance.axis.values[1_622] - 1100.34788).abs() < 0.000001);
    assert!((transmittance.values[0] - 30.313837).abs() < 0.000001);
    assert!((transmittance.values[1_622] - 54.054054).abs() < 0.000001);
    // Legacy `.TRM` carries the full triple plus acquisition metadata; the
    // mode label and acquisition values must be promoted top-level.
    let metadata = &records[0].metadata;
    assert_eq!(metadata["measurement_mode"].as_str(), Some("transmittance"));
    assert_eq!(metadata["point_count"].as_u64(), Some(1_623));
    assert!(metadata.contains_key("integration_time_ms"));
}

#[test]
fn reads_avantes_legacy_raw_reference_binaries() {
    for (relative, signal_name, first_value, expected_mode) in [
        (
            "samples/avantes/avantes_reflect.ROH",
            "scope",
            805.0,
            "raw_scope",
        ),
        (
            "samples/avantes/1305084U1.DRK",
            "dark_reference",
            785.900024,
            "dark_reference",
        ),
        (
            "samples/avantes/1305084U1.REF",
            "white_reference",
            856.0,
            "white_reference",
        ),
    ] {
        let records = open_path(workspace_file(relative)).expect("open avantes legacy raw");
        assert_eq!(records.len(), 1);
        let signal = records[0].signals.get(signal_name).expect(signal_name);
        assert_eq!(signal.axis.values.len(), 1_442);
        assert_eq!(signal.signal_type, SignalType::RawCounts);
        assert!((signal.values[0] - first_value).abs() < 0.000001);
        // Legacy single-channel files only carry one vector; we expect the
        // top-level harmonized metadata and the provenance warning that flags
        // the missing companion files.
        let metadata = &records[0].metadata;
        assert_eq!(
            metadata["measurement_mode"].as_str(),
            Some(expected_mode),
            "{relative}"
        );
        assert_eq!(metadata["point_count"].as_u64(), Some(1_442), "{relative}");
        let warning =
            format!("avantes_legacy_single_channel:{expected_mode}:companion_files_required");
        assert!(
            records[0].provenance.warnings.contains(&warning),
            "{relative}: warnings = {:?}",
            records[0].provenance.warnings
        );
    }
}

#[test]
fn reads_avantes_avasoft8_raw_binary() {
    let records =
        open_path(workspace_file("samples/avantes/1904090M1_0003.Raw8")).expect("open raw8");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "avantes-avasoft8-binary");
    assert!(records[0].provenance.warnings.is_empty());
    assert!(records[0].signals.contains_key("dark_reference"));
    assert!(records[0].signals.contains_key("white_reference"));
    let metadata = &records[0].metadata["avantes"];
    assert_eq!(metadata["magic"].as_str(), Some("AVS84"));
    assert_eq!(metadata["measure_mode"].as_u64(), Some(0));
    assert_eq!(
        records[0].metadata["acquisition_start_date"].as_str(),
        Some("2019-07-06")
    );
    assert_eq!(
        records[0].metadata["acquisition_start_time"].as_str(),
        Some("15:48")
    );
    assert_eq!(
        metadata["spc_date_decoded"]["date"].as_str(),
        Some("2019-07-06")
    );
    assert_eq!(metadata["spc_date_decoded"]["minute"].as_u64(), Some(48));
    // Harmonized top-level metadata mirrors the raw block for cross-format
    // consumers.
    let top = &records[0].metadata;
    assert_eq!(top["magic"].as_str(), Some("AVS84"));
    assert_eq!(top["measurement_mode"].as_str(), Some("raw_scope"));
    assert_eq!(top["point_count"].as_u64(), Some(1_019));
    assert_eq!(top["first_pixel"].as_u64(), Some(238));
    assert_eq!(top["last_pixel"].as_u64(), Some(1_256));
    assert_eq!(top["instrument_serial"].as_str(), Some("1904090M1"));
    assert_eq!(top["operator"].as_str(), Some("1904090M1"));
    assert!(top.contains_key("integration_time_ms"));
    assert!(top.contains_key("averages_count"));
    assert!(top.contains_key("integration_delay"));
    let scope = records[0].signals.get("scope").expect("scope");
    assert_eq!(scope.axis.values.len(), 1_019);
    assert_eq!(scope.axis.unit, "nm");
    assert_eq!(scope.axis.kind, AxisKind::Wavelength);
    assert_eq!(scope.signal_type, SignalType::RawCounts);
    assert!((scope.axis.values[0] - 300.013855).abs() < 0.000001);
    assert!((scope.axis.values[1_018] - 899.874878).abs() < 0.000001);
    assert!((scope.values[0] - 267.155243).abs() < 0.000001);
    assert!((scope.values[1_018] - 360.127502).abs() < 0.000001);
    let sample = records[0].signals.get("sample").expect("sample");
    assert_eq!(scope.values, sample.values);
}

#[test]
fn reads_avantes_avasoft8_irradiance_binary() {
    let records = open_path(workspace_file("samples/avantes/eg.IRR8")).expect("open irr8");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "avantes-avasoft8-binary");
    let metadata = &records[0].metadata["avantes"];
    assert_eq!(metadata["magic"].as_str(), Some("AVS84"));
    assert_eq!(metadata["measure_mode"].as_u64(), Some(4));
    assert_eq!(
        records[0].metadata["acquisition_start_date"].as_str(),
        Some("2022-03-20")
    );
    assert_eq!(
        records[0].metadata["acquisition_start_time"].as_str(),
        Some("16:39")
    );
    assert_eq!(
        metadata["spc_date_decoded"]["date"].as_str(),
        Some("2022-03-20")
    );
    assert_eq!(metadata["spc_date_decoded"]["hour"].as_u64(), Some(16));
    let irradiance = records[0].signals.get("irradiance").expect("irradiance");
    assert_eq!(irradiance.axis.values.len(), 1_620);
    assert_eq!(irradiance.axis.unit, "nm");
    assert_eq!(irradiance.axis.kind, AxisKind::Wavelength);
    assert_eq!(irradiance.signal_type, SignalType::Irradiance);
    assert!((irradiance.axis.values[0] - 144.942429).abs() < 0.000001);
    assert!((irradiance.axis.values[1_619] - 1100.441406).abs() < 0.000001);
    assert!((irradiance.values[0] - 1096.812012).abs() < 0.000001);
    assert!((irradiance.values[1_619] - 2009.875).abs() < 0.000001);
    let sample = records[0].signals.get("sample").expect("sample");
    assert_eq!(irradiance.values, sample.values);
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"avantes_irr8_irradiance_calibration_not_applied".to_string()));
    // The fourth array in IRR8 mode is the per-pixel irradiance calibration
    // vector (very large dynamic range), not a raw white scan: it must NOT be
    // exposed under `white_reference` and must be labelled accordingly.
    assert!(!records[0].signals.contains_key("white_reference"));
    let calibration = records[0]
        .signals
        .get("irradiance_calibration")
        .expect("irradiance_calibration");
    assert_eq!(calibration.axis.values.len(), 1_620);
    assert_eq!(calibration.signal_type, SignalType::Unknown);
    assert!(calibration.values[0] > 1.0e9);
    // Harmonized top-level metadata is promoted for IRR8 too.
    let top = &records[0].metadata;
    assert_eq!(top["magic"].as_str(), Some("AVS84"));
    assert_eq!(top["measurement_mode"].as_str(), Some("irradiance"));
    assert_eq!(top["point_count"].as_u64(), Some(1_620));
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
fn reads_ocean_optics_oceanview_text_export() {
    let records =
        open_path(workspace_file("samples/ocean_optics/OceanView.txt")).expect("open oceanview");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "ocean-optics-text");
    let signal = records[0].signals.get("processed").expect("processed");
    assert_eq!(signal.axis.values.len(), 2_389);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.axis.order, AxisOrder::Ascending);
    assert_eq!(signal.signal_type, SignalType::Unknown);
    assert!((signal.axis.values[0] - 187.92).abs() < 0.000001);
    assert!((signal.axis.values[2_388] - 2_116.5).abs() < 0.000001);
    assert!((signal.values[0] - 18.995).abs() < 0.000001);
    assert!((signal.values[2_388] - 4.6991).abs() < 0.000001);
}

#[test]
fn reads_ocean_optics_two_column_csv_export() {
    let records =
        open_path(workspace_file("samples/ocean_optics/spec.csv")).expect("open ocean csv");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "ocean-optics-two-column-csv");
    let signal = records[0].signals.get("processed").expect("processed");
    assert_eq!(signal.axis.values.len(), 1_994);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.axis.order, AxisOrder::Ascending);
    assert_eq!(signal.signal_type, SignalType::Unknown);
    assert!((signal.axis.values[0] - 299.99).abs() < 0.000001);
    assert!((signal.axis.values[1_993] - 700.03).abs() < 0.000001);
    assert!((signal.values[0] - 10.013).abs() < 0.000001);
    assert!((signal.values[1_993] - 15.408).abs() < 0.000001);
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
    assert_eq!(records[0].signal_type, SignalType::Transmittance);
    let transmittance = records[0]
        .signals
        .get("transmittance")
        .expect("transmittance");
    assert_eq!(transmittance.axis.values.len(), 3_648);
    assert_eq!(transmittance.axis.unit, "nm");
    assert_eq!(transmittance.axis.kind, AxisKind::Wavelength);
    assert_eq!(transmittance.unit.as_deref(), Some("%"));
    assert!((transmittance.axis.values[0] - 176.3604183).abs() < 0.000001);
    assert!((transmittance.axis.values[3_647] - 893.6943397004063).abs() < 0.000001);
    assert_eq!(transmittance.signal_type, SignalType::Transmittance);
    assert!((transmittance.values[0] - 0.0).abs() < 0.000001);
    assert!((transmittance.values[3_647] - 125.07433102081265).abs() < 0.000001);
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
    assert_eq!(windows[0].signal_type, SignalType::Transmittance);
    let windows_transmittance = windows[0]
        .signals
        .get("transmittance")
        .expect("transmittance");
    assert_eq!(windows_transmittance.axis.values.len(), 2_048);
    assert_eq!(windows_transmittance.signal_type, SignalType::Transmittance);
    assert!((windows_transmittance.values[0] - 282.8571428571289).abs() < 0.000001);
    assert!((windows_transmittance.values[2_047] - 40.05032131664623).abs() < 0.000001);

    let whiteref =
        open_path(workspace_file("samples/ocean_optics/whiteref.ProcSpec")).expect("open whiteref");
    assert_eq!(whiteref[0].signal_type, SignalType::Reflectance);
    let whiteref_reflectance = whiteref[0].signals.get("reflectance").expect("reflectance");
    assert_eq!(whiteref_reflectance.axis.values.len(), 3_648);
    assert_eq!(whiteref_reflectance.signal_type, SignalType::Reflectance);
    assert_eq!(whiteref_reflectance.unit.as_deref(), Some("%"));
    assert!((whiteref_reflectance.values[0] - 0.0).abs() < 0.000001);
    assert!((whiteref_reflectance.values[3_647] - 97.29425028184893).abs() < 0.000001);
}

#[test]
fn reads_envi_standard_image_cube_as_pixel_spectra() {
    for relative in [
        "samples/envi_sli/cubescope-mini-cube.hdr",
        "samples/envi_sli/cubescope-mini-cube.img",
    ] {
        let records = open_path(workspace_file(relative)).expect("open envi cube");

        assert_eq!(records.len(), 2_304, "{relative}");
        assert_eq!(records[0].provenance.format, "envi-standard-cube");
        assert_eq!(
            records[0].metadata["sample_id"].as_str(),
            Some("pixel_y0_x0")
        );
        assert_eq!(records[0].metadata["pixel_x"].as_u64(), Some(0));
        assert_eq!(records[0].metadata["pixel_y"].as_u64(), Some(0));
        assert_eq!(records[0].metadata["spatial_x"].as_f64(), Some(500000.0));
        assert_eq!(records[0].metadata["spatial_y"].as_f64(), Some(4100000.0));
        assert_eq!(records[0].metadata["spatial_unit"].as_str(), Some("m"));
        assert_eq!(
            records[0].metadata["map_axis_order"].as_str(),
            Some("row_slowest_x_fastest")
        );
        assert_eq!(records[0].metadata["map_projection"].as_str(), Some("UTM"));
        assert_eq!(records[0].metadata["map_ref_pixel_x"].as_f64(), Some(1.0));
        assert_eq!(records[0].metadata["map_ref_pixel_y"].as_f64(), Some(1.0));
        assert_eq!(records[0].metadata["map_ref_x"].as_f64(), Some(500000.0));
        assert_eq!(records[0].metadata["map_ref_y"].as_f64(), Some(4100000.0));
        assert_eq!(records[0].metadata["map_pixel_size_x"].as_f64(), Some(30.0));
        assert_eq!(records[0].metadata["map_pixel_size_y"].as_f64(), Some(30.0));
        assert_eq!(records[0].metadata["map_zone"].as_str(), Some("50"));
        assert_eq!(
            records[0].metadata["map_hemisphere"].as_str(),
            Some("North")
        );
        assert_eq!(records[0].metadata["map_datum"].as_str(), Some("WGS-84"));
        assert_eq!(
            records[0].metadata["envi"]["map_info_parsed"]["unit"].as_str(),
            Some("m")
        );
        assert_eq!(
            records[0].metadata["envi"]["map_info_parsed"]["raw_unit"].as_str(),
            Some("Meters")
        );
        let signal = records[0].signals.get("spectrum").expect("spectrum");
        assert_eq!(signal.axis.values.len(), 32);
        assert_eq!(signal.axis.unit, "unknown");
        assert_eq!(signal.axis.kind, AxisKind::Wavelength);
        assert_eq!(signal.signal_type, SignalType::Unknown);
        assert!((signal.axis.values[0] - 400.0).abs() < 0.000001);
        assert!((signal.axis.values[31] - 710.0).abs() < 0.000001);
        assert!((signal.values[0] - 100.0).abs() < 0.000001);
        assert!((signal.values[31] - 3223.0).abs() < 0.000001);
        assert!((signal.values.iter().sum::<f64>() - 54_138.0).abs() < 0.000001);

        let last = &records[2_303];
        assert_eq!(last.metadata["sample_id"].as_str(), Some("pixel_y47_x47"));
        assert_eq!(last.metadata["spatial_x"].as_f64(), Some(501410.0));
        assert_eq!(last.metadata["spatial_y"].as_f64(), Some(4098590.0));
        let signal = last.signals.get("spectrum").expect("spectrum");
        assert!((signal.values[0] - 152.0).abs() < 0.000001);
        assert!((signal.values[31] - 3275.0).abs() < 0.000001);
    }
}

#[test]
fn reads_envi_standard_image_cube_window() {
    let path = workspace_file("samples/envi_sli/cubescope-mini-cube.hdr");
    let full = open_path(&path).expect("open full ENVI cube");
    let options = ReadOptions::default().with_cube_window(CubeWindow::new(2, Some(4), 3, Some(6)));

    let records = open_path_with_options(&path, &options).expect("open ENVI cube window");

    assert_eq!(records.len(), 6);
    let first = &records[0];
    assert_eq!(first.metadata["sample_id"].as_str(), Some("pixel_y2_x3"));
    assert_eq!(first.metadata["pixel_x"].as_u64(), Some(3));
    assert_eq!(first.metadata["pixel_y"].as_u64(), Some(2));
    assert_eq!(first.metadata["spatial_x"].as_f64(), Some(500090.0));
    assert_eq!(first.metadata["spatial_y"].as_f64(), Some(4099940.0));
    let full_index = 2 * 48 + 3;
    assert_eq!(
        records[0].signals["spectrum"].values,
        full[full_index].signals["spectrum"].values
    );
    let last = records.last().expect("last ROI pixel");
    assert_eq!(last.metadata["sample_id"].as_str(), Some("pixel_y3_x5"));

    let invalid =
        ReadOptions::default().with_cube_window(CubeWindow::new(47, Some(49), 0, Some(1)));
    let err = open_path_with_options(path, &invalid).expect_err("invalid cube window");
    assert!(err.to_string().contains("row window 47..49"));
}

#[test]
fn reads_envi_standard_image_cube_sparse_mask() {
    let path = workspace_file("samples/envi_sli/cubescope-mini-cube.hdr");
    let full = open_path(&path).expect("open full ENVI cube");

    let pixels = vec![(47, 47), (0, 0), (12, 7)];
    let mask = ReadOptions::default().with_cube_mask(CubeMask::new(pixels.clone()));
    let records = open_path_with_options(&path, &mask).expect("open ENVI cube mask");

    assert_eq!(records.len(), pixels.len());
    for (record, &(row, col)) in records.iter().zip(&pixels) {
        assert_eq!(
            record.metadata["sample_id"].as_str(),
            Some(format!("pixel_y{row}_x{col}").as_str())
        );
        assert_eq!(record.metadata["pixel_x"].as_u64(), Some(col as u64));
        assert_eq!(record.metadata["pixel_y"].as_u64(), Some(row as u64));
        let full_index = row * 48 + col;
        assert_eq!(
            record.signals["spectrum"].values,
            full[full_index].signals["spectrum"].values
        );
    }

    let empty = ReadOptions::default().with_cube_mask(CubeMask::new(Vec::new()));
    let err = open_path_with_options(&path, &empty).expect_err("empty mask");
    assert!(err.to_string().contains("ENVI Standard cube mask is empty"));

    let out_of_bounds = ReadOptions::default().with_cube_mask(CubeMask::new(vec![(0, 48)]));
    let err = open_path_with_options(path, &out_of_bounds).expect_err("out of bounds mask");
    assert!(err
        .to_string()
        .contains("mask pixel (0, 48) is outside 0..48 x 0..48"));
}

#[test]
fn reads_envi_standard_image_cube_as_single_nd_record() {
    let path = workspace_file("samples/envi_sli/cubescope-mini-cube.hdr");
    let per_pixel = open_path(&path).expect("open ENVI cube per-pixel");

    let options = ReadOptions::default().single_record();
    let records = open_path_with_options(&path, &options).expect("open ENVI cube single record");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "envi-standard-cube");
    let signal = records[0].signals.get("spectrum").expect("spectrum");
    assert_eq!(signal.shape, vec![48, 48, 32]);
    assert_eq!(signal.dims, vec!["row", "col", "x"]);
    assert_eq!(signal.axis.values.len(), 32);
    assert_eq!(signal.coords["row"].values.len(), 48);
    assert_eq!(signal.coords["col"].values.len(), 48);
    // C-order [row][col][band]: pixel (0,0) is the first 32 values and the
    // interleave (bsq here) is resolved back to the same spectrum as
    // the per-pixel reader.
    assert_eq!(
        signal.values[..32].to_vec(),
        per_pixel[0].signals["spectrum"].values
    );
    // Map-level georeferencing is preserved on the single record.
    assert_eq!(records[0].metadata["map_projection"].as_str(), Some("UTM"));
    assert_eq!(records[0].metadata["map_pixel_size_x"].as_f64(), Some(30.0));
}

#[test]
fn reads_numpy_npy_matrix_with_generated_axis() {
    let records =
        open_path(workspace_file("samples/numpy/synthetic_nirs_X.npy")).expect("open npy");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "numpy-npy");
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("row_0"));
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"numpy_npy_axis_generated_index".to_string()));
    let signal = records[0].signals.get("spectrum").expect("spectrum");
    assert_eq!(signal.axis.values.len(), 200);
    assert_eq!(signal.axis.unit, "index");
    assert_eq!(signal.axis.kind, AxisKind::Index);
    assert!((signal.values[0] - 0.0367427170).abs() < 0.000001);
    assert!((signal.values[199] + 0.1465858221).abs() < 0.000001);
    let last = records[49].signals.get("spectrum").expect("spectrum");
    assert!((last.values[199] - 0.0608757548).abs() < 0.000001);
}

#[test]
fn reads_numpy_npz_canonical_dataset() {
    let records = open_path(workspace_file("samples/numpy/synthetic_nirs.npz")).expect("open npz");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "numpy-npz");
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("S000"));
    assert!((records[0].targets["y"].as_f64().expect("target") - 10.53211185).abs() < 0.000001);
    let signal = records[0].signals.get("spectrum").expect("spectrum");
    assert_eq!(signal.axis.values.len(), 200);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert!((signal.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((signal.axis.values[199] - 2500.0).abs() < 0.000001);
    assert!((signal.values[0] - 0.0367427170).abs() < 0.000001);
    assert!((signal.values[199] + 0.1465858221).abs() < 0.000001);
}

#[test]
fn reads_parquet_spectral_matrix() {
    let records =
        open_path(workspace_file("samples/parquet/synthetic_nirs.parquet")).expect("open parquet");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "parquet-nirs-table");
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("S000"));
    assert!(
        (records[0].targets["protein"].as_f64().expect("protein") - 10.5321118543).abs() < 0.000001
    );
    let signal = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(signal.axis.values.len(), 200);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.signal_type, SignalType::Absorbance);
    assert!((signal.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((signal.axis.values[199] - 2500.0).abs() < 0.000001);
    assert!((signal.values[0] - 0.0367427152).abs() < 0.000001);
    assert!((signal.values[199] + 0.1465858247).abs() < 0.000001);
}

#[test]
fn rejects_non_spectral_parquet_table() {
    let err = open_path(workspace_file("samples/parquet/alltypes_plain.parquet"))
        .expect_err("non-spectral parquet should fail");

    assert!(err.to_string().contains("not a NIRS spectral table"));
}

#[test]
fn reads_spectral_evolution_sed() {
    let records = open_path(workspace_file(
        "samples/spectral_evolution/1566060_09506_working.sed",
    ))
    .expect("open sed");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "spectral-evolution-sed");
    assert!(record.signals.keys().any(|key| key.contains("reflect")));
    assert_eq!(
        record.metadata["acquisition_start_date"].as_str(),
        Some("2012-10-03")
    );
    assert_eq!(
        record.metadata["acquisition_end_time"].as_str(),
        Some("12:05:44")
    );
    assert_eq!(
        record.metadata["instrument"].as_str(),
        Some("PSR+3500_SN1566060 [3]")
    );
    assert_eq!(
        record.metadata["instrument_model"].as_str(),
        Some("PSR+3500")
    );
    assert_eq!(
        record.metadata["instrument_serial"].as_str(),
        Some("1566060")
    );
    assert_eq!(
        record.metadata["measurement_mode"].as_str(),
        Some("reflectance")
    );
    assert_eq!(
        record.metadata["radiometric_calibration"].as_str(),
        Some("DN")
    );
    assert_eq!(record.metadata["point_count"].as_u64(), Some(2_151));
    let wavelength_range = record.metadata["wavelength_range_nm"]
        .as_array()
        .expect("wavelength range");
    assert_eq!(wavelength_range[0].as_f64(), Some(350.0));
    assert_eq!(wavelength_range[1].as_f64(), Some(2500.0));
    assert_eq!(record.metadata["declared_column_count"].as_u64(), Some(4));
    assert_json_u64_array(&record.metadata["detector_channels"], &[512, 256, 256]);
    assert_json_f64_array(
        &record.metadata["detector_temperatures_reference_celsius"],
        &[26.14, 8.47, -5.77],
    );
    assert_json_f64_array(
        &record.metadata["detector_temperatures_target_celsius"],
        &[26.78, 8.54, -6.11],
    );
    assert_json_f64_array(
        &record.metadata["integration_time_reference_ms"],
        &[50.0, 50.0, 30.0],
    );
    assert_json_f64_array(
        &record.metadata["integration_time_target_ms"],
        &[100.0, 50.0, 30.0],
    );
    assert_json_f64_array(&record.metadata["battery_voltages_volts"], &[7.49, 7.40]);
    assert_json_u64_array(&record.metadata["scan_averages"], &[10, 10]);
    assert_json_str_array(&record.metadata["dark_mode"], &["AUTO", "AUTO"]);
    assert_json_str_array(&record.metadata["foreoptic"], &["PROBE", "PROBE"]);
    assert_json_str_array(&record.metadata["foreoptic_signal_units"], &["DN", "DN"]);
    assert_sed_signal_units(record, &["DN", "DN", "%"]);
    assert!(!record.metadata.contains_key("gps_latitude"));

    let reference = record.signals.get("norm__dn_ref_").expect("DN reference");
    assert_eq!(reference.unit.as_deref(), Some("DN"));
    let target = record.signals.get("norm__dn_target").expect("DN target");
    assert_eq!(target.unit.as_deref(), Some("DN"));
    let reflectance = record
        .signals
        .iter()
        .find(|(key, _)| key.contains("reflect"))
        .map(|(_, value)| value)
        .expect("reflectance");
    assert_eq!(reflectance.axis.values.len(), 2_151);
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert_eq!(reflectance.unit.as_deref(), Some("%"));
}

#[test]
fn flags_spectral_evolution_sed_without_reflectance() {
    let records = open_path(workspace_file(
        "samples/spectral_evolution/1566060_15025_not_working.sed",
    ))
    .expect("open broken-but-valid sed");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "spectral-evolution-sed");
    assert_eq!(records[0].signal_type, SignalType::RawCounts);
    assert_eq!(records[0].signals.len(), 2);
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"sed_missing_reflectance_signal".to_string()));
    assert!(records[0]
        .quality_flags
        .contains(&"missing_reflectance_signal".to_string()));
    assert!(!records[0]
        .signals
        .values()
        .any(|signal| signal.signal_type == SignalType::Reflectance));

    let reference = records[0]
        .signals
        .get("norm__dn_ref_")
        .expect("DN reference");
    assert_eq!(reference.axis.values.len(), 2_151);
    assert_eq!(reference.axis.unit, "nm");
    assert_eq!(reference.axis.kind, AxisKind::Wavelength);
    assert_eq!(reference.signal_type, SignalType::RawCounts);
    assert_eq!(reference.unit.as_deref(), Some("DN"));
    assert!((reference.axis.values[0] - 350.0).abs() < 0.000001);
    assert!((reference.axis.values[2_150] - 2500.0).abs() < 0.000001);
    assert!((reference.values[0] - 5.282287).abs() < 0.000001);
    assert!((reference.values[2_150] - 16.15534).abs() < 0.000001);

    let target = records[0]
        .signals
        .get("norm__dn_target")
        .expect("DN target");
    assert_eq!(target.signal_type, SignalType::RawCounts);
    assert_eq!(target.unit.as_deref(), Some("DN"));
    assert!((target.values[0] - 1.922703).abs() < 0.000001);
    assert!((target.values[2_150] - 1.271258).abs() < 0.000001);
    assert_sed_signal_units(&records[0], &["DN", "DN"]);
    assert_eq!(
        records[0].metadata["measurement_mode"].as_str(),
        Some("direct_energy")
    );
    assert_eq!(records[0].metadata["point_count"].as_u64(), Some(2_151));
    assert_eq!(
        records[0].metadata["declared_column_count"].as_u64(),
        Some(3)
    );
    assert_json_f64_array(
        &records[0].metadata["integration_time_reference_ms"],
        &[20.0, 32.0, 27.0],
    );
    assert_json_f64_array(
        &records[0].metadata["integration_time_target_ms"],
        &[50.0, 50.0, 30.0],
    );

    let vendor = &records[0].metadata["vendor"];
    assert_eq!(
        vendor["instrument"].as_str(),
        Some("PSR+3500_SN1566060 [3]")
    );
    assert_eq!(vendor["measurement"].as_str(), Some("DIRECT_ENERGY"));
    assert_eq!(vendor["radiometric_calibration"].as_str(), Some("DN"));
}

#[test]
fn reads_spectral_evolution_sed_fraction_reflectance_units_and_gps() {
    let records = open_path(workspace_file(
        "samples/spectral_evolution/serbinsh_cvars_grape_leaf.sed",
    ))
    .expect("open serbin sed");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "spectral-evolution-sed");
    assert_eq!(
        record.metadata["instrument_model"].as_str(),
        Some("PSM-3500")
    );
    assert_eq!(
        record.metadata["instrument_serial"].as_str(),
        Some("1336023")
    );
    assert_eq!(
        record.metadata["measurement_mode"].as_str(),
        Some("reflectance")
    );
    assert_eq!(record.metadata["point_count"].as_u64(), Some(2_151));
    assert_eq!(record.metadata["gps_time"].as_str(), Some("15:57:11"));
    assert_eq!(record.metadata["gps_satellites_used"].as_u64(), Some(7));
    assert_eq!(record.metadata["gps_satellites_visible"].as_u64(), Some(11));
    assert!((record.metadata["gps_latitude"].as_f64().unwrap() - 33.52465).abs() < 0.000001);
    assert!((record.metadata["gps_longitude"].as_f64().unwrap() + 116.16258).abs() < 0.000001);
    assert_json_f64_array(
        &record.metadata["detector_temperatures_reference_celsius"],
        &[43.22, 8.94, -4.97],
    );
    assert_json_f64_array(
        &record.metadata["integration_time_target_ms"],
        &[10.0, 16.0, 16.0],
    );
    assert_json_str_array(&record.metadata["foreoptic"], &["PROBE", "PROBE"]);
    assert_sed_signal_units(record, &["DN", "DN", "1"]);

    let reference = record.signals.get("norm__dn_ref_").expect("DN reference");
    assert_eq!(reference.signal_type, SignalType::RawCounts);
    assert_eq!(reference.unit.as_deref(), Some("DN"));

    let reflectance = record.signals.get("reflect__1_0").expect("reflectance");
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert_eq!(reflectance.unit.as_deref(), Some("1"));
    assert!((reflectance.values[0] - 0.18909).abs() < 0.000001);
}

fn assert_sed_signal_units(record: &nirs4all_io::SpectralRecord, expected: &[&str]) {
    let units = record.metadata["source_signal_units"]
        .as_array()
        .expect("source signal units");
    assert_eq!(units.len(), expected.len());
    for (unit, expected) in units.iter().zip(expected) {
        assert_eq!(unit.as_str(), Some(*expected));
    }
}

fn assert_json_f64_array(value: &serde_json::Value, expected: &[f64]) {
    let array = value.as_array().expect("json f64 array");
    assert_eq!(array.len(), expected.len());
    for (actual, expected) in array.iter().zip(expected) {
        assert!(
            (actual.as_f64().expect("f64") - expected).abs() < 0.000001,
            "actual={actual:?} expected={expected}"
        );
    }
}

fn assert_json_u64_array(value: &serde_json::Value, expected: &[u64]) {
    let array = value.as_array().expect("json u64 array");
    assert_eq!(array.len(), expected.len());
    for (actual, expected) in array.iter().zip(expected) {
        assert_eq!(actual.as_u64(), Some(*expected));
    }
}

fn assert_json_str_array(value: &serde_json::Value, expected: &[&str]) {
    let array = value.as_array().expect("json string array");
    assert_eq!(array.len(), expected.len());
    for (actual, expected) in array.iter().zip(expected) {
        assert_eq!(actual.as_str(), Some(*expected));
    }
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
    assert!(records[0]
        .quality_flags
        .contains(&"overlap_removed".to_string()));
    // overlap_removed and detector_overlap_preserved are mutually exclusive.
    assert!(!records[0]
        .quality_flags
        .contains(&"detector_overlap_preserved".to_string()));
    assert_eq!(
        records[0].metadata["overlap_policy"].as_str(),
        Some("remove")
    );
    let breakpoints = records[0].metadata["overlap_break_wavelengths_nm"]
        .as_array()
        .expect("overlap_break_wavelengths_nm");
    assert_eq!(breakpoints.len(), 2);
    assert!((breakpoints[0].as_f64().unwrap() - 970.0).abs() < 0.000001);
    assert!((breakpoints[1].as_f64().unwrap() - 1901.0).abs() < 0.000001);
    let matching_type = records[0].metadata["matching_type"]
        .as_str()
        .expect("matching_type");
    assert_eq!(matching_type, "Radiance @ 976 - 1010 / NIR-SWIR On");
}

#[test]
fn reads_svc_sig_laptop_firmware_variant() {
    let records = open_path(workspace_file("samples/svc_ger/BNL13001_000_laptop.sig"))
        .expect("open laptop sig");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "svc-ger-sig");
    assert_eq!(
        records[0].quality_flags,
        vec!["detector_overlap_preserved".to_string()]
    );
    assert_eq!(records[0].signals.len(), 3);
    for name in ["reference", "target", "reflectance"] {
        assert!(records[0].signals.contains_key(name), "missing {name}");
    }
    assert_svc_signal_units(&records[0]);
    assert_eq!(
        records[0].metadata["acquisition_start_date"].as_str(),
        Some("2017-07-29")
    );
    assert_eq!(
        records[0].metadata["acquisition_start_time"].as_str(),
        Some("01:54:23")
    );
    assert_eq!(
        records[0].metadata["acquisition_end_time"].as_str(),
        Some("01:55:32")
    );
    assert!(!records[0].metadata.contains_key("gps_latitude"));
    assert_eq!(
        records[0].metadata["instrument_model"].as_str(),
        Some("HR-1024i")
    );
    assert_eq!(
        records[0].metadata["instrument_serial"].as_str(),
        Some("6142041")
    );
    assert_eq!(
        records[0].metadata["overlap_policy"].as_str(),
        Some("preserve")
    );
    assert_eq!(records[0].metadata["matching_type"].as_str(), Some("None"));
    assert!(!records[0]
        .metadata
        .contains_key("overlap_break_wavelengths_nm"));

    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert_eq!(reflectance.axis.values.len(), 1_024);
    assert_eq!(reflectance.axis.unit, "nm");
    assert_eq!(reflectance.axis.kind, AxisKind::Wavelength);
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert_eq!(reflectance.unit.as_deref(), Some("%"));
    assert!((reflectance.axis.values[0] - 338.2).abs() < 0.000001);
    assert!((reflectance.axis.values[1_023] - 2517.2).abs() < 0.000001);
    assert!((reflectance.values[0] - 8.56).abs() < 0.000001);
    assert!((reflectance.values[1_023] - 2.55).abs() < 0.000001);

    let reference = records[0].signals.get("reference").expect("reference");
    assert_eq!(reference.signal_type, SignalType::Radiance);
    assert!((reference.values[0] - 469.43).abs() < 0.000001);
    let target = records[0].signals.get("target").expect("target");
    assert_eq!(target.signal_type, SignalType::Radiance);
    assert!((target.values[0] - 40.16).abs() < 0.000001);
}

#[test]
fn reads_svc_sig_second_laptop_firmware_variant() {
    let records = open_path(workspace_file("samples/svc_ger/BNL13002_000_laptop.sig"))
        .expect("open second laptop sig");

    assert_eq!(records.len(), 1);
    assert_svc_sig_triplet_record(&records[0], 338.2, 2517.2, &["detector_overlap_preserved"]);
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert!((reflectance.values[0] - 13.44).abs() < 0.000001);
    assert!((reflectance.values[1_023] - 5.61).abs() < 0.000001);
    assert!((reflectance.values.iter().sum::<f64>() - 22_957.18).abs() < 0.01);
    let reference = records[0].signals.get("reference").expect("reference");
    assert!((reference.values[0] - 469.43).abs() < 0.000001);
    let target = records[0].signals.get("target").expect("target");
    assert!((target.values[0] - 63.11).abs() < 0.000001);
}

#[test]
fn reads_svc_sig_clean_acer_pda_variant() {
    let records = open_path(workspace_file("samples/svc_ger/ACPL_D2_P1_B_1_001.sig"))
        .expect("open Acer PDA sig");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "svc-ger-sig");
    assert_eq!(
        records[0].quality_flags,
        vec!["detector_overlap_preserved".to_string()]
    );
    assert!(records[0].provenance.warnings.is_empty());
    assert_svc_signal_units(&records[0]);
    assert_eq!(
        records[0].metadata["acquisition_start_date"].as_str(),
        Some("2015-08-06")
    );
    assert_eq!(
        records[0].metadata["acquisition_end_time"].as_str(),
        Some("09:37:15")
    );
    assert_eq!(records[0].metadata["gps_time"].as_str(), Some("14:32:23"));
    assert_eq!(
        records[0].metadata["gps_end_time"].as_str(),
        Some("14:37:08")
    );
    assert!((records[0].metadata["gps_latitude"].as_f64().unwrap() - 46.679205).abs() < 0.000001);
    assert!(
        (records[0].metadata["gps_longitude"].as_f64().unwrap() + 92.51937833333333).abs()
            < 0.000001
    );
    assert_eq!(
        records[0].metadata["instrument_model"].as_str(),
        Some("HR-1024i")
    );
    assert_eq!(
        records[0].metadata["instrument_serial"].as_str(),
        Some("1152050")
    );
    let foreoptic = records[0].metadata["foreoptic"]
        .as_array()
        .expect("foreoptic array");
    assert_eq!(foreoptic.len(), 2);
    assert_eq!(foreoptic[0].as_str(), Some("FIBER1(2)"));
    assert_eq!(foreoptic[1].as_str(), Some("FIBER1(2)"));
    let factors = records[0].metadata["radiometric_factors"]
        .as_array()
        .expect("radiometric_factors array");
    assert_eq!(factors.len(), 3);
    assert!((factors[0].as_f64().unwrap() - 1.080).abs() < 0.000001);
    assert!((factors[1].as_f64().unwrap() - 1.133).abs() < 0.000001);
    assert!((factors[2].as_f64().unwrap() - 1.000).abs() < 0.000001);
    assert_eq!(
        records[0].metadata["overlap_policy"].as_str(),
        Some("preserve")
    );
    assert_eq!(records[0].metadata["matching_type"].as_str(), Some("None"));
    let ref_int = records[0].metadata["integration_time_reference_ms"]
        .as_array()
        .expect("integration_time_reference_ms");
    assert_eq!(ref_int.len(), 3);
    assert!((ref_int[0].as_f64().unwrap() - 70.0).abs() < 0.000001);
    let tgt_int = records[0].metadata["integration_time_target_ms"]
        .as_array()
        .expect("integration_time_target_ms");
    assert_eq!(tgt_int.len(), 3);
    assert!((tgt_int[0].as_f64().unwrap() - 200.0).abs() < 0.000001);
    let ref_temp = records[0].metadata["detector_temperatures_reference_celsius"]
        .as_array()
        .expect("detector_temperatures_reference_celsius");
    assert!((ref_temp[0].as_f64().unwrap() - 33.1).abs() < 0.000001);
    let ref_coadds = records[0].metadata["coadds_reference"]
        .as_array()
        .expect("coadds_reference");
    assert_eq!(ref_coadds[0].as_i64(), Some(28));
    let battery = records[0].metadata["battery_voltages_volts"]
        .as_array()
        .expect("battery_voltages_volts");
    assert_eq!(battery.len(), 2);
    assert!((battery[0].as_f64().unwrap() - 7.81).abs() < 0.000001);
    let errors = records[0].metadata["error_codes"]
        .as_array()
        .expect("error_codes");
    assert_eq!(errors, &vec![serde_json::json!(7), serde_json::json!(3)]);
    let slots = records[0].metadata["memory_slots"]
        .as_array()
        .expect("memory_slots");
    assert_eq!(slots, &vec![serde_json::json!(0), serde_json::json!(0)]);
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert_eq!(reflectance.axis.values.len(), 1_024);
    assert_eq!(reflectance.axis.unit, "nm");
    assert_eq!(reflectance.axis.kind, AxisKind::Wavelength);
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert!((reflectance.axis.values[0] - 340.5).abs() < 0.000001);
    assert!((reflectance.axis.values[1_023] - 2522.8).abs() < 0.000001);
    assert!((reflectance.values[0] - 6.13).abs() < 0.000001);
    assert!((reflectance.values[1_023] - 10.28).abs() < 0.000001);
}

#[test]
fn reads_svc_sig_clean_acer_fixture_set() {
    for (relative, first, last, sum) in [
        (
            "samples/svc_ger/ACPL_D2_P1_B_2_001.sig",
            6.13,
            9.58,
            23_971.81,
        ),
        (
            "samples/svc_ger/ACPL_D2_P1_M_1_000.sig",
            7.0,
            8.58,
            22_861.49,
        ),
        (
            "samples/svc_ger/ACPL_D2_P1_M_2_000.sig",
            9.63,
            9.68,
            24_036.92,
        ),
        (
            "samples/svc_ger/ACPL_D2_P1_T_1_000.sig",
            7.88,
            8.08,
            22_733.75,
        ),
        (
            "samples/svc_ger/ACPL_D2_P1_T_2_000.sig",
            12.25,
            8.88,
            23_292.20,
        ),
        (
            "samples/svc_ger/ACPL_F3_P2_B_1_000.sig",
            12.25,
            8.75,
            22_805.46,
        ),
    ] {
        let records = open_path(workspace_file(relative)).expect("open clean Acer sig");

        assert_eq!(records.len(), 1, "{relative}");
        assert_svc_sig_triplet_record(&records[0], 340.5, 2522.8, &["detector_overlap_preserved"]);
        let reflectance = records[0].signals.get("reflectance").expect("reflectance");
        assert!(
            (reflectance.values[0] - first).abs() < 0.000001,
            "{relative}"
        );
        assert!(
            (reflectance.values[1_023] - last).abs() < 0.000001,
            "{relative}"
        );
        assert!(
            (reflectance.values.iter().sum::<f64>() - sum).abs() < 0.01,
            "{relative}"
        );
    }
}

#[test]
fn reads_svc_sig_acer_white_reference_fixture() {
    let records = open_path(workspace_file("samples/svc_ger/ACPL_D2_P1_T_1_WR_000.sig"))
        .expect("open Acer white reference sig");

    assert_eq!(records.len(), 1);
    assert_svc_sig_triplet_record(
        &records[0],
        340.5,
        2522.8,
        &["detector_overlap_preserved", "white_reference"],
    );
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert!((reflectance.values[0] - 97.5).abs() < 0.000001);
    assert!((reflectance.values[1_023] - 100.5).abs() < 0.000001);
    assert!((reflectance.values.iter().sum::<f64>() - 102_439.28).abs() < 0.01);
    let reference = records[0].signals.get("reference").expect("reference");
    assert!((reference.values[0] - 1323.43).abs() < 0.000001);
    let target = records[0].signals.get("target").expect("target");
    assert!((target.values[0] - 1290.34).abs() < 0.000001);
    // Mean reflectance close to 100% confirms this is a white-reference capture.
    let mean_reflectance = reflectance.values.iter().sum::<f64>() / reflectance.values.len() as f64;
    assert!(
        (mean_reflectance - 100.0).abs() < 2.0,
        "mean reflectance {mean_reflectance}"
    );
}

fn assert_svc_sig_triplet_record(
    record: &nirs4all_io::SpectralRecord,
    first_axis: f64,
    last_axis: f64,
    expected_quality_flags: &[&str],
) {
    assert_eq!(record.provenance.format, "svc-ger-sig");
    let expected = expected_quality_flags
        .iter()
        .map(|flag| flag.to_string())
        .collect::<Vec<_>>();
    assert_eq!(record.quality_flags, expected);
    assert!(record.provenance.warnings.is_empty());
    assert_eq!(record.signals.len(), 3);
    for (name, signal_type) in [
        ("reference", SignalType::Radiance),
        ("target", SignalType::Radiance),
        ("reflectance", SignalType::Reflectance),
    ] {
        let signal = record.signals.get(name).expect(name);
        assert_eq!(signal.axis.values.len(), 1_024);
        assert_eq!(signal.axis.unit, "nm");
        assert_eq!(signal.axis.kind, AxisKind::Wavelength);
        assert!((signal.axis.values[0] - first_axis).abs() < 0.000001);
        assert!((signal.axis.values[1_023] - last_axis).abs() < 0.000001);
        assert_eq!(signal.signal_type, signal_type);
    }
    let reflectance = record.signals.get("reflectance").expect("reflectance");
    assert_eq!(reflectance.unit.as_deref(), Some("%"));
    assert_svc_signal_units(record);
}

fn assert_svc_signal_units(record: &nirs4all_io::SpectralRecord) {
    let units = record.metadata["source_signal_units"]
        .as_array()
        .expect("source signal units");
    assert_eq!(units.len(), 3);
    assert_eq!(units[0].as_str(), Some("Radiance"));
    assert_eq!(units[1].as_str(), Some("Radiance"));
    assert_eq!(units[2].as_str(), Some("%"));
}

#[test]
fn flags_declared_bad_svc_sig_fixtures() {
    for relative in [
        "samples/svc_ger/ACPL_D2_P1_B_1_000_BAD.sig",
        "samples/svc_ger/3_6_PANVI_2_T_1_001_BAD.sig",
    ] {
        let records = open_path(workspace_file(relative)).expect("open bad sig fixture");
        assert_eq!(records[0].provenance.format, "svc-ger-sig");
        assert!(records[0]
            .quality_flags
            .contains(&"declared_bad_fixture".to_string()));
        assert!(records[0]
            .provenance
            .warnings
            .contains(&"svc_sig_declared_bad_fixture".to_string()));
    }
}

#[test]
fn promotes_svc_sig_factors_metadata_for_resampled_export() {
    let records = open_path(workspace_file(
        "samples/svc_ger/serbinsh_BEO_CakeEater_Pheno_026_resamp.sig",
    ))
    .expect("open BEO resampled sig");

    assert_eq!(records.len(), 1);
    // Resampled spectra are overlap-removed exports and cannot keep raw overlap.
    assert!(records[0]
        .quality_flags
        .contains(&"resampled_export".to_string()));
    assert!(records[0]
        .quality_flags
        .contains(&"overlap_removed".to_string()));
    assert!(!records[0]
        .quality_flags
        .contains(&"detector_overlap_preserved".to_string()));
    assert_eq!(
        records[0].metadata["instrument_model"].as_str(),
        Some("HR-1024i")
    );
    assert_eq!(
        records[0].metadata["overlap_policy"].as_str(),
        Some("remove")
    );
    let breakpoints = records[0].metadata["overlap_break_wavelengths_nm"]
        .as_array()
        .expect("overlap_break_wavelengths_nm");
    assert_eq!(breakpoints.len(), 2);
    assert!((breakpoints[0].as_f64().unwrap() - 997.0).abs() < 0.000001);
    assert!((breakpoints[1].as_f64().unwrap() - 1901.0).abs() < 0.000001);
    let matching = records[0].metadata["matching_type"]
        .as_str()
        .expect("matching_type");
    assert!(matching.contains("987 - 1002"), "{matching}");
    let foreoptic = records[0].metadata["foreoptic"]
        .as_array()
        .expect("foreoptic array");
    assert_eq!(foreoptic[0].as_str(), Some("LENS 4(1)"));
    // Resampled exports always land on the canonical ASD 350-2500 nm grid at 1 nm step.
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert_eq!(reflectance.axis.values.len(), 2_151);
    let step = reflectance.axis.values[1] - reflectance.axis.values[0];
    assert!((step - 1.0).abs() < 0.000001, "step={step}");
}

#[test]
fn promotes_svc_sig_metadata_for_ger3700_pda() {
    let records = open_path(workspace_file("samples/svc_ger/serbinsh_gr070214_003.sig"))
        .expect("open GER 3700 sig");

    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].metadata["instrument_model"].as_str(),
        Some("HR-1024i")
    );
    assert_eq!(
        records[0].metadata["overlap_policy"].as_str(),
        Some("remove")
    );
    // GER 3700 raw export still passes through the SVC-style overlap policy.
    let factors = records[0].metadata["radiometric_factors"]
        .as_array()
        .expect("radiometric_factors");
    assert_eq!(factors.len(), 3);
    assert!((factors[0].as_f64().unwrap() - 0.993).abs() < 0.000001);
    assert!((factors[1].as_f64().unwrap() - 0.990).abs() < 0.000001);
    assert!((factors[2].as_f64().unwrap() - 1.000).abs() < 0.000001);
    let ref_int = records[0].metadata["integration_time_reference_ms"]
        .as_array()
        .expect("integration_time_reference_ms");
    assert_eq!(ref_int.len(), 3);
    assert!((ref_int[0].as_f64().unwrap() - 500.0).abs() < 0.000001);
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
            "samples/envi_sli/ecostress_a.spectrum.txt",
            "reflectance",
            "um",
            561,
            SignalType::Reflectance,
            0.3,
            8.82,
        ),
        (
            "samples/envi_sli/aster_granite.spectrum.txt",
            "reflectance",
            "um",
            2_844,
            SignalType::Reflectance,
            14.0112,
            7.2712,
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
fn reads_shimadzu_uvprobe_text_export() {
    let records =
        open_path(workspace_file("samples/shimadzu/synthetic_uvprobe.txt")).expect("open uvprobe");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "row-spectral-table");
    assert_eq!(
        records[0].metadata["notes"][0].as_str(),
        Some("Spectrum Data")
    );
    let sample = records[0].signals.get("sample_s000").expect("sample_s000");
    assert_eq!(sample.axis.values.len(), 200);
    assert_eq!(sample.axis.unit, "nm");
    assert_eq!(sample.axis.kind, AxisKind::Wavelength);
    assert_eq!(sample.signal_type, SignalType::Unknown);
    assert!((sample.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((sample.axis.values[199] - 2500.0).abs() < 0.000001);
    assert!((sample.values[0] - 0.036743).abs() < 0.000001);
    assert!((sample.values[199] + 0.146586).abs() < 0.000001);
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
fn reads_siware_api_csv_stream_as_row_table() {
    let records = open_path(workspace_file(
        "samples/siware_api/synthetic_siware_api.csv",
    ))
    .expect("open siware csv");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "row-spectral-table");
    assert_eq!(
        records[0].metadata["notes"][0].as_str(),
        Some("Spectro Inc. SiWare API CSV stream")
    );
    assert_eq!(
        records[0].metadata["notes"][1].as_str(),
        Some("measurement_id,meas-2026-05-18-001")
    );
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 200);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavelength);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((absorbance.axis.values[199] - 2500.0).abs() < 0.000001);
    assert!((absorbance.values[0] - 0.024871).abs() < 0.000001);
    assert!((absorbance.values[199] + 0.057148).abs() < 0.000001);
}

#[test]
fn reads_real_neospectra_ossl_wide_csv_slice() {
    let records = open_path(workspace_file(
        "samples/siware_neospectra/neospectra_ossl_50samples_slice.csv",
    ))
    .expect("open real neospectra ossl csv");

    assert_eq!(records.len(), 24);
    assert_eq!(records[0].provenance.format, "delimited-text");
    assert_eq!(
        records[0].metadata["sample_id"].as_str(),
        Some("a20c71b9ead451310a3d22317355ac57")
    );
    assert_eq!(
        records[0].metadata["dataset.code_ascii_txt"].as_str(),
        Some("Neospectra")
    );
    assert!(records[0].targets.contains_key("oc_usda.c729_w.pct"));
    let signal = records[0].signals.get("signal").expect("signal");
    assert_eq!(signal.axis.values.len(), 601);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert!((signal.axis.values[0] - 1350.0).abs() < 0.000001);
    assert!((signal.axis.values[600] - 2550.0).abs() < 0.000001);
    assert!((signal.values[0] - 0.37305).abs() < 0.000001);
}

#[test]
fn refuses_neospectra_ossl_schema_descriptor_as_non_spectral() {
    let err = open_path(workspace_file(
        "samples/siware_neospectra/neospectra_ossl_column_names.csv",
    ))
    .expect_err("OSSL column descriptor is not a spectrum");

    assert!(err
        .to_string()
        .contains("no numeric spectral headers found"));
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
    let err = open_path(workspace_file("samples/netcdf/air_temperature.nc"))
        .expect_err("non-NIRS NetCDF");
    assert!(err.to_string().contains("no spectra variable"));
}

#[test]
fn refuses_andi_ms_netcdf_chromatography() {
    let err = open_path(workspace_file("samples/andi_ms/gc01_0812_066.cdf"))
        .expect_err("ANDI/MS should be refused");
    let message = err.to_string();
    assert!(message.contains("ANDI/MS NetCDF chromatography"));
    assert!(message.contains("not NIRS spectroscopy"));
    assert!(message.contains("scan_acquisition_time"));
    assert!(message.contains("pyteomics.openms.ANDIMS"));
}

#[test]
fn reads_synthetic_nirs_hdf5_dataset() {
    let records = open_path(workspace_file("samples/hdf5/synthetic_nirs.h5")).expect("open hdf5");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "hdf5-nirs");
    assert_eq!(records[0].metadata["container"].as_str(), Some("hdf5"));
    assert_eq!(records[0].metadata["group_path"].as_str(), Some("/"));
    assert_eq!(records[0].metadata["sample_index"].as_u64(), Some(0));
    assert_eq!(
        records[0].metadata["signal_datasets"].as_array().unwrap(),
        &vec![
            serde_json::json!("spectra"),
            serde_json::json!("reflectance")
        ]
    );
    assert_eq!(
        records[0].metadata["signal_units"]["absorbance"].as_str(),
        Some("absorbance")
    );
    assert_eq!(
        records[0].metadata["signal_units"]["reflectance"].as_str(),
        Some("reflectance")
    );
    assert!(records[0].targets.contains_key("protein"));
    assert_eq!(records[0].signals.len(), 2);
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 200);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavelength);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((absorbance.axis.values[199] - 2500.0).abs() < 0.000001);
    assert!((absorbance.values[0] - 0.036742717027664185).abs() < 0.000001);
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert_eq!(reflectance.axis.values.len(), 200);
    assert_eq!(reflectance.axis.unit, "nm");
    assert_eq!(reflectance.axis.kind, AxisKind::Wavelength);
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert!((reflectance.values[0] - 0.91887677).abs() < 0.000001);
    assert!((reflectance.values[199] - 1.4014765).abs() < 0.000001);
}

#[test]
fn reads_hdf5_data_group_aliases_and_transposed_matrix() {
    let records = open_path(workspace_file("samples/hdf5/generic_aliases_data_group.h5"))
        .expect("open hdf5 aliases");

    assert_eq!(records.len(), 3);
    assert_eq!(records[0].provenance.format, "hdf5-nirs");
    assert_eq!(records[0].metadata["group_path"].as_str(), Some("/data"));
    assert_eq!(
        records[0].metadata["spectra_dataset"].as_str(),
        Some("absorbance")
    );
    assert_eq!(records[0].metadata["axis_dataset"].as_str(), Some("wn"));
    assert_eq!(
        records[0].metadata["matrix_orientation"].as_str(),
        Some("bands_by_samples")
    );
    assert_eq!(
        records[0].metadata["root_attributes"]["instrument"].as_str(),
        Some("synthetic-hdf5-aliases")
    );
    assert_eq!(records[0].targets["temperature"].as_f64(), Some(21.5));

    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 4);
    assert_eq!(absorbance.axis.unit, "cm-1");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavenumber);
    assert_eq!(absorbance.axis.order, AxisOrder::Descending);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 10000.0).abs() < 0.000001);
    assert!((absorbance.axis.values[3] - 4000.0).abs() < 0.000001);
    assert!((absorbance.values[0] - 0.10).abs() < 0.000001);
    assert!((absorbance.values[3] - 0.13).abs() < 0.000001);

    let third = records[2].signals.get("absorbance").expect("absorbance");
    assert!((third.values[0] - 0.30).abs() < 0.000001);
    assert!((third.values[3] - 0.33).abs() < 0.000001);
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
fn reads_fgi_xml_sidecar_with_hdf5_payload() {
    let records = open_path(workspace_file("samples/fgi/synthetic_fgi.xml")).expect("open fgi xml");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "fgi-hdf5-xml");
    assert_eq!(records[0].provenance.sources.len(), 2);
    assert_eq!(records[0].provenance.sources[0].role, "primary");
    assert_eq!(records[0].provenance.sources[1].role, "metadata_sidecar");
    assert_eq!(
        records[0].metadata["fgi_xml"]["instrument"].as_str(),
        Some("FGI-mock")
    );
    assert_eq!(
        records[0].metadata["fgi_xml"]["operator"].as_str(),
        Some("synthetic")
    );
    assert_eq!(
        records[0].metadata["fgi_data_reference"].as_str(),
        Some("synthetic_fgi.h5")
    );
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 200);
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
fn reads_local_indian_pines_matlab_cube_when_present() {
    let path = workspace_file("samples_local/hyperspectral_cubes/indian_pines_corrected.mat");
    if !path.exists() {
        eprintln!("skipping local-only Indian Pines MATLAB sample");
        return;
    }

    let records = open_path(path).expect("open local Indian Pines MAT cube");

    assert_eq!(records.len(), 145 * 145);
    assert_eq!(records[0].provenance.format, "matlab-indian-pines-cube");
    assert_eq!(
        records[0].metadata["container"].as_str(),
        Some("matlab_v5_hyperspectral_cube")
    );
    assert_eq!(
        records[0].metadata["dataset"].as_str(),
        Some("indian_pines_corrected")
    );
    assert_eq!(records[0].metadata["pixel_x"].as_u64(), Some(0));
    assert_eq!(records[0].metadata["pixel_y"].as_u64(), Some(0));
    assert_eq!(records[0].targets["land_cover_class"].as_u64(), Some(3));
    assert_eq!(records[0].provenance.sources.len(), 2);
    assert_eq!(records[0].provenance.sources[1].role, "target_sidecar");
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|warning| warning == "matlab_hyperspectral_cube_axis_generated_index"));

    let signal = records[0].signals.get("raw_counts").expect("raw_counts");
    assert_eq!(signal.axis.values.len(), 200);
    assert_eq!(signal.axis.unit, "index");
    assert_eq!(signal.axis.kind, AxisKind::Index);
    assert_eq!(signal.signal_type, SignalType::RawCounts);
    assert_eq!(signal.unit.as_deref(), Some("counts"));
    assert_eq!(signal.values[0], 3172.0);
    assert_eq!(signal.values[1], 4142.0);
    assert_eq!(signal.values[199], 1020.0);
    assert_eq!(signal.values.iter().sum::<f64>(), 533_141.0);

    let last = records.last().expect("last record");
    assert_eq!(last.metadata["pixel_x"].as_u64(), Some(144));
    assert_eq!(last.metadata["pixel_y"].as_u64(), Some(144));
    assert_eq!(last.targets["land_cover_class"].as_u64(), Some(0));
    assert_eq!(last.signals["raw_counts"].values[0], 3323.0);
    assert_eq!(last.signals["raw_counts"].values[199], 1000.0);
}

#[test]
fn reads_prospectr_nirsoil_rdata_dataset() {
    let records = open_path(workspace_file("samples/matlab/prospectr_NIRsoil.RData"))
        .expect("open NIRsoil RData");

    assert_eq!(records.len(), 825);
    assert_eq!(records[0].provenance.format, "rdata-prospectr-nirsoil");
    assert_eq!(
        records[0].metadata["dataset"].as_str(),
        Some("prospectr_NIRsoil")
    );
    assert_eq!(records[0].metadata["split"].as_str(), Some("train"));
    assert_eq!(records[0].metadata["train"].as_bool(), Some(true));
    assert_eq!(records[0].targets["Nt"].as_f64(), Some(0.3));
    assert_eq!(records[0].targets["Ciso"].as_f64(), Some(0.22));
    assert!(records[0].targets["CEC"].is_null());
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values.len(), 700);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavelength);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert!((absorbance.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((absorbance.axis.values[699] - 2498.0).abs() < 0.000001);
    assert!((absorbance.values[0] - 0.3386885).abs() < 0.000001);
    assert!((absorbance.values[699] - 0.3725677).abs() < 0.000001);

    assert_eq!(records[824].metadata["split"].as_str(), Some("test"));
    assert_eq!(records[824].metadata["train"].as_bool(), Some(false));
    assert_eq!(records[824].targets["Nt"].as_f64(), Some(8.0));
    assert!((records[824].targets["Ciso"].as_f64().expect("Ciso") - 7.7599998).abs() < 0.000001);
    assert!((records[824].targets["CEC"].as_f64().expect("CEC") - 46.2999992).abs() < 0.000001);
    assert!((records[824].signals["absorbance"].values[0] - 0.5835323).abs() < 0.000001);
    assert!((records[824].signals["absorbance"].values[699] - 0.7344803).abs() < 0.000001);
}

#[test]
fn reads_synthetic_excel_workbook() {
    for relative in [
        "samples/excel/synthetic_nirs.xlsx",
        "samples/excel/synthetic_nirs_macro_compatible.xlsm",
    ] {
        let records = open_path(workspace_file(relative)).expect("open excel");

        assert_eq!(records.len(), 50, "{relative}");
        assert_eq!(records[0].provenance.format, "excel-xlsx", "{relative}");
        assert_eq!(
            records[0].metadata["sample_id"].as_str(),
            Some("S000"),
            "{relative}"
        );
        assert_eq!(
            records[0].metadata["sheet"].as_str(),
            Some("spectra"),
            "{relative}"
        );
        assert_eq!(
            records[0].targets["protein"].as_f64(),
            Some(10.53211185428271),
            "{relative}"
        );
        let absorbance = records[0].signals.get("absorbance").expect("absorbance");
        assert_eq!(absorbance.axis.values.len(), 200, "{relative}");
        assert_eq!(absorbance.axis.unit, "nm", "{relative}");
        assert_eq!(absorbance.axis.kind, AxisKind::Wavelength, "{relative}");
        assert_eq!(absorbance.signal_type, SignalType::Absorbance, "{relative}");
        assert!((absorbance.axis.values[0] - 1100.0).abs() < 0.000001);
        assert!((absorbance.axis.values[199] - 2500.0).abs() < 0.000001);
        assert!((absorbance.values[0] - 0.03674271524932157).abs() < 0.000001);
    }
}

#[test]
fn reads_real_axis_descriptor_excel_workbooks() {
    for (
        relative,
        expected_len,
        sample_id,
        signal_name,
        signal_type,
        axis_len,
        first_axis,
        last_axis,
        first_value,
    ) in [
        (
            "samples/viavi_micronir/micronir_forensic_K_avg.xlsx",
            88,
            "K1",
            "absorbance",
            SignalType::Absorbance,
            125,
            908.1,
            1676.2,
            0.06214933,
        ),
        (
            "samples/viavi_micronir/micronir_forensic_T_avg.xlsx",
            71,
            "T1",
            "absorbance",
            SignalType::Absorbance,
            125,
            908.1,
            1676.2,
            0.4184331,
        ),
        (
            "samples/siware_neospectra/neospectra_forensic_K_avg.xlsx",
            88,
            "K1",
            "absorbance",
            SignalType::Absorbance,
            160,
            1299.36951243185,
            2604.09316651211,
            0.2083189,
        ),
        (
            "samples/excel/nirone_forensic_T_avg.xlsx",
            71,
            "T0",
            "absorbance",
            SignalType::Absorbance,
            201,
            1550.0,
            1950.0,
            0.4187897,
        ),
        (
            "samples/excel/scio_forensic_P_avg.xlsx",
            71,
            "P1",
            "raw",
            SignalType::RawCounts,
            331,
            740.0,
            1070.0,
            0.06505498,
        ),
    ] {
        let records = open_path(workspace_file(relative)).expect("open real descriptor xlsx");

        assert_eq!(records.len(), expected_len);
        assert_eq!(records[0].provenance.format, "excel-xlsx");
        assert_eq!(records[0].metadata["sample_id"].as_str(), Some(sample_id));
        assert!(records[0].metadata.contains_key("axis_descriptor"));
        let signal = records[0].signals.get(signal_name).expect(signal_name);
        assert_eq!(signal.axis.values.len(), axis_len);
        assert_eq!(signal.axis.unit, "nm");
        assert_eq!(signal.axis.kind, AxisKind::Wavelength);
        assert_eq!(signal.signal_type, signal_type);
        assert!((signal.axis.values[0] - first_axis).abs() < 0.000001);
        assert!((signal.axis.values[axis_len - 1] - last_axis).abs() < 0.000001);
        assert!((signal.values[0] - first_value).abs() < 0.000001);
    }
}

#[test]
fn reads_multisheet_excel_workbook() {
    let records = open_path(workspace_file(
        "samples/excel/synthetic_multisheet_nirs.xlsx",
    ))
    .expect("open multisheet xlsx");

    assert_eq!(records.len(), 4);
    assert_eq!(records[0].provenance.format, "excel-xlsx");
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("MS000"));
    assert_eq!(records[0].metadata["sheet"].as_str(), Some("spectra"));
    assert_eq!(
        records[0].metadata["metadata_sheet"].as_str(),
        Some("metadata")
    );
    assert_eq!(
        records[0].metadata["reference_sheet"].as_str(),
        Some("references")
    );
    assert_eq!(records[0].metadata["batch"].as_str(), Some("batch-a"));
    assert_eq!(records[0].metadata["operator"].as_str(), Some("ana"));
    assert_eq!(records[0].metadata["replicate"].as_f64(), Some(1.0));
    assert_eq!(records[0].targets["protein"].as_f64(), Some(10.2));
    assert_eq!(records[0].targets["moisture"].as_f64(), Some(6.1));
    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(absorbance.axis.values, vec![1100.0, 1200.0, 1300.0, 1400.0]);
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavelength);
    assert!((absorbance.values[0] - 0.101).abs() < 0.000001);
    assert!((absorbance.values[3] - 0.171).abs() < 0.000001);
}

#[test]
fn reads_pp_systems_row_tables_with_multiple_signals() {
    let records =
        open_path(workspace_file("samples/pp_systems/synthetic_unispec.SPT")).expect("open spt");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "row-spectral-table");
    assert_eq!(
        records[0].metadata["vendor"]["file"].as_str(),
        Some("synthetic_unispec.SPT")
    );
    assert_eq!(
        records[0].metadata["vendor"]["notes"].as_str(),
        Some("synthetic test fixture for nirs_loader")
    );
    let dn_white = records[0].signals.get("dn_white").expect("dn white");
    assert_eq!(dn_white.signal_type, SignalType::RawCounts);
    assert_eq!(dn_white.axis.values.len(), 200);
    assert_eq!(dn_white.axis.unit, "nm");
    assert_eq!(dn_white.axis.kind, AxisKind::Wavelength);
    assert!((dn_white.values[0] - 1500.0).abs() < 0.000001);
    assert!((dn_white.values[199] - 1500.0).abs() < 0.000001);
    let dn_target = records[0].signals.get("dn_target").expect("dn target");
    assert_eq!(dn_target.signal_type, SignalType::RawCounts);
    assert!((dn_target.values[0] - 1018.0).abs() < 0.000001);
    assert!((dn_target.values[199] - 927.0).abs() < 0.000001);
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert_eq!(reflectance.axis.values.len(), 200);
    assert_eq!(reflectance.axis.unit, "nm");
    assert_eq!(reflectance.axis.kind, AxisKind::Wavelength);
    assert!((reflectance.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((reflectance.axis.values[199] - 2500.0).abs() < 0.000001);
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert!((reflectance.values[0] - 0.6787).abs() < 0.000001);
    assert!((reflectance.values[199] - 0.6180).abs() < 0.000001);

    let records = open_path(workspace_file(
        "samples/pp_systems/synthetic_unispec_dc.SPU",
    ))
    .expect("open spu");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "row-spectral-table");
    assert_eq!(
        records[0].metadata["vendor"]["file"].as_str(),
        Some("synthetic_unispec_dc.SPU")
    );
    let channel_a = records[0]
        .signals
        .get("channel_a_dn")
        .expect("channel a dn");
    let channel_b = records[0]
        .signals
        .get("channel_b_dn")
        .expect("channel b dn");
    assert_eq!(channel_a.signal_type, SignalType::RawCounts);
    assert_eq!(channel_b.signal_type, SignalType::RawCounts);
    assert_eq!(channel_a.axis.values.len(), 200);
    assert_eq!(channel_a.axis.unit, "nm");
    assert_eq!(channel_a.axis.kind, AxisKind::Wavelength);
    assert!((channel_a.axis.values[0] - 1100.0).abs() < 0.000001);
    assert!((channel_a.axis.values[199] - 2500.0).abs() < 0.000001);
    assert!((channel_a.values[0] - 1018.0).abs() < 0.000001);
    assert!((channel_b.values[0] - 804.0).abs() < 0.000001);
    assert!((channel_b.values[199] - 838.0).abs() < 0.000001);
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert!((reflectance.values[0] - 1.2646).abs() < 0.000001);
    assert!((reflectance.values[199] - 1.1049).abs() < 0.000001);
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
    assert_eq!(stddev.signal_type, SignalType::Uncertainty);
    assert!((reflectance.values[0] - 0.042736).abs() < 0.000001);
}

#[test]
fn reads_usgs_aref_single_column_dump_with_index_axis() {
    let records =
        open_path(workspace_file("samples/envi_sli/usgs_liba_AREF.txt")).expect("open usgs aref");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "usgs-aref-single-column");
    assert_eq!(records[0].metadata["record_number"].as_u64(), Some(1));
    assert!(records[0]
        .provenance
        .warnings
        .contains(&"usgs_aref_axis_generated_index".to_string()));
    let reflectance = records[0].signals.get("reflectance").expect("reflectance");
    assert_eq!(reflectance.axis.kind, AxisKind::Index);
    assert_eq!(reflectance.axis.unit, "index");
    assert_eq!(reflectance.axis.values.len(), 24);
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert!((reflectance.values[0] - 0.33849356).abs() < 0.000001);
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
fn reads_metrohm_vision_air_csv_matrix_targets() {
    let records = open_path(workspace_file("samples/metrohm/synthetic_visionair.csv"))
        .expect("open Metrohm Vision Air CSV");

    assert_eq!(records.len(), 50);
    assert_eq!(records[0].provenance.format, "spectral-matrix");
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("S000"));
    assert_eq!(records[0].metadata["row_index"].as_u64(), Some(0));
    assert_eq!(
        records[0].metadata["vendor"]["title"].as_str(),
        Some("Vision Air Export")
    );
    assert_eq!(records[0].targets["protein"].as_f64(), Some(10.53));
    assert_eq!(records[0].targets["moisture"].as_f64(), Some(7.94));
    assert_eq!(records[0].targets["fat"].as_f64(), Some(1.83));

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
    assert_eq!(aot.signal_type, SignalType::AerosolOpticalThickness);
    assert!((aot.values[0] - 0.124).abs() < 0.000001);
    assert!((aot.values[2] - 0.211).abs() < 0.000001);
}

#[test]
fn reads_local_microtops_man_ascii_when_present() {
    let all_points = "samples_local/microtops/aeronet_man_Okeanos_19_2_all_points.lev20";
    let all_points_path = workspace_file(all_points);
    if !all_points_path.exists() {
        eprintln!("skipping local-only Microtops MAN ASCII samples");
        return;
    }

    for (relative, expected_len, expected_level, expected_aggregation, has_std) in [
        (
            "samples_local/microtops/aeronet_man_Okeanos_19_2_all_points.lev10",
            35,
            "1.0",
            "All Points",
            false,
        ),
        (
            "samples_local/microtops/aeronet_man_Okeanos_19_2_all_points.lev15",
            25,
            "1.5",
            "All Points",
            false,
        ),
        (all_points, 25, "2.0", "All Points", false),
        (
            "samples_local/microtops/aeronet_man_Okeanos_19_2_daily.lev15",
            5,
            "1.5",
            "Daily Averages",
            true,
        ),
        (
            "samples_local/microtops/aeronet_man_Okeanos_19_2_daily.lev20",
            5,
            "2.0",
            "Daily Averages",
            true,
        ),
        (
            "samples_local/microtops/aeronet_man_Okeanos_19_2_series.lev15",
            6,
            "1.5",
            "Series",
            true,
        ),
        (
            "samples_local/microtops/aeronet_man_Okeanos_19_2_series.lev20",
            6,
            "2.0",
            "Series",
            true,
        ),
    ] {
        let records = open_path(workspace_file(relative)).expect("open local Microtops MAN export");
        assert_eq!(records.len(), expected_len, "{relative}");
        assert_eq!(records[0].provenance.format, "microtops-man-ascii");
        assert_eq!(records[0].metadata["level"].as_str(), Some(expected_level));
        assert_eq!(
            records[0].metadata["aggregation"].as_str(),
            Some(expected_aggregation)
        );
        let aot = records[0].signals.get("aot").expect("aot");
        assert_eq!(aot.axis.values, vec![380.0, 440.0, 500.0, 675.0, 870.0]);
        assert_eq!(aot.axis.unit, "nm");
        assert_eq!(aot.signal_type, SignalType::AerosolOpticalThickness);
        assert_eq!(records[0].signals.contains_key("aot_std"), has_std);
    }

    let records = open_path(all_points_path).expect("open local Microtops MAN all-points export");

    assert_eq!(records.len(), 25);
    assert_eq!(records[0].provenance.format, "microtops-man-ascii");
    assert_eq!(records[0].metadata["level"].as_str(), Some("2.0"));
    assert_eq!(
        records[0].metadata["aggregation"].as_str(),
        Some("All Points")
    );
    assert_eq!(
        records[0].metadata["campaign"].as_str(),
        Some("Okeanos_19_2")
    );
    assert_eq!(records[0].metadata["aeronet_number"].as_f64(), Some(891.0));
    assert_eq!(
        records[0].metadata["microtops_number"].as_f64(),
        Some(19747.0)
    );
    assert_eq!(
        records[0].metadata["missing_aod_channels"]
            .as_array()
            .expect("missing channels")
            .len(),
        3
    );
    let aot = records[0].signals.get("aot").expect("aot");
    assert_eq!(aot.axis.values, vec![380.0, 440.0, 500.0, 675.0, 870.0]);
    assert_eq!(aot.axis.unit, "nm");
    assert_eq!(aot.unit.as_deref(), Some("1"));
    assert_eq!(aot.signal_type, SignalType::AerosolOpticalThickness);
    assert!((aot.values[0] - 0.095165).abs() < 0.000001);
    assert!((aot.values[4] - 0.05505).abs() < 0.000001);

    let daily = open_path(workspace_file(
        "samples_local/microtops/aeronet_man_Okeanos_19_2_daily.lev20",
    ))
    .expect("open local Microtops MAN daily export");
    assert_eq!(daily.len(), 5);
    assert_eq!(
        daily[0].metadata["aggregation"].as_str(),
        Some("Daily Averages")
    );
    assert_eq!(
        daily[0].metadata["number_of_observations"].as_f64(),
        Some(1.0)
    );
    let daily_std = daily[0].signals.get("aot_std").expect("aot_std");
    assert_eq!(daily_std.signal_type, SignalType::Uncertainty);
    assert_eq!(
        daily_std.axis.values,
        vec![380.0, 440.0, 500.0, 675.0, 870.0]
    );
    assert_eq!(daily_std.values, vec![0.0, 0.0, 0.0, 0.0, 0.0]);

    let series = open_path(workspace_file(
        "samples_local/microtops/aeronet_man_Okeanos_19_2_series.lev20",
    ))
    .expect("open local Microtops MAN series export");
    let series_std = series[0].signals.get("aot_std").expect("aot_std");
    assert_eq!(series_std.signal_type, SignalType::Uncertainty);
    assert_eq!(
        series_std.values,
        vec![0.003836, 0.003214, 0.003314, 0.006083, 0.00387]
    );
}

#[test]
fn reads_local_arm_mfrsr_netcdf_when_present() {
    let path = workspace_file("samples_local/mfr/arm_mfrsr_sgp_E11_20210329.nc");
    if !path.exists() {
        eprintln!("skipping local-only ARM MFRSR NetCDF sample");
        return;
    }

    let records = open_path(path).expect("open local ARM MFRSR NetCDF");

    assert_eq!(records.len(), 4_320);
    assert_eq!(records[0].provenance.format, "arm-mfrsr-netcdf");
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|warning| warning == "arm_mfrsr_netcdf_experimental"));
    assert_eq!(
        records[0].metadata["global_attributes"]["datastream"].as_str(),
        Some("sgpmfrsr7nchE11.b1")
    );
    assert_eq!(
        records[0].metadata["time_units"].as_str(),
        Some("seconds since 2021-03-29 00:00:00 0:00")
    );
    assert_eq!(records[0].metadata["time"].as_f64(), Some(25_200.0));
    assert_eq!(records[0].signals.len(), 6);

    let hemisp = records[0]
        .signals
        .get("hemispheric_irradiance")
        .expect("hemispheric signal");
    assert_eq!(
        hemisp.axis.values,
        vec![413.3, 501.0, 613.5, 671.4, 869.3, 939.4, 1624.2]
    );
    assert_eq!(hemisp.axis.unit, "nm");
    assert_eq!(hemisp.signal_type, SignalType::Irradiance);
    assert_eq!(hemisp.unit.as_deref(), Some("W/(m^2 nm)"));
    assert!((hemisp.values[0] - 0.0006153076537884772).abs() < 1e-15);
    assert!((hemisp.values[1] - 0.0005408665747381747).abs() < 1e-15);
    assert!((hemisp.values[6] + 0.0012479281285777688).abs() < 1e-15);

    let alltime = records[0]
        .signals
        .get("alltime_hemispheric_voltage")
        .expect("alltime signal");
    assert_eq!(alltime.signal_type, SignalType::RawCounts);
    assert_eq!(alltime.unit.as_deref(), Some("mV"));
    assert_eq!(
        records[0].metadata["qc_hemispheric_irradiance"]
            .as_array()
            .expect("qc row")
            .len(),
        7
    );

    let sidecar_sources = records[0]
        .provenance
        .sources
        .iter()
        .filter(|source| source.role == "qc_sidecar")
        .count();
    assert_eq!(sidecar_sources, 1);
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|warning| warning == "arm_mfrsr_qc_sidecar_loaded"));

    let incorrect = records
        .iter()
        .find(|record| record.metadata["time"].as_f64() == Some(62_800.0))
        .expect("incorrect sidecar time");
    assert!(incorrect.quality_flags.contains(
        &"arm_mfrsr_sidecar_diffuse_hemispheric_irradiance_filter4_incorrect".to_string()
    ));
    assert_eq!(
        incorrect.metadata["arm_mfrsr_qc_sidecar_flags"][0]["severity"].as_str(),
        Some("Incorrect")
    );
    assert_eq!(
        incorrect.metadata["arm_mfrsr_qc_sidecar_flags"][0]["reason"].as_str(),
        Some("Values are incorrect by visual inspection")
    );

    let suspect = records
        .iter()
        .find(|record| record.metadata["time"].as_f64() == Some(66_880.0))
        .expect("suspect sidecar time");
    assert!(suspect
        .quality_flags
        .contains(&"arm_mfrsr_sidecar_diffuse_hemispheric_irradiance_filter4_suspect".to_string()));
}

#[test]
fn reads_local_arm_surfspecalb_netcdf_when_present() {
    let path = workspace_file("samples_local/netcdf/arm_nsa_surfspecalb_20160609.nc");
    if !path.exists() {
        eprintln!("skipping local-only ARM SURFSPECALB NetCDF sample");
        return;
    }

    let records = open_path(path).expect("open local ARM SURFSPECALB NetCDF");

    assert_eq!(records.len(), 986);
    assert_eq!(records[0].provenance.format, "arm-surfspecalb-netcdf");
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|warning| warning == "arm_surfspecalb_netcdf_derived_product"));
    assert_eq!(records[0].metadata["sample_index"].as_u64(), Some(410));
    assert_eq!(records[0].metadata["time"].as_i64(), Some(410));

    let albedo = records[0]
        .signals
        .get("surface_albedo")
        .expect("surface albedo");
    assert_eq!(
        albedo.axis.values,
        vec![415.0, 500.0, 615.0, 673.0, 870.0, 940.0]
    );
    assert_eq!(albedo.axis.unit, "nm");
    assert_eq!(albedo.signal_type, SignalType::Reflectance);
    assert_eq!(albedo.unit.as_deref(), Some("1"));
    assert!((albedo.values[0] - 0.3362593352794647).abs() < 1e-12);
    assert!((albedo.values[5] - 0.5350303053855896).abs() < 1e-12);
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
fn reads_animl_synthetic_nirs_autoincrement_axis() {
    let records = open_path(workspace_file(
        "samples/animl/synthetic_nirs_autoincrement.animl",
    ))
    .expect("open animl autoincrement");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "animl");
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("S002"));
    assert_eq!(records[0].targets["protein"].as_f64(), Some(11.25));

    let absorbance = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(
        absorbance.axis.values,
        vec![1100.0, 1125.0, 1150.0, 1175.0, 1200.0]
    );
    assert_eq!(absorbance.axis.unit, "nm");
    assert_eq!(absorbance.axis.kind, AxisKind::Wavelength);
    assert_eq!(absorbance.signal_type, SignalType::Absorbance);
    assert_eq!(absorbance.unit.as_deref(), Some("AU"));
    assert_eq!(absorbance.values, vec![0.10, 0.12, 0.15, 0.14, 0.11]);
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
    let emission = records[0]
        .signals
        .get("fluorescence")
        .expect("fluorescence");
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|warning| warning.contains("asm_signal_label_derived_from_cube_context")));
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
fn reads_local_allotrope_adf_data_cubes_when_present() {
    let path = workspace_file("samples_local/allotrope_adf/adfsee_example.adf");
    if !path.exists() {
        return;
    }

    let records = open_path(path).expect("open local ADF");
    assert_eq!(records.len(), 4);

    let first = &records[0];
    assert_eq!(first.provenance.format, "allotrope-adf");
    assert_eq!(
        first.metadata["cube_id"].as_str(),
        Some("146cc0ae-64c5-4577-997b-bb56a2bab545")
    );
    assert_eq!(
        first.metadata["axis_source"].as_str(),
        Some("generated_index")
    );
    assert_eq!(first.metadata["cube_title"].as_str(), Some("double array"));
    let signal = first.signals.values().next().expect("ADF measure signal");
    assert_eq!(signal.axis.values.len(), 18_001);
    assert_eq!(signal.axis.unit, "index");
    assert_eq!(signal.axis.kind, AxisKind::Index);
    assert_eq!(signal.signal_type, SignalType::Unknown);
    assert!((signal.values[0] - 0.0).abs() < 0.000001);
    assert!((signal.values[18_000] - 0.147371).abs() < 0.000001);
    assert!(first
        .provenance
        .warnings
        .contains(&"allotrope_adf_rdf_semantics_partially_mapped".to_string()));

    let scaled = &records[1];
    assert_eq!(
        scaled.metadata["axis_source"].as_str(),
        Some("scale_dataset")
    );
    assert_eq!(scaled.metadata["secondary_index"].as_u64(), Some(0));
    assert_eq!(scaled.metadata["cube_title"].as_str(), Some("uv spectrum"));
    assert_eq!(
        scaled.metadata["adf_measure_component_type"].as_str(),
        Some("AbsorbanceUnitValue")
    );
    assert_eq!(
        scaled.metadata["adf_axis_component_type"].as_str(),
        Some("SecondTimeValue")
    );
    assert_eq!(
        scaled.metadata["secondary_scale_id"].as_str(),
        Some("a6643890-8173-42f9-9616-8d6f6589989b")
    );
    assert_eq!(
        scaled.metadata["secondary_axis_value"].as_f64(),
        Some(250.0)
    );
    assert_eq!(scaled.metadata["secondary_axis_unit"].as_str(), Some("nm"));
    assert_eq!(
        scaled.metadata["secondary_axis_kind"].as_str(),
        Some("wavelength")
    );
    let signal = scaled.signals.values().next().expect("scaled ADF signal");
    assert_eq!(signal.signal_type, SignalType::Absorbance);
    assert_eq!(signal.unit.as_deref(), Some("mAU"));
    assert_eq!(signal.axis.unit, "s");
    assert_eq!(signal.axis.kind, AxisKind::Time);
    assert!((signal.axis.values[0] - 0.0).abs() < 0.000001);
    assert!((signal.axis.values[18_000] - 15.002275).abs() < 0.000001);
    assert!(!scaled
        .provenance
        .warnings
        .contains(&"allotrope_adf_time_axis_mapped_as_index".to_string()));

    assert_eq!(
        records[2].metadata["secondary_axis_value"].as_f64(),
        Some(400.0)
    );
    assert_eq!(
        records[3].metadata["cube_title"].as_str(),
        Some("uv chromatogram")
    );
    let chromatogram = records[3].signals.get("absorbance").expect("absorbance");
    assert_eq!(chromatogram.signal_type, SignalType::Absorbance);
    assert_eq!(chromatogram.unit.as_deref(), Some("mAU"));
    assert_eq!(chromatogram.axis.unit, "s");
    assert_eq!(chromatogram.axis.kind, AxisKind::Time);
    assert!((chromatogram.axis.values[18_000] - 15.002275).abs() < 0.000001);
}

#[test]
fn rejects_target_only_reports_without_spectra() {
    for relative in [
        "samples/foss_winisi/synthetic_ds3_report.csv",
        "samples/perten/synthetic_perten.csv",
    ] {
        let err = open_path(workspace_file(relative)).expect_err("report has no spectrum");
        assert!(err
            .to_string()
            .contains("no numeric spectral headers found"));
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
    assert_eq!(signal.axis.kind, AxisKind::Energy);
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
    assert_eq!(signal.axis.kind, AxisKind::Energy);
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
    assert_eq!(signal.axis.kind, AxisKind::Energy);
    assert!((signal.axis.values[0] - 0.0).abs() < 0.000001);
    assert!((signal.axis.values[79] - 790.0).abs() < 0.000001);
    assert!((signal.values[0] - 65.820).abs() < 0.000001);
    assert!((signal.values[79] - 49.442).abs() < 0.000001);
}

#[test]
fn reads_msa_iso22029_nonconformant_metadata_as_preserved_headers() {
    for relative in [
        "samples/msa_iso22029/example1_wrong_date.msa",
        "samples/msa_iso22029/example1_wrong_date_empty_field.msa",
    ] {
        let records = open_path(workspace_file(relative)).expect("open msa metadata variant");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].provenance.format, "emsa-mas-msa");
        let signal = records[0].signals.get("counts").expect("counts");
        assert_eq!(signal.axis.values.len(), 20);
        assert_eq!(
            records[0].metadata["emsa_mas"]["date"][0].as_str(),
            Some("01-09-1991")
        );
        assert_eq!(
            records[0].metadata["emsa_mas"]["time"][0].as_str(),
            Some("12:100")
        );
        assert_eq!(
            records[0].provenance.warnings,
            ["msa_npoints_truncated: declared 20, parsed 21"]
        );
        assert!(records[0].quality_flags.is_empty());
    }

    let records = open_path(workspace_file(
        "samples/msa_iso22029/example1_wrong_date_empty_field.msa",
    ))
    .expect("open msa empty metadata variant");
    assert_eq!(
        records[0].metadata["emsa_mas"]["magcam"][0].as_str(),
        Some("")
    );
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
fn reads_top_level_multi_block_jcamp_dx_as_multiple_records() {
    let records =
        open_path(workspace_file("samples/jcamp_dx/nist_sucrose_ir.jdx")).expect("open sucrose");

    assert_eq!(records.len(), 2);
    for (index, record) in records.iter().enumerate() {
        assert_eq!(record.provenance.format, "jcamp-dx");
        assert_eq!(
            record.metadata["jcamp_block_index"].as_u64(),
            Some(index as u64)
        );
        let signal = record.signals.get("signal").expect("signal");
        assert_eq!(signal.axis.values.len(), 7_153);
        assert_eq!(signal.axis.unit, "cm-1");
        assert_eq!(signal.signal_type, SignalType::Reflectance);
        assert!((signal.axis.values[0] - 7_498.994).abs() < 0.000001);
        assert!((signal.axis.values[7_152] - 600.883992).abs() < 0.000001);
    }
    assert!((records[0].signals["signal"].values[0] - 0.422011).abs() < 0.000001);
    assert!((records[1].signals["signal"].values[0] - 0.471453).abs() < 0.000001);
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
    assert_eq!(real.axis.kind, AxisKind::Time);
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
fn reads_jcamp_synthetic_peak_assignments_as_sparse_record() {
    let records = open_path(workspace_file(
        "samples/jcamp_dx/synthetic_peak_assignments.jdx",
    ))
    .expect("open peak-assignments");

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.provenance.format, "jcamp-dx");
    assert_eq!(record.signals.len(), 1);

    let signal = record
        .signals
        .get("peak_intensity")
        .expect("peak_intensity signal");
    assert_eq!(signal.axis.unit, "cm-1");
    assert_eq!(signal.axis.kind, AxisKind::Wavenumber);
    assert_eq!(signal.signal_type, SignalType::Absorbance);
    assert_eq!(signal.axis.values, vec![3300.0, 2950.0, 1650.0, 1050.0]);
    assert_eq!(signal.values, vec![0.42, 0.18, 0.85, 0.55]);
    // Peaks are listed in descending wavenumber order; the axis recorder must
    // preserve that order verbatim rather than re-sort the sparse list.
    assert_eq!(signal.axis.order, AxisOrder::Descending);

    let table = record
        .metadata
        .get("jcamp_peak_table")
        .expect("jcamp_peak_table metadata");
    assert_eq!(table["kind"], "peak_assignments");
    assert_eq!(table["sparse"], true);
    assert_eq!(table["packed"], false);
    assert_eq!(table["peak_count"], 4);
    let peaks = table["peaks"].as_array().expect("peaks list");
    assert_eq!(peaks.len(), 4);
    assert_eq!(peaks[0]["assignment"], "O-H stretch, broad");
    assert_eq!(peaks[3]["assignment"], "C-O stretch");
    assert!(record.provenance.warnings.is_empty());
}

#[test]
fn reads_galactic_spc_single_even_axis() {
    let records = open_path(workspace_file("samples/galactic_spc/BENZENE.SPC")).expect("open spc");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].provenance.format, "galactic-spc");
    assert_eq!(
        records[0].metadata["galactic_spc"]["data_layout"].as_str(),
        Some("single_generated_x")
    );
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
    assert_eq!(
        records[0].metadata["galactic_spc"]["data_layout"].as_str(),
        Some("single_explicit_x")
    );
    let signal = records[0]
        .signals
        .get("arbitrary_intensity")
        .expect("arbitrary intensity");
    assert_eq!(signal.axis.values.len(), 512);
    assert_eq!(signal.axis.unit, "min");
    assert_eq!(signal.axis.kind, AxisKind::Time);
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
    assert_eq!(
        records[0].metadata["galactic_spc"]["data_layout"].as_str(),
        Some("multi_common_generated_x")
    );
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
    assert_eq!(
        records[0].metadata["galactic_spc"]["data_layout"].as_str(),
        Some("multi_independent_xyxy")
    );
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

#[test]
fn reads_galactic_spc_multi_generated_x_variants() {
    let records = open_path(workspace_file("samples/galactic_spc/m_evenz.spc"))
        .expect("open multi even-z spc");

    assert_eq!(records.len(), 32);
    assert_eq!(records[0].metadata["sample_id"].as_str(), Some("subfile_1"));
    let header = &records[0].metadata["galactic_spc"];
    assert_eq!(header["version"].as_str(), Some("new_lsb_0x4b"));
    assert_eq!(
        header["data_layout"].as_str(),
        Some("multi_common_generated_x")
    );
    assert_eq!(header["flags"]["tmulti"].as_bool(), Some(true));
    assert_eq!(header["flags"]["txvals"].as_bool(), Some(false));
    let signal = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(signal.axis.values.len(), 171);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.signal_type, SignalType::Absorbance);
    assert!((signal.axis.values[0] - 200.0).abs() < 0.000001);
    assert!((signal.axis.values[170] - 800.0).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 4.61097).abs() < 0.000001);
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|warning| warning == "invalid_spc_integer_exponent_255_treated_as_0"));
}

#[test]
fn reads_galactic_spc_old_ordered_z_variant() {
    let records =
        open_path(workspace_file("samples/galactic_spc/m_ordz.spc")).expect("open old ordz spc");

    assert_eq!(records.len(), 10);
    let header = &records[0].metadata["galactic_spc"];
    assert_eq!(header["version"].as_str(), Some("old_lsb_0x4d"));
    assert_eq!(
        header["data_layout"].as_str(),
        Some("multi_common_generated_x")
    );
    assert_eq!(header["flags"]["tmulti"].as_bool(), Some(true));
    assert_eq!(header["flags"]["tordrd"].as_bool(), Some(true));
    let signal = records[0].signals.get("absorbance").expect("absorbance");
    assert_eq!(signal.axis.values.len(), 857);
    assert_eq!(signal.axis.unit, "cm-1");
    assert_eq!(signal.axis.kind, AxisKind::Wavenumber);
    assert!((signal.axis.values[0] - 698.229736328125).abs() < 0.000001);
    assert!((signal.axis.values[856] - 4000.354736328125).abs() < 0.000001);
    assert!((signal.values[0] - 0.02219367027282715).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 12.425798).abs() < 0.000001);
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|warning| warning.contains("old_spc_header_limited")));
}

#[test]
fn reads_galactic_spc_directory_backed_mass_spectra() {
    let records = open_path(workspace_file("samples/galactic_spc/DRUG_SAMPLE.SPC"))
        .expect("open drug sample spc");

    assert_eq!(records.len(), 400);
    let header = &records[0].metadata["galactic_spc"];
    assert_eq!(
        header["data_layout"].as_str(),
        Some("multi_independent_xyxy")
    );
    assert_eq!(header["flags"]["tmulti"].as_bool(), Some(true));
    assert_eq!(header["flags"]["txyxys"].as_bool(), Some(true));
    assert_eq!(header["flags"]["txvals"].as_bool(), Some(true));
    let signal = records[0]
        .signals
        .get("arbitrary_intensity")
        .expect("intensity");
    assert_eq!(signal.axis.values.len(), 60);
    assert_eq!(signal.axis.unit, "m/z");
    assert_eq!(signal.signal_type, SignalType::RawCounts);
    assert!((signal.axis.values[0] - 34_659.0).abs() < 0.000001);
    assert!((signal.axis.values[59] - 33_848.399993896484).abs() < 0.000001);
    assert!((signal.values.iter().sum::<f64>() - 245_071.0).abs() < 0.000001);
    assert!(records[0].metadata["galactic_spc_subfile"]["directory"].is_object());
}

#[test]
fn reads_galactic_spc_adjacent_nmr_fid_without_promoting_scope() {
    let records =
        open_path(workspace_file("samples/galactic_spc/NMR_FID.SPC")).expect("open fid spc");

    assert_eq!(records.len(), 1);
    let header = &records[0].metadata["galactic_spc"];
    assert_eq!(header["version"].as_str(), Some("new_lsb_0x4b"));
    assert_eq!(header["experiment_type"].as_str(), Some("General SPC"));
    let signal = records[0]
        .signals
        .get("arbitrary_intensity")
        .expect("fid intensity");
    assert_eq!(signal.axis.values.len(), 16_384);
    assert_eq!(signal.axis.unit, "s");
    assert_eq!(signal.axis.kind, AxisKind::Time);
    assert_eq!(signal.signal_type, SignalType::RawCounts);
    assert!((signal.axis.values[16_383] - 0.3268608).abs() < 0.000001);
    assert_eq!(signal.values[0], 0.0);
    assert_eq!(signal.values[16_383], -139_836.0);
}

#[test]
fn refuses_witec_wip_binary_projects() {
    let mut path = std::env::temp_dir();
    path.push(format!("nirs4all-io-witec-wip-{}.wip", std::process::id()));
    std::fs::write(&path, b"WIT^\0\0\0\0synthetic").expect("write synthetic wip");

    let err = open_path(&path).expect_err("WiTec WIP must be refused");
    let _ = std::fs::remove_file(&path);

    match err {
        Error::InvalidRecord(message) => {
            assert!(message.contains("legacy WiTec WIP/WID WIT^ project layout"));
            assert!(message.contains("current native subset is limited to the WIT_PR06 TDGraph"));
            assert!(message.contains("export other WiTec projects from WiTec Project/FIVE"));
        }
        other => panic!("unexpected error: {other}"),
    }
}

fn asd_metadata(relative: &str) -> Value {
    let records = open_path(workspace_file(relative)).unwrap_or_else(|err| {
        panic!("open {relative}: {err}");
    });
    records[0].metadata["asd"].clone()
}

fn count_asd_blocks(blocks: &[Value], kind: &str) -> usize {
    blocks
        .iter()
        .filter(|block| block["kind"].as_str() == Some(kind))
        .count()
}

fn find_asd_block<'a>(blocks: &'a [Value], kind: &str) -> &'a Value {
    blocks
        .iter()
        .find(|block| block["kind"].as_str() == Some(kind))
        .unwrap_or_else(|| panic!("missing ASD block {kind}"))
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
