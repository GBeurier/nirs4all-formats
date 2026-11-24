use std::path::{Path, PathBuf};
use std::sync::Arc;

use nirs4all_formats::{
    builtin_probes, open_bytes, open_bytes_with_options, open_path, open_path_with_options,
    open_with_sidecars, open_with_sidecars_and_options, sidecar_requirements, AxisKind,
    InMemorySidecars, ReadOptions, SignalType, SourceFile, SpectralRecord,
};

const DATA: &[u8] = include_bytes!("../../../samples/foss_nsas_da/synthetic.da");
const SPC: &[u8] = include_bytes!("../../../samples/foss_nsas_da/synthetic.spc");

#[test]
fn registry_memory_path_metadata_and_full_export_match() {
    let probes = builtin_probes(DATA, Path::new("renamed.bin"));
    assert_eq!(probes[0].format, "foss-nsas-da");
    assert!(builtin_probes(b"arbitrary bytes", Path::new("extension_only.DA")).is_empty());
    let records = open_bytes("renamed.bin", DATA).unwrap();
    let exported = open_bytes("export.spc", SPC).unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(exported.len(), records.len());
    for (row, (record, reference)) in records.iter().zip(&exported).enumerate() {
        assert_eq!(
            record.signals["absorbance"].axis,
            reference.signals["absorbance"].axis
        );
        assert_eq!(
            record.signals["absorbance"].values,
            reference.signals["absorbance"].values
        );
        let signal = &record.signals["absorbance"];
        assert_eq!(signal.axis.kind, AxisKind::Wavelength);
        assert_eq!(signal.axis.unit, "nm");
        assert_eq!(signal.values.len(), 1050);
        assert_eq!(
            signal.values[1049],
            0.5 + row as f64 * 0.125 + 1049.0 / 4096.0
        );
        assert_eq!(record.signal_type, SignalType::Absorbance);
        assert!(record.targets.is_empty());
        assert_eq!(record.metadata["record_index"], row);
        assert_eq!(record.metadata["scan_count"], 32);
        assert_eq!(
            record.metadata["sensor_values"],
            serde_json::json!([-0.25, 0.5, 5.75])
        );
        assert_eq!(
            record.metadata["da_header_hex"].as_str().unwrap().len(),
            256
        );
        assert_eq!(record.metadata["da_trailer_hex"], "20".repeat(24));
        assert_eq!(record.provenance.sources[0].sha256.len(), 64);
        assert_eq!(record.provenance.warnings.len(), 3);
    }
    assert_eq!(records[0].metadata["sample_id"], "DEMO-A");
    assert_eq!(records[1].metadata["sample_id"], "DEMO-A");
    assert_eq!(records[2].metadata["vendor_record_number"], 2);
    assert_eq!(records[0].metadata["acquired_local"], "2000-02-29T12:00:30");
    assert!(!records[0].metadata.contains_key("acquired_utc"));
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/foss_nsas_da/synthetic.da");
    let disk = open_path(path).unwrap();
    assert_eq!(disk[2].signals, records[2].signals);
    assert_eq!(disk[2].metadata, records[2].metadata);
}

#[test]
fn rejects_truncation_corrupt_records_unknown_layout_and_nonfinite_values() {
    for size in [0, 2, 127, 128, 4328, 4351, DATA.len() - 1] {
        assert!(open_bytes("bad.da", &DATA[..size]).is_err());
    }
    for offset in [0, 14, 16, 18, 20, 22, 52, 54, 56, 58, 60, 4352, 4352 + 14] {
        let mut bad = DATA.to_vec();
        bad[offset] ^= 0xff;
        assert!(open_bytes("bad.da", &bad).is_err());
    }
    for offset in [128, 4352 + 128, 80] {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut bad = DATA.to_vec();
            bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(open_bytes("bad.da", &bad).is_err());
        }
    }
    let mut extra = DATA.to_vec();
    extra.push(0);
    assert!(open_bytes("extra.da", &extra).is_err());
}

