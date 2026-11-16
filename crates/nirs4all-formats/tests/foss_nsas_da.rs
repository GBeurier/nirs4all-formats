use std::path::Path;

use nirs4all_formats::{builtin_probes, open_bytes, open_path, AxisKind, SignalType};

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
