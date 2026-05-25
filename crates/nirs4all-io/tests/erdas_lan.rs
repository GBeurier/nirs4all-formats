use nirs4all_io::{
    open_path, open_path_with_options, AxisKind, CubeMask, CubeWindow, ReadOptions, SignalType,
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
    // G2 (2026-05-23): the `erdas_lan_aviris_experimental` flag is a
    // format-wide status carried only by the first record, not
    // repeated 21,025 times.
    assert!(!last
        .provenance
        .warnings
        .contains(&"erdas_lan_aviris_experimental".to_string()));

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

#[test]
fn reads_aviris_indian_pines_erdas_lan_sparse_mask() {
    let path = workspace_file("samples/hyperspectral_cubes/92AV3C.lan");
    let full = open_path(&path).expect("open AVIRIS LAN");

    let pixels = vec![(0, 0), (72, 36), (144, 144), (10, 20)];
    let mask = CubeMask::new(pixels.clone());
    let options = ReadOptions::default().with_cube_mask(mask);
    let records = open_path_with_options(&path, &options).expect("open AVIRIS LAN mask");

    assert_eq!(records.len(), pixels.len());
    for (record, &(row, col)) in records.iter().zip(&pixels) {
        assert_eq!(
            record.metadata["sample_id"].as_str(),
            Some(format!("pixel_y{row}_x{col}").as_str())
        );
        assert_eq!(record.metadata["x_index"].as_u64(), Some(col as u64));
        assert_eq!(record.metadata["y_index"].as_u64(), Some(row as u64));
        let full_index = row * 145 + col;
        assert_eq!(
            record.signals["raw_counts"].values,
            full[full_index].signals["raw_counts"].values
        );
        assert_eq!(
            record.targets["land_cover_class"],
            full[full_index].targets["land_cover_class"]
        );
    }

    let duplicated =
        ReadOptions::default().with_cube_mask(CubeMask::new(vec![(5, 5), (5, 5), (5, 6)]));
    let dup_records = open_path_with_options(&path, &duplicated).expect("open AVIRIS LAN dup mask");
    assert_eq!(dup_records.len(), 3);
    assert_eq!(
        dup_records[0].signals["raw_counts"].values,
        dup_records[1].signals["raw_counts"].values
    );
    assert_ne!(
        dup_records[0].signals["raw_counts"].values,
        dup_records[2].signals["raw_counts"].values
    );

    let empty = ReadOptions::default().with_cube_mask(CubeMask::new(Vec::new()));
    let err = open_path_with_options(&path, &empty).expect_err("empty mask");
    assert!(err.to_string().contains("ERDAS LAN cube mask is empty"));

    let out_of_bounds =
        ReadOptions::default().with_cube_mask(CubeMask::new(vec![(0, 0), (145, 0)]));
    let err = open_path_with_options(path, &out_of_bounds).expect_err("out of bounds mask");
    assert!(err
        .to_string()
        .contains("mask pixel (145, 0) is outside 0..145 x 0..145"));
}

#[test]
fn reads_aviris_indian_pines_as_single_nd_cube() {
    let path = workspace_file("samples/hyperspectral_cubes/92AV3C.lan");
    let per_pixel = open_path(&path).expect("open AVIRIS LAN");

    let options = ReadOptions::default().single_record();
    let records = open_path_with_options(&path, &options).expect("open AVIRIS LAN single cube");

    // One N-D record instead of 21,025 pixel records.
    assert_eq!(records.len(), 1);
    let signal = &records[0].signals["raw_counts"];
    assert_eq!(signal.shape, vec![145, 145, 220]);
    assert_eq!(signal.dims, vec!["row", "col", "x"]);
    assert_eq!(signal.values.len(), 145 * 145 * 220);
    // Spectral axis is the band axis.
    assert_eq!(signal.axis.values.len(), 220);
    assert_eq!(signal.axis.kind, AxisKind::Wavelength);
    // Row/col coordinates carry the absolute pixel indices.
    assert_eq!(signal.coords["row"].values.len(), 145);
    assert_eq!(signal.coords["col"].values.len(), 145);
    assert_eq!(signal.coords["row"].values[0], 0.0);
    assert_eq!(signal.coords["col"].values[144], 144.0);
    // The cube is C-order [row][col][band]: pixel (0,0) is the first 220
    // values and must match the per-pixel record.
    assert_eq!(
        signal.values[..220].to_vec(),
        per_pixel[0].signals["raw_counts"].values
    );
    // Ground-truth labels are preserved as a 2-D grid in metadata.
    let grid = records[0].metadata["land_cover_class_grid"]
        .as_array()
        .unwrap();
    assert_eq!(grid.len(), 145);
    assert_eq!(grid[0].as_array().unwrap().len(), 145);

    // A rectangular window yields a sub-cube; a sparse mask is refused.
    let windowed = ReadOptions::default()
        .single_record()
        .with_cube_window(CubeWindow::new(10, Some(12), 20, Some(23)));
    let sub = open_path_with_options(&path, &windowed).expect("windowed single cube");
    assert_eq!(sub[0].signals["raw_counts"].shape, vec![2, 3, 220]);
    assert_eq!(
        sub[0].signals["raw_counts"].coords["row"].values,
        vec![10.0, 11.0]
    );

    let masked = ReadOptions::default()
        .single_record()
        .with_cube_mask(CubeMask::new(vec![(0, 0), (1, 1)]));
    let err = open_path_with_options(&path, &masked).expect_err("mask incompatible with N-D");
    assert!(err
        .to_string()
        .contains("sparse pixel mask is incompatible"));
}

fn workspace_file(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