#[test]
fn preserves_unknown_fields_and_invalid_dates() {
    let mut bytes = DATA.to_vec();
    // February 29 in a non-leap century is invalid; retain the raw words.
    bytes[68..70].copy_from_slice(&1900_u16.to_le_bytes());
    bytes[100] = 0xaa;
    bytes[4328] = 0xbb;
    let records = open_bytes("date.da", &bytes).unwrap();
    let record = &records[0];
    assert!(!record.metadata.contains_key("acquired_local"));
    assert_eq!(record.metadata["da_date_time_words"][2], 1900);
    assert!(record
        .provenance
        .warnings
        .iter()
        .any(|w| w.starts_with("foss_nsas_da_invalid_date:")));
    assert_eq!(
        &record.metadata["da_header_hex"].as_str().unwrap()[200..202],
        "aa"
    );
    assert!(record.metadata["da_trailer_hex"]
        .as_str()
        .unwrap()
        .starts_with("bb"));
}

#[test]
#[ignore = "requires the private Rapitec DA/SPC pair in samples_local/foss_nsas_da"]
fn local_pair_all_78750_values_match_the_spc_payload() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples_local/foss_nsas_da");
    let bytes = std::fs::read(root.join("0LC1B44.DA")).unwrap();
    let spc = std::fs::read(root.join("0LC1B44.SPC")).unwrap();
    let records = open_bytes("0LC1B44.DA", &bytes).unwrap();
    assert_eq!(records.len(), 75);
    // The source SPC incorrectly declares seven subfiles. A test-only header
    // correction exposes all physical blocks to the independent SPC decoder.
    assert_eq!(u32::from_le_bytes(spc[24..28].try_into().unwrap()), 7);
    assert_eq!(
        u32::from_le_bytes(spc[248..252].try_into().unwrap()),
        512 + 75 * 4232
    );
    let mut corrected = spc.clone();
    corrected[24..28].copy_from_slice(&75_u32.to_le_bytes());
    let reference = open_bytes("corrected.spc", &corrected).unwrap();
    assert_eq!(reference.len(), 75);
    for (index, record) in records.iter().enumerate() {
        assert_eq!(
            record.signals["absorbance"].axis,
            reference[index].signals["absorbance"].axis
        );
        assert_eq!(
            record.signals["absorbance"].values,
            reference[index].signals["absorbance"].values
        );
        assert_eq!(
            &bytes[index * 4352 + 128..index * 4352 + 4328],
            &spc[544 + index * 4232..4744 + index * 4232]
        );
    }
    assert_eq!(records[74].metadata["record_index"], 74);
}

#[test]
#[ignore = "requires the three private DA files in samples_local/foss_nsas_da"]
fn local_three_file_corpus_preserves_all_200550_values_and_original_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples_local/foss_nsas_da");
    let mut total = 0;
    for (name, count, sample_count, expected_sha) in [
        (
            "0LC1B44.DA",
            75,
            15,
            "6e2b5651aa163e37b6720ffd8301cf1f16dc9303cf3f07f5064552d6242ca235",
        ),
        (
            "0LC1B70.DA",
            60,
            12,
            "79310867fdc464cc54ddfb4d388f4d8c79a3df15634862c7e3006c04147b2339",
        ),
        (
            "0LC1B83.DA",
            56,
            11,
            "269fc5daf406bcafa2cd7842e7db3aefb97da335389335ac7638148de12df6ea",
        ),
    ] {
        let bytes = std::fs::read(root.join(name)).unwrap();
        let records = open_bytes(name, &bytes).unwrap();
        assert_eq!(records.len(), count);
        let ids: std::collections::BTreeSet<_> = records
            .iter()
            .map(|r| r.metadata["sample_id"].as_str().unwrap())
            .collect();
        assert_eq!(ids.len(), sample_count);
        let mut reconstructed = Vec::new();
        for (index, record) in records.iter().enumerate() {
            assert_eq!(record.metadata["record_index"], index);
            assert_eq!(record.provenance.sources[0].sha256, expected_sha);
            let signal = &record.signals["absorbance"];
            assert_eq!(signal.values.len(), 1050);
            assert_eq!(signal.axis.values.first(), Some(&400.0));
            assert_eq!(signal.axis.values.last(), Some(&2498.0));
            assert!(signal.values.iter().all(|v| v.is_finite()));
            assert!(record.targets.is_empty());
            // This proves preservation of uninterpreted fields too. It is a
            // lossless structural check, not a reference-calibration comparison.
            let decode_hex = |key: &str| {
                let text = record.metadata[key].as_str().unwrap();
                (0..text.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
                    .collect::<Vec<_>>()
            };
            reconstructed.extend(decode_hex("da_header_hex"));
            for value in &signal.values {
                reconstructed.extend((*value as f32).to_le_bytes());
            }
            reconstructed.extend(decode_hex("da_trailer_hex"));
            total += signal.values.len();
        }
        assert_eq!(reconstructed, bytes, "lossless reconstruction of {name}");
        let disk = open_path(root.join(name)).unwrap();
        for (memory, path) in records.iter().zip(&disk) {
            assert_eq!(memory.signals, path.signals);
            assert_eq!(memory.metadata, path.metadata);
        }
    }
    assert_eq!(total, 200_550);
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples/foss_nsas_da")
        .join(name)
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    std::fs::read(fixture(name)).unwrap()
}

