use nirs4all_io::{
    open_path, open_path_with_options, AxisKind, CubeWindow, ReadOptions, SignalType,
};

#[test]
fn reads_aviris_indian_pines_erdas_lan_cube() {
    let records = open_path(workspace_file("samples/hyperspectral_cubes/92AV3C.lan"))
        .expect("open AVIRIS LAN");

    assert_eq!(records.len(), 21_025);
    let first = &records[0];
    assert_eq!(first.provenance.format, "erdas-lan-aviris");
    assert_eq!(first.signal_type, SignalType::RawCounts);
    assert_eq!(first.metadata["sample_id"].as_str(), Some("pixel_y0_x0"));
    assert_eq!(first.metadata["x_index"].as_u64(), Some(0));
    assert_eq!(first.metadata["y_index"].as_u64(), Some(0));
    assert_eq!(first.targets["land_cover_class"].as_u64(), Some(3));
    assert_eq!(first.provenance.sources.len(), 3);
    assert_eq!(first.provenance.sources[0].role, "primary");
    assert_eq!(first.provenance.sources[1].role, "wavelength_sidecar");
    assert_eq!(first.provenance.sources[2].role, "ground_truth_sidecar");
    assert!(first
        .provenance
        .warnings
        .contains(&"erdas_lan_aviris_experimental".to_string()));
    assert!(first
        .provenance
        .warnings
        .contains(&"erdas_lan_spc_axis_non_monotonic_native_order".to_string()));

    let signal = first.signals.get("raw_counts").expect("raw_counts");
    assert_eq!(signal.signal_type, SignalType::RawCounts);
    assert_eq!(signal.unit.as_deref(), Some("dn"));
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    assert_eq!(signal.axis.unit, "nm");
    assert_eq!(signal.axis.values.len(), 220);
    assert!((signal.axis.values[0] - 400.019989).abs() < 0.000001);
    assert!((signal.axis.values[219] - 2498.959961).abs() < 0.000001);
    assert_eq!(
        &signal.values[..5],
        &[3172.0, 4142.0, 4506.0, 4279.0, 4782.0]
    );
    assert!((signal.values.iter().sum::<f64>() - 554_098.0).abs() < 0.000001);

    let last = records.last().expect("last");
    assert_eq!(last.metadata["sample_id"].as_str(), Some("pixel_y144_x144"));
    assert_eq!(last.metadata["x_index"].as_u64(), Some(144));
    assert_eq!(last.metadata["y_index"].as_u64(), Some(144));
    assert_eq!(last.targets["land_cover_class"].as_u64(), Some(0));

    let options =
        ReadOptions::default().with_cube_window(CubeWindow::new(10, Some(12), 20, Some(22)));
    let roi = open_path_with_options(
        workspace_file("samples/hyperspectral_cubes/92AV3C.lan"),
        &options,
    )
    .expect("open AVIRIS LAN ROI");

    assert_eq!(roi.len(), 4);
    let first_roi = &roi[0];
    assert_eq!(
        first_roi.metadata["sample_id"].as_str(),
        Some("pixel_y10_x20")
    );
    assert_eq!(first_roi.metadata["x_index"].as_u64(), Some(20));
    assert_eq!(first_roi.metadata["y_index"].as_u64(), Some(10));
    let full_index = 10 * 145 + 20;
    assert_eq!(
        first_roi.targets["land_cover_class"],
        records[full_index].targets["land_cover_class"]
    );
    assert_eq!(
        first_roi.signals["raw_counts"].values,
        records[full_index].signals["raw_counts"].values
    );
    assert_eq!(
        roi.last().unwrap().metadata["sample_id"].as_str(),
        Some("pixel_y11_x21")
    );
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
