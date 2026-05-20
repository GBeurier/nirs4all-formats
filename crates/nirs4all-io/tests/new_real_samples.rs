use nirs4all_io::{open_path, AxisKind, SignalType};

#[test]
fn reads_real_usgs_envi_spectral_libraries() {
    for (relative, expected_records, first_sample, first_value) in [
        (
            "samples/envi_sli/usgs_splib06a_aviris95_envi.hdr",
            1_365,
            "Acmite NMNH133746 Pyroxene s06av95a=a",
            0.04158623889088631,
        ),
        (
            "samples/envi_sli/usgs_splib06a_aviris95_envi.sli",
            1_365,
            "Acmite NMNH133746 Pyroxene s06av95a=a",
            0.04158623889088631,
        ),
        (
            "samples/envi_sli/usgs_splib07_aviris95_envi.hdr",
            3_139,
            "Wavelengths in microns 224ch AVIRIS95.1",
            0.38314998149871826,
        ),
        (
            "samples/envi_sli/usgs_splib07_aviris95_envi.sli",
            3_139,
            "Wavelengths in microns 224ch AVIRIS95.1",
            0.38314998149871826,
        ),
    ] {
        let records = open_path(workspace_file(relative)).expect("open ENVI SLI");

        assert_eq!(records.len(), expected_records, "{relative}");
        assert_eq!(records[0].provenance.format, "envi-sli");
        assert_eq!(
            records[0].metadata["sample_id"].as_str(),
            Some(first_sample),
            "{relative}"
        );
        let signal = records[0].signals.get("spectrum").expect("spectrum");
        assert_eq!(signal.axis.values.len(), 224);
        assert_eq!(signal.axis.unit, "um");
        assert_eq!(signal.axis.kind, AxisKind::Wavelength);
        assert_close(signal.axis.values[0], 0.38315);
        assert_close(signal.axis.values[223], 2.5082);
        assert_close(signal.values[0], first_value);
    }
}

#[test]
fn reads_additional_bruker_opus_sed_and_sig_samples() {
    for (relative, first_absorbance) in [
        ("samples/bruker_opus/icr_087266_B2.0", 0.14134784042835236),
        ("samples/bruker_opus/icr_087273_G3.0", 0.18761110305786133),
    ] {
        let records = open_path(workspace_file(relative)).expect("open OPUS");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].provenance.format, "bruker-opus");
        assert_eq!(records[0].signals.len(), 4);
        let absorbance = records[0].signals.get("absorbance").expect("absorbance");
        assert_eq!(absorbance.axis.values.len(), 3_578);
        assert_eq!(absorbance.axis.unit, "cm-1");
        assert_eq!(absorbance.signal_type, SignalType::Absorbance);
        assert_close(absorbance.axis.values[0], 7_498.059202194214);
        assert_close(absorbance.axis.values[3_577], 599.7675956487656);
        assert_close(absorbance.values[0], first_absorbance);
        assert!(records[0].provenance.warnings.is_empty());
    }

    let sed = open_path(workspace_file(
        "samples/spectral_evolution/serbinsh_cvars_grape_leaf.sed",
    ))
    .expect("open SED");
    assert_eq!(sed[0].provenance.format, "spectral-evolution-sed");
    let reflect = sed[0].signals.get("reflect__1_0").expect("reflect");
    assert_eq!(reflect.axis.values.len(), 2_151);
    assert_close(reflect.axis.values[0], 350.0);
    assert_close(reflect.axis.values[2_150], 2_500.0);
    assert_close(reflect.values[0], 0.18909);
    assert_close(sed[0].metadata["gps_latitude"].as_f64().unwrap(), 33.52465);
    assert_close(
        sed[0].metadata["gps_longitude"].as_f64().unwrap(),
        -116.16258,
    );
    assert_close(sed[0].metadata["gps_altitude_m"].as_f64().unwrap(), 22.20);
    assert_eq!(
        sed[0].metadata["acquisition_start_date"].as_str(),
        Some("2013-06-08")
    );
    assert_eq!(
        sed[0].metadata["acquisition_start_time"].as_str(),
        Some("10:57:51")
    );
    assert_eq!(sed[0].metadata["gps_time"].as_str(), Some("15:57:11"));
    assert_eq!(sed[0].metadata["gps_satellites_used"].as_u64(), Some(7));
    assert_eq!(sed[0].metadata["gps_satellites_visible"].as_u64(), Some(11));

    for (relative, axis_len, first_axis, last_axis, first_reflectance) in [
        (
            "samples/svc_ger/serbinsh_BEO_CakeEater_Pheno_026_resamp.sig",
            2_151,
            350.0,
            2_500.0,
            1.93,
        ),
        (
            "samples/svc_ger/serbinsh_gr070214_003.sig",
            991,
            337.9,
            2_514.3,
            8.73,
        ),
    ] {
        let records = open_path(workspace_file(relative)).expect("open SIG");
        assert_eq!(records[0].provenance.format, "svc-ger-sig");
        let reflectance = records[0].signals.get("reflectance").expect("reflectance");
        assert_eq!(reflectance.axis.values.len(), axis_len, "{relative}");
        assert_eq!(reflectance.axis.unit, "nm");
        assert_eq!(reflectance.signal_type, SignalType::Reflectance);
        assert_close(reflectance.axis.values[0], first_axis);
        assert_close(reflectance.axis.values[axis_len - 1], last_axis);
        assert_close(reflectance.values[0], first_reflectance);
    }
}