fn set_word(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn get_word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn from_hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn metadata_bytes(record: &SpectralRecord, key: &str) -> Vec<u8> {
    from_hex(record.metadata[key].as_str().unwrap())
}

fn reconstruct_da(records: &[SpectralRecord]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for record in records {
        bytes.extend(metadata_bytes(record, "da_header_hex"));
        for value in &record.signals.values().next().unwrap().values {
            bytes.extend((*value as f32).to_le_bytes());
        }
        bytes.extend(metadata_bytes(record, "da_trailer_hex"));
    }
    bytes
}

fn reconstruct_cn(records: &[SpectralRecord]) -> Vec<u8> {
    let mut bytes = metadata_bytes(&records[0], "cn_header_hex");
    for record in records {
        bytes.extend(metadata_bytes(record, "cn_record_hex"));
    }
    bytes
}

fn paired(da: &[u8], cn: &[u8]) -> nirs4all_formats::Result<Vec<SpectralRecord>> {
    open_with_sidecars(
        "references700.da",
        da,
        Arc::new(InMemorySidecars::new().with("references700.cn", cn.to_vec())),
    )
}

fn set_counts(da: &mut [u8], count: u16) {
    let points = usize::from(get_word(da, 14));
    let stride = (128 + points * 4).div_ceil(128) * 128;
    for row in da.chunks_exact_mut(stride) {
        set_word(row, 30, count);
    }
}

#[test]
fn supported_variants_use_their_axes_alignment_and_unknown_signal_semantics() {
    for (name, count, points, start, end, step, stride) in [
        ("references700.da", 3, 700, 1100.0, 2500.0, 2.0, 2944),
        ("variant1700.da", 2, 1700, 800.0, 1650.0, 0.5, 7040),
        ("variant4200.da", 2, 4200, 400.0, 2500.0, 0.5, 17024),
    ] {
        let bytes = fixture_bytes(name);
        let records = open_bytes("renamed.bin", &bytes).unwrap();
        assert_eq!(records.len(), count);
        assert_eq!(bytes.len(), count * stride);
        assert_eq!(
            builtin_probes(&bytes, Path::new("renamed.bin"))[0].format,
            "foss-nsas-da"
        );
        for record in &records {
            assert_eq!(record.signal_type, SignalType::Unknown);
            let signal = &record.signals["spectrum"];
            assert_eq!(signal.signal_type, SignalType::Unknown);
            assert_eq!(signal.role, "spectrum");
            assert_eq!(signal.unit, None);
            assert_eq!(signal.axis.kind, AxisKind::Wavelength);
            assert_eq!(signal.axis.unit, "nm");
            assert_eq!(signal.values.len(), points);
            assert_eq!(signal.axis.values.first(), Some(&start));
            assert_eq!(signal.axis.values.last(), Some(&(end - step)));
            assert!(signal.axis.values.windows(2).all(|v| v[1] - v[0] == step));
            assert_eq!(record.metadata["da_point_count"], points);
            assert_eq!(record.metadata["da_wavelength_start"], start);
            assert_eq!(record.metadata["da_wavelength_end"], end);
            assert_eq!(record.metadata["da_wavelength_step"], step);
            assert_eq!(record.metadata["da_record_bytes"], stride);
            assert!(record.targets.is_empty());
        }
        assert_eq!(reconstruct_da(&records), bytes);
        assert!(open_bytes(name, &bytes[..bytes.len() - 1]).is_err());
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(open_bytes(name, &extra).is_err());
    }
}

#[test]
fn paired_cn_targets_match_all_entry_points_and_preserve_both_sources() {
    let da = fixture_bytes("references700.da");
    let cn = fixture_bytes("references700.cn");
    let records = paired(&da, &cn).unwrap();
    assert_eq!(records.len(), 3);
    let missing = open_bytes("references700.da", &da).unwrap();
    for record in &missing {
        assert!(record.targets.is_empty());
        assert!(record
            .provenance
            .warnings
            .iter()
            .any(|w| w.starts_with("foss_nsas_da_missing_cn")));
    }
    let requirements = sidecar_requirements("logical/references700.da", &da);
    assert_eq!(requirements.len(), 1);
    assert!(!requirements[0].required);
    assert!(requirements[0].path.ends_with(".cn") || requirements[0].path.ends_with(".CN"));
    let disk = open_path(fixture("references700.da")).unwrap();
    let nested = open_with_sidecars(
        "logical/references700.da",
        &da,
        Arc::new(InMemorySidecars::new().with("references700.cn", cn.clone())),
    )
    .unwrap();
    let upper = open_with_sidecars(
        "references700.DA",
        &da,
        Arc::new(InMemorySidecars::new().with("references700.CN", cn.clone())),
    )
    .unwrap();
    for (index, record) in records.iter().enumerate() {
        assert_eq!(record.metadata["da_constituent_count"], 2);
        assert_eq!(record.targets.len(), 2);
        for (column, key, name) in [(0, "water", "WATER"), (1, "oil", "Oil")] {
            let offset = 384 + index * 128 + column * 4;
            let expected = f32::from_le_bytes(cn[offset..offset + 4].try_into().unwrap()) as f64;
            assert_eq!(record.targets[key], expected);
            let independent_expected = [[1.25, 0.0], [0.0, 3.25], [2.5, 1.5]];
            assert_eq!(record.targets[key], independent_expected[index][column]);
            let slot = &record.metadata["cn_constituents"][column];
            assert_eq!(slot["index"], column);
            assert_eq!(slot["name"], name);
            assert_eq!(slot["target_key"], key);
            assert_eq!(
                from_hex(slot["name_hex"].as_str().unwrap()),
                cn[column * 10..column * 10 + 10]
            );
        }
        assert_eq!(record.provenance.sources.len(), 2);
        assert_eq!(
            record.provenance.sources[0].sha256,
            SourceFile::from_bytes("x", &da, "primary").sha256
        );
        let source = record
            .provenance
            .sources
            .iter()
            .find(|s| s.role == "constituent_reference")
            .unwrap();
        assert_eq!(
            source.sha256,
            SourceFile::from_bytes("x", &cn, "constituent_reference").sha256
        );
        for equivalent in [&disk[index], &nested[index], &upper[index]] {
            assert_eq!(record.signals, equivalent.signals);
            assert_eq!(record.targets, equivalent.targets);
            assert_eq!(record.metadata, equivalent.metadata);
        }
        assert!(!record
            .provenance
            .warnings
            .iter()
            .any(|w| w.starts_with("foss_nsas_da_missing_cn")));
    }
    assert_eq!(reconstruct_da(&records), da);
    assert_eq!(reconstruct_cn(&records), cn);
    assert!(nested[0]
        .provenance
        .sources
        .iter()
        .any(|s| s.role == "constituent_reference" && s.path == "logical/references700.cn"));
}

#[test]
fn zero_single_and_multiple_reference_counts_do_not_invent_targets() {
    let original = fixture_bytes("references700.da");
    let mut cn = fixture_bytes("references700.cn");
    // Zero and negative values are valid references, not missing-value sentinels.
    cn[384..388].copy_from_slice(&0.0_f32.to_le_bytes());
    cn[388..392].copy_from_slice(&(-2.5_f32).to_le_bytes());
    for count in [0, 1, 2, 32] {
        let mut da = original.clone();
        set_counts(&mut da, count);
        let records = paired(&da, &cn).unwrap();
        for record in &records {
            assert_eq!(record.targets.len(), usize::from(count));
            assert_eq!(record.metadata["da_constituent_count"], count);
        }
        if count > 0 {
            assert_eq!(records[0].targets["water"], 0.0);
        }
        if count >= 2 {
            assert_eq!(records[0].targets["oil"], -2.5);
        }
        assert_eq!(reconstruct_cn(&records), cn);
        assert_eq!(reconstruct_da(&records), da);
        if count == 0 {
            let absent = open_bytes("references700.da", &da).unwrap();
            assert!(!absent[0]
                .provenance
                .warnings
                .iter()
                .any(|w| w.starts_with("foss_nsas_da_missing_cn")));
        }
    }
}

#[test]
fn cn_names_cannot_overwrite_values_and_opaque_bytes_survive() {
    let mut da = fixture_bytes("references700.da");
    set_counts(&mut da, 4);
    let mut cn = fixture_bytes("references700.cn");
    for (i, name) in [b"WATER     ", b"water     ", b"water_2   ", b"          "]
        .iter()
        .enumerate()
    {
        cn[i * 10..i * 10 + 10].copy_from_slice(*name);
    }
    for row in 0..3 {
        for column in 0..4 {
            let offset = 384 + 128 * row + 4 * column;
            cn[offset..offset + 4].copy_from_slice(&((column + 1) as f32).to_le_bytes());
        }
    }
    cn[320] = 0xff;
    let records = paired(&da, &cn).unwrap();
    for record in &records {
        assert_eq!(record.targets.len(), 4);
        let mut values: Vec<_> = record
            .targets
            .values()
            .map(|v| v.as_f64().unwrap() as u32)
            .collect();
        values.sort_unstable();
        assert_eq!(values, [1, 2, 3, 4]);
        assert_eq!(record.targets["water"], 1.0);
        assert_eq!(record.targets["water_2"], 2.0);
        let slots = record.metadata["cn_constituents"].as_array().unwrap();
        let keys: std::collections::BTreeSet<_> = slots
            .iter()
            .map(|s| s["target_key"].as_str().unwrap())
            .collect();
        assert_eq!(keys.len(), 4);
    }
    assert_eq!(reconstruct_cn(&records), cn);
    cn[..10].copy_from_slice(&[0x80; 10]);
    let records = paired(&da, &cn).unwrap();
    assert_eq!(
        records[0].metadata["cn_constituents"][0]["target_key"],
        "constituent_1"
    );
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|w| w.contains("name") || w.contains("encoding")));
    assert_eq!(reconstruct_cn(&records), cn);
}

#[test]
fn cn_rejects_malformed_active_data_but_preserves_inactive_nonfinite_words() {
    let da = fixture_bytes("references700.da");
    let cn = fixture_bytes("references700.cn");
    for size in [0, 383, 384, 384 + 128, cn.len() - 1] {
        assert!(
            paired(&da, &cn[..size]).is_err(),
            "accepted CN length {size}"
        );
    }
    let mut extra = cn.clone();
    extra.extend([0; 128]);
    assert!(paired(&da, &extra).is_err());
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut bad = cn.clone();
        bad[384..388].copy_from_slice(&value.to_le_bytes());
        assert!(paired(&da, &bad).is_err());
        let mut opaque = cn.clone();
        opaque[384 + 8..384 + 12].copy_from_slice(&value.to_le_bytes());
        let records = paired(&da, &opaque).unwrap();
        assert_eq!(records[0].targets.len(), 2);
        assert_eq!(reconstruct_cn(&records), opaque);
        let mut no_targets = da.clone();
        set_counts(&mut no_targets, 0);
        let records = paired(&no_targets, &bad).unwrap();
        assert!(records[0].targets.is_empty());
        assert_eq!(reconstruct_cn(&records), bad);
    }
    let aliased = Arc::new(
        InMemorySidecars::new()
            .with("references700.cn", cn.clone())
            .with("references700.CN", cn.clone()),
    );
    let records = open_with_sidecars("references700.da", &da, aliased).unwrap();
    assert_eq!(records[0].targets["water"], 1.25);
    let mut other = cn.clone();
    other[384..388].copy_from_slice(&9.0f32.to_le_bytes());
    let ambiguous = Arc::new(
        InMemorySidecars::new()
            .with("references700.cn", cn)
            .with("references700.CN", other),
    );
    assert!(open_with_sidecars("references700.da", &da, ambiguous).is_err());
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("references700.da"), &da).unwrap();
    std::fs::create_dir(dir.path().join("references700.cn")).unwrap();
    assert!(open_path(dir.path().join("references700.da")).is_err());
}