#[test]
fn reads_scio_csv_exports() {
    let app = open_path(workspace_file("samples/scio/scio_app_scan.csv")).expect("open SCiO app");
    assert_eq!(app.len(), 1);
    assert_eq!(app[0].provenance.format, "scio-csv");
    let spectrum = app[0].signals.get("spectrum").expect("spectrum");
    assert_eq!(spectrum.axis.values.len(), 331);
    assert_eq!(spectrum.axis.unit, "nm");
    assert_close(spectrum.axis.values[0], 740.0);
    assert_close(spectrum.axis.values[330], 1_070.0);
    assert_close(spectrum.values[0], 1.499853245);

    let tech = open_path(workspace_file(
        "samples/scio/scio_scans_from_tech_support.csv",
    ))
    .expect("open SCiO developer export");
    assert_eq!(tech.len(), 145);
    assert_eq!(tech[0].provenance.format, "scio-csv");
    assert_eq!(
        tech[0].metadata["sample_id"].as_str(),
        Some("0875e771-ccb4-4449-80ec-bb761610d968")
    );
    assert!(tech[0].targets.contains_key("protein"));
    for name in ["spectrum", "wr_raw", "sample_raw"] {
        let signal = tech[0].signals.get(name).expect(name);
        assert_eq!(signal.axis.values.len(), 331, "{name}");
        assert_close(signal.axis.values[0], 740.0);
        assert_close(signal.axis.values[330], 1_070.0);
    }
    assert_close(tech[0].signals["spectrum"].values[0], 0.619431817);
    assert_close(tech[0].signals["wr_raw"].values[0], 7_973.843642);
    assert_close(tech[0].signals["sample_raw"].values[0], 4_939.252455);

    let calibration = open_path(workspace_file(
        "samples/scio/scio_calibration_plate_Polypen.csv",
    ))
    .expect("open SCiO calibration plate");
    assert_eq!(calibration.len(), 1);
    assert_eq!(calibration[0].provenance.format, "row-spectral-table");
    let reflectance = calibration[0]
        .signals
        .get("reflectance")
        .expect("reflectance");
    assert_eq!(reflectance.axis.values.len(), 256);
    assert_eq!(reflectance.axis.unit, "nm");
    assert_eq!(reflectance.signal_type, SignalType::Reflectance);
    assert_close(reflectance.axis.values[0], 324.0);
    assert_close(reflectance.axis.values[255], 790.0);
    assert_close(reflectance.values[0], 1.06959706959707);
    assert_close(reflectance.values[255], 1.014440433213);
}

#[test]
fn reads_microtops_man_netcdf_and_refuses_pyrnet() {
    let records = open_path(workspace_file(
        "samples/microtops/microtops_arc_msm114_2.nc",
    ))
    .expect("open Microtops MAN NetCDF");
    assert_eq!(records.len(), 378);
    assert_eq!(records[0].provenance.format, "microtops-man-netcdf");
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|warning| warning == "microtops_man_netcdf_experimental"));
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|warning| warning == "microtops_man_netcdf_known_fixture_layout"));
    let aot = records[0].signals.get("aot").expect("aot");
    assert_eq!(aot.axis.values, vec![380.0, 440.0, 500.0, 675.0, 870.0]);
    assert_eq!(aot.unit.as_deref(), Some("1"));
    assert_close(aot.values[0], 0.262656);
    assert_close(aot.values[4], 0.195324);
    let aot_std = records[0].signals.get("aot_std").expect("aot_std");
    assert_eq!(aot_std.axis.values, vec![380.0, 440.0, 500.0, 675.0, 870.0]);
    assert_eq!(aot_std.unit.as_deref(), Some("1"));
    assert_close(aot_std.values[0], 0.004276);
    assert_close(aot_std.values[4], 0.005922);
    assert_close(records[0].metadata["lat"].as_f64().unwrap(), 10.836353);
    assert_eq!(records[0].metadata["number_obs"].as_i64(), Some(11));
    assert_eq!(
        records[0].metadata["global_attributes"]["platform"].as_str(),
        Some("RV Maria S. Merian")
    );
    assert_eq!(
        records[0].metadata["global_attributes"]["conventions"].as_str(),
        Some("CF-1.7")
    );
    assert_eq!(
        records[0].metadata["time_units"].as_str(),
        Some("seconds since 2023-01-17T12:19:00")
    );
    assert_eq!(
        records[0].metadata["time_calendar"].as_str(),
        Some("proleptic_gregorian")
    );

    let last = records.last().expect("last");
    let last_aot = last.signals.get("aot").expect("aot");
    assert_close(last.metadata["lat"].as_f64().unwrap(), -47.244);
    assert_close(last.metadata["lon"].as_f64().unwrap(), -59.7812);
    assert_close(last_aot.values[0], 0.066577);
    assert_close(last_aot.values[4], 0.048534);

    let err = open_path(workspace_file("samples/netcdf/pyrnet_to_l1a_output.nc"))
        .expect_err("PyrNet must not be accepted as spectroscopy");
    let message = err.to_string();
    assert!(message.contains("not a supported NIRS spectroscopy schema"));
    assert!(message.contains("no Microtops aot_* channel set"));
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.000001,
        "actual {actual} != expected {expected}"
    );
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