#[test]
fn rejects_unsupported_geometry_counts_codes_and_nonterminal_nul_ids() {
    let da = fixture_bytes("references700.da");
    for (offset, value) in [
        (0, 1234),
        (14, 0),
        (14, 3101),
        (16, 2500),
        (18, 1100),
        (20, 12),
        (22, 7),
        (30, 33),
        (52, 3),
        (54, 99),
        (56, 2),
        (58, 7),
        (60, 7),
    ] {
        let mut bad = da.clone();
        set_word(&mut bad, offset, value);
        assert!(
            open_bytes("bad.da", &bad).is_err(),
            "accepted word{offset}={value}"
        );
    }
    let mut changed_count = da.clone();
    set_word(&mut changed_count, 2944 + 30, 1);
    assert!(paired(&changed_count, &fixture_bytes("references700.cn")).is_err());
    let mut mixed_geometry = da[..2944].to_vec();
    mixed_geometry.extend(&DATA[..4352]);
    assert!(open_bytes("mixed.da", &mixed_geometry).is_err());
    let mut invalid_id = da.clone();
    invalid_id[4..14].copy_from_slice(b"A\0B       ");
    assert!(open_bytes("bad.da", &invalid_id).is_err());
    invalid_id[4..14].fill(b' ');
    assert!(open_bytes("bad.da", &invalid_id).is_err());
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut bad = da.clone();
        bad[128..132].copy_from_slice(&value.to_le_bytes());
        assert!(open_bytes("bad.da", &bad).is_err());
    }
}

#[test]
fn preserves_mixed_model_codes_nul_padding_opaque_words_and_nonfinite_sensors() {
    let mut da = fixture_bytes("references700.da");
    for (row, model) in [0, 5000, 6500].into_iter().enumerate() {
        set_word(&mut da, row * 2944, model);
        set_word(&mut da, row * 2944 + 62, 0x9abc);
        da[row * 2944 + 4..row * 2944 + 14].copy_from_slice(b" SAMPLE\0\0\0");
    }
    da[80..84].copy_from_slice(&f32::from_bits(0x7fc0_1234).to_le_bytes());
    da[84..88].copy_from_slice(&f32::INFINITY.to_le_bytes());
    da[88..92].copy_from_slice(&(-4.5_f32).to_le_bytes());
    let records = open_bytes("models.da", &da).unwrap();
    for (row, model) in [0, 5000, 6500].into_iter().enumerate() {
        assert_eq!(records[row].metadata["instrument_model_code"], model);
        assert_eq!(records[row].metadata["sample_id"], "SAMPLE");
    }
    assert_eq!(
        records[0].metadata["sensor_values"],
        serde_json::json!([null, null, -4.5])
    );
    assert!(records[0]
        .provenance
        .warnings
        .iter()
        .any(|w| w.starts_with("foss_nsas_da_nonfinite_sensor")));
    assert_eq!(reconstruct_da(&records), da);
}

#[test]
fn rejects_read_options_consistently_with_and_without_sidecars() {
    let da = fixture_bytes("references700.da");
    let cn = fixture_bytes("references700.cn");
    let options = ReadOptions::default().single_record();
    assert!(open_bytes_with_options("references700.da", &da, &options).is_err());
    assert!(open_path_with_options(fixture("references700.da"), &options).is_err());
    assert!(open_with_sidecars_and_options(
        "references700.da",
        &da,
        Arc::new(InMemorySidecars::new().with("references700.cn", cn)),
        &options,
    )
    .is_err());
}

#[test]
#[ignore = "requires the proprietary VISION DA/CN corpus in samples_local/foss_nsas_da"]
fn local_vendor_corpus_preserves_30_da_28_cn_and_all_active_references() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples_local/foss_nsas_da/research_20261118/vision_extracted");
    let files: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    let mut da_count = 0;
    let mut cn_count = 0;
    let mut spectral_values = 0;
    let mut target_values = 0;
    for path in files
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("da")))
    {
        da_count += 1;
        let da = std::fs::read(path).unwrap();
        let sidecar = files.iter().find(|p| {
            p.file_stem() == path.file_stem()
                && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("cn"))
        });
        let records = open_path(path).unwrap();
        let bare = open_bytes(path.file_name().unwrap(), &da).unwrap();
        let expected_da_sha = SourceFile::from_bytes(path, &da, "primary").sha256;
        assert_eq!(records.len(), bare.len());
        assert_eq!(reconstruct_da(&records), da, "{}", path.display());
        for (record, without_cn) in records.iter().zip(&bare) {
            assert_eq!(record.signals, without_cn.signals);
            assert!(without_cn.targets.is_empty());
            assert_eq!(record.provenance.sources[0].sha256, expected_da_sha);
            spectral_values += record.signals.values().next().unwrap().values.len();
            target_values += record.targets.len();
        }
        if let Some(cn_path) = sidecar {
            cn_count += 1;
            let cn = std::fs::read(cn_path).unwrap();
            assert_eq!(reconstruct_cn(&records), cn, "{}", cn_path.display());
            let expected_sha = SourceFile::from_bytes(cn_path, &cn, "constituent_reference").sha256;
            let memory = open_with_sidecars(
                path.file_name().unwrap(),
                &da,
                Arc::new(InMemorySidecars::new().with(cn_path.file_name().unwrap(), cn.clone())),
            )
            .unwrap();
            for (index, record) in records.iter().enumerate() {
                assert_eq!(record.targets, memory[index].targets);
                assert_eq!(record.metadata, memory[index].metadata);
                assert_eq!(record.signals, memory[index].signals);
                let source = record
                    .provenance
                    .sources
                    .iter()
                    .find(|s| s.role == "constituent_reference")
                    .unwrap();
                assert_eq!(source.sha256, expected_sha);
                let count = record.metadata["da_constituent_count"].as_u64().unwrap() as usize;
                assert_eq!(record.targets.len(), count);
                for column in 0..count {
                    let offset = 384 + index * 128 + column * 4;
                    let expected =
                        f32::from_le_bytes(cn[offset..offset + 4].try_into().unwrap()) as f64;
                    let key = record.metadata["cn_constituents"][column]["target_key"]
                        .as_str()
                        .unwrap();
                    assert_eq!(record.targets[key], expected);
                }
            }
        } else {
            assert!(records.iter().all(|r| r.targets.is_empty()));
        }
    }
    assert_eq!((da_count, cn_count), (30, 28));
    assert_eq!(spectral_values, 317_250);
    assert_eq!(target_values, 180);
}

#[test]
fn path_resolution_uses_actual_sidecar_casing_and_bytes_mode_does_not_read_disk() {
    let da = fixture_bytes("references700.da");
    let cn = fixture_bytes("references700.cn");
    let directory = tempfile::tempdir().unwrap();
    let primary = directory.path().join("MiXeD.DA");
    std::fs::write(&primary, &da).unwrap();
    std::fs::write(directory.path().join("mixed.cN"), &cn).unwrap();
    let disk = open_path(&primary).unwrap();
    assert_eq!(disk[0].targets["water"], 1.25);
    let source = disk[0]
        .provenance
        .sources
        .iter()
        .find(|s| s.role == "constituent_reference")
        .unwrap();
    assert!(source.path.ends_with("mixed.cN"));
    let memory = open_bytes(&primary, &da).unwrap();
    assert!(memory.iter().all(|r| r.targets.is_empty()));
    assert!(memory[0]
        .provenance
        .warnings
        .iter()
        .any(|w| w.starts_with("foss_nsas_da_missing_cn")));
}

#[test]
fn recognized_nonlegacy_math_code_does_not_claim_absorbance() {
    let mut bytes = DATA[..4352].to_vec();
    set_word(&mut bytes, 20, 11);
    let records = open_bytes("other_math.da", &bytes).unwrap();
    assert_eq!(records[0].signal_type, SignalType::Unknown);
    assert_eq!(records[0].signals["spectrum"].role, "spectrum");
    assert_eq!(reconstruct_da(&records), bytes);
}
