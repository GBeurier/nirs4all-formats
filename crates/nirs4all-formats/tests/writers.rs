//! Writers are checked against our readers on every public sample, and against
//! hand-built records for the declared refusals and losses.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use nirs4all_formats::{
    decode_xlsx_cells, open_bytes, open_path, software_catalog, write_records, writer_catalog,
    AxisKind, Provenance, SignalType, SourceFile, SpectralArray, SpectralAxis, SpectralRecord,
    WriteError, WriteOptions, WriteOutput, XlsxCellValue, XlsxDecodeLimits,
};
use serde_json::json;

const MAX_RECORDS: usize = 40;

fn samples() -> Vec<PathBuf> {
    fn visit(dir: &Path, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                visit(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples");
    visit(&root, &mut out);
    out
}

/// Every decodable public sample, with a bounded number of records.
fn sample_records() -> Vec<(PathBuf, Vec<SpectralRecord>)> {
    samples()
        .into_iter()
        .filter_map(|path| {
            let mut records = open_path(&path).ok()?;
            records.truncate(MAX_RECORDS);
            (!records.is_empty()).then_some((path, records))
        })
        .collect()
}

fn loss(output: &WriteOutput, code: &str) -> Option<f64> {
    output
        .losses
        .iter()
        .filter(|loss| loss.code == code)
        .map(|loss| loss.max_error.unwrap_or(0.0))
        .fold(None, |acc, error| {
            Some(acc.map_or(error, |acc: f64| acc.max(error)))
        })
}

fn close(left: &[f64], right: &[f64], absolute: f64, context: &str) {
    assert_eq!(left.len(), right.len(), "{context}: length");
    for (index, (left, right)) in left.iter().zip(right).enumerate() {
        let bound = absolute * (1.0 + 1e-9) + 4e-12 * left.abs().max(right.abs());
        assert!(
            (left - right).abs() <= bound,
            "{context}: point {index}: {left} vs {right}, bound {bound}"
        );
    }
}

/// Spectra in the order writers visit them: record, then signal name.
fn expected(records: &[SpectralRecord]) -> Vec<(&SpectralArray, Vec<f64>)> {
    let mut out = Vec::new();
    for record in records {
        for signal in record.signals.values() {
            let x = signal.x_dim_index();
            let stride: usize = signal.shape[x + 1..].iter().product();
            let points = signal.shape[x];
            for flat in 0..signal.values.len() / points {
                let start = flat / stride * points * stride + flat % stride;
                let values = (0..points)
                    .map(|i| signal.values[start + i * stride])
                    .collect();
                out.push((signal, values));
            }
        }
    }
    out
}

fn is_finite(records: &[SpectralRecord]) -> bool {
    records
        .iter()
        .flat_map(|record| record.signals.values())
        .all(|signal| signal.values.iter().all(|value| value.is_finite()))
}

#[test]
fn per_spectrum_targets_read_back_every_sample() {
    let mut checked = BTreeMap::<&str, usize>::new();
    let mut refusals = Vec::new();
    for (path, records) in sample_records() {
        for target in ["jcamp-dx", "galactic-spc"] {
            let label = format!("{target} {}", path.display());
            let output = match write_records(&records, target, &WriteOptions::default()) {
                Ok(output) => output,
                Err(error) => {
                    let f32_range = error.to_string().contains("32-bit float range");
                    let interferograms = error.to_string().contains("interferograms need");
                    assert!(
                        !is_finite(&records) || f32_range || interferograms,
                        "{label}: {error}"
                    );
                    refusals.push(label);
                    continue;
                }
            };
            let mut expected = expected(&records);
            if target == "jcamp-dx" {
                expected.retain(|(signal, _)| signal.signal_type != SignalType::Interferogram);
            }
            assert_eq!(output.files.len(), expected.len(), "{label}");
            let axis_error = loss(&output, "axis_rounded_f32").unwrap_or(0.0);
            let value_error = loss(&output, "ordinate_rounded_f32")
                .or(loss(&output, "ordinate_quantized"))
                .unwrap_or(0.0);
            for (file, (signal, values)) in output.files.iter().zip(expected) {
                let back = open_bytes(&file.name, &file.bytes)
                    .unwrap_or_else(|error| panic!("{label} {}: {error}", file.name));
                assert_eq!(back.len(), 1, "{label}");
                assert_eq!(back[0].signals.len(), 1, "{label}");
                let read = back[0].signals.values().next().unwrap();
                close(&signal.axis.values, &read.axis.values, axis_error, &label);
                close(&values, &read.values, value_error, &label);
                let standard = matches!(
                    signal.signal_type,
                    SignalType::Absorbance | SignalType::Reflectance | SignalType::Transmittance
                );
                if standard && signal.unit.is_none() {
                    assert_eq!(read.signal_type, signal.signal_type, "{label}");
                }
                if matches!(signal.axis.unit.as_str(), "nm" | "cm-1" | "um") {
                    assert_eq!(
                        (&read.axis.kind, &read.axis.unit),
                        (&signal.axis.kind, &signal.axis.unit),
                        "{label}"
                    );
                }
                *checked.entry(target).or_default() += 1;
            }
        }
    }
    eprintln!("checked {checked:?}; refused {refusals:?}");
    assert!(checked.values().all(|count| *count > 500));
}

#[test]
fn matrix_targets_hold_every_sample_value() {
    let mut checked = 0usize;
    for (path, records) in sample_records() {
        let spectra = expected(&records);
        let total: usize = spectra.iter().map(|(_, values)| values.len()).sum();
        if total > 60_000 || spectra.iter().any(|(_, values)| values.len() > 16_000) {
            continue;
        }
        let output = write_records(&records, "xlsx", &WriteOptions::default())
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let csv = write_records(&records, "csv", &WriteOptions::default()).unwrap();
        assert_eq!(output.files.len(), csv.files.len());
        let mut stored = Vec::new();
        for (file, csv) in output.files.iter().zip(&csv.files) {
            let book = decode_xlsx_cells(&file.bytes, "x.xlsx", XlsxDecodeLimits::unlimited())
                .unwrap_or_else(|error| panic!("{}: {error:?}", path.display()));
            assert_eq!(book.sheet_names, ["spectra", "info"]);
            let cells = &book.sheets[0].cells;
            let spectral_start = cells
                .iter()
                .filter(|cell| cell.position.0 == 0 && matches!(cell.value, XlsxCellValue::Text(_)))
                .count() as u32;
            let mut sheet = Vec::new();
            for cell in cells {
                if cell.position.0 == 0 || cell.position.1 < spectral_start {
                    continue;
                }
                match &cell.value {
                    XlsxCellValue::Float64 { bits, .. } => sheet.push(f64::from_bits(*bits)),
                    XlsxCellValue::IntegerLexeme { token } => sheet.push(token.parse().unwrap()),
                    other => panic!("{}: unexpected cell {other:?}", path.display()),
                }
            }
            let text = std::str::from_utf8(&csv.bytes).unwrap();
            let header: Vec<&str> = text.lines().next().unwrap().split(',').collect();
            assert_eq!(header[0], "sample_id");
            stored.extend(sheet);
        }
        let mut wanted: Vec<f64> = spectra
            .iter()
            .flat_map(|(_, values)| values.iter().copied())
            .filter(|value| value.is_finite())
            .collect();
        stored.sort_by(f64::total_cmp);
        wanted.sort_by(f64::total_cmp);
        assert!(
            stored == wanted,
            "{}: workbook values differ",
            path.display()
        );
        checked += 1;
    }
    eprintln!("checked {checked} samples");
    assert!(checked > 60);
}

#[test]
fn json_records_read_back() {
    for (path, records) in sample_records() {
        let output = write_records(&records, "records-json", &WriteOptions::default()).unwrap();
        let back: Vec<SpectralRecord> = serde_json::from_slice(&output.files[0].bytes).unwrap();
        assert_eq!(back.len(), records.len());
        for (left, right) in records.iter().zip(&back) {
            for (name, signal) in &left.signals {
                let read = &right.signals[name];
                // serde_json parses decimals to within a few units in the last place.
                let same = |a: &f64, b: &f64| {
                    (a - b).abs() <= 4.0 * f64::EPSILON * a.abs() || (!a.is_finite() && b.is_nan())
                };
                let axis = signal.axis.values.iter().zip(&read.axis.values);
                let values = signal.values.iter().zip(&read.values);
                assert!(
                    axis.chain(values).all(|(a, b)| same(a, b)),
                    "{}",
                    path.display()
                );
                assert_eq!(signal.values.len(), read.values.len());
                assert_eq!(signal.axis.unit, read.axis.unit);
            }
        }
    }
}

fn record(id: &str, axis: &[f64], unit: &str, values: &[f64], kind: SignalType) -> SpectralRecord {
    let axis_kind = if unit == "cm-1" {
        AxisKind::Wavenumber
    } else {
        AxisKind::Wavelength
    };
    let signal = SpectralArray::new(
        SpectralAxis::new(axis.to_vec(), unit, axis_kind).unwrap(),
        values.to_vec(),
        vec!["x".to_string()],
        kind.clone(),
        None,
        "signal",
        "test",
    )
    .unwrap();
    SpectralRecord {
        signals: BTreeMap::from([("signal".to_string(), signal)]),
        signal_type: kind,
        targets: BTreeMap::new(),
        metadata: BTreeMap::from([("sample_id".to_string(), json!(id))]),
        provenance: Provenance {
            format: "test".to_string(),
            reader: "test".to_string(),
            reader_version: "0".to_string(),
            sources: vec![SourceFile::from_bytes("a.dat", b"", "primary")],
            parsed_at_utc: None,
            record_schema_version: "1".to_string(),
            warnings: Vec::new(),
        },
        quality_flags: Vec::new(),
    }
}

fn nir(id: &str) -> SpectralRecord {
    record(
        id,
        &[1000.0, 1002.0, 1004.0, 1006.0],
        "nm",
        &[0.5, 0.25, 0.75, 0.125],
        SignalType::Absorbance,
    )
}

fn options(value: serde_json::Value) -> WriteOptions {
    serde_json::from_value(value).unwrap()
}

#[test]
fn even_axes_use_xydata_and_uneven_axes_keep_explicit_points() {
    let even = write_records(&[nir("a")], "jcamp-dx", &WriteOptions::default()).unwrap();
    let text = String::from_utf8(even.files[0].bytes.clone()).unwrap();
    assert!(text.contains("##JCAMP-DX=4.24\r\n"), "{text}");
    assert!(
        text.contains("##XYDATA=(X++(Y..Y))\r\n1000 500000000 250000000 750000000 125000000\r\n")
    );
    assert!(text.contains("##YFACTOR=1E-09\r\n"));
    assert!(text.contains("##XUNITS=NANOMETERS\r\n"));
    assert!(text.ends_with("##END=\r\n"));
    assert!(text.lines().all(|line| line.len() <= 80));
    assert!(loss(&even, "ordinate_quantized").is_none());

    let jitter = record(
        "b",
        &[1000.0, 1002.0, 1004.01, 1006.0],
        "nm",
        &[0.1, 0.2, 0.3, 0.4],
        SignalType::Reflectance,
    );
    let uneven = write_records(
        std::slice::from_ref(&jitter),
        "jcamp-dx",
        &WriteOptions::default(),
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&uneven.files[0].bytes)
        .contains("##XYPOINTS=(XY..XY)\r\n1000,0.1\r\n"));
    assert!(loss(&uneven, "axis_regularized").is_none());

    let forced = write_records(
        &[jitter],
        "jcamp-dx",
        &options(json!({"axis_tolerance": 0.02})),
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&forced.files[0].bytes).contains("##XYDATA="));
    assert!((loss(&forced, "axis_regularized").unwrap() - 0.01).abs() < 1e-9);
}

#[test]
fn multi_spc_keeps_order_names_and_targets_in_one_file() {
    let mut records = vec![nir("alpha"), nir("beta")];
    records[1].signals.get_mut("signal").unwrap().values[0] = 0.875;
    records[0]
        .targets
        .insert("protein".to_string(), json!(12.5));
    let output = write_records(&records, "galactic-spc", &options(json!({"multi": true}))).unwrap();
    assert_eq!(output.files.len(), 1);
    assert!(loss(&output, "targets_in_comments").is_some());
    let back = open_bytes("x.spc", &output.files[0].bytes).unwrap();
    assert_eq!(back.len(), 2);
    assert_eq!(back[0].metadata["sample_id"], "alpha");
    assert_eq!(back[1].metadata["sample_id"], "beta");
    assert_eq!(back[1].signals.values().next().unwrap().values[0], 0.875);
    assert_eq!(
        back[0].metadata["galactic_spc_log"]["TARGET1 protein"],
        "12.5"
    );

    // Spectra on different axes never share a file.
    records.push(record(
        "gamma",
        &[1.0, 2.0, 4.0],
        "um",
        &[0.0, 0.5, 1.0],
        SignalType::Absorbance,
    ));
    let output = write_records(&records, "galactic-spc", &options(json!({"multi": true}))).unwrap();
    assert_eq!(output.files.len(), 2);
}

#[test]
fn matrix_rows_align_targets_and_refuse_nothing_silently() {
    let mut records = vec![nir("a,1"), nir("b")];
    records[0]
        .targets
        .insert("protein".to_string(), json!(12.5));
    records[1].targets.insert("1450".to_string(), json!(3));
    records[1].signals.get_mut("signal").unwrap().values[2] = f64::NAN;
    let output = write_records(&records, "csv", &WriteOptions::default()).unwrap();
    let text = String::from_utf8(output.files[0].bytes.clone()).unwrap();
    assert_eq!(
        text,
        "sample_id,target_1450,protein,1000,1002,1004,1006\r\n\
         a_1,,12.5,0.5,0.25,0.75,0.125\r\n\
         b,3,,0.5,0.25,,0.125\r\n"
    );
    assert!(loss(&output, "nonfinite_as_empty").is_some());
    assert!(loss(&output, "text_sanitized").is_some());
    let back = open_bytes("x.csv", &output.files[0].bytes).unwrap();
    assert_eq!(back.len(), 2);
    assert_eq!(back[0].metadata["sample_id"], "a_1");
    assert_eq!(back[0].targets["protein"], 12.5);
    assert_eq!(back[1].targets["target_1450"], 3.0);
    assert_eq!(
        back[0].signals.values().next().unwrap().axis.values,
        vec![1000.0, 1002.0, 1004.0, 1006.0]
    );
}

#[test]
fn unrepresentable_values_are_refused() {
    let mut missing = nir("a");
    missing.signals.get_mut("signal").unwrap().values[1] = f64::NAN;
    for target in ["jcamp-dx", "galactic-spc", "xy-text"] {
        let error =
            write_records(&[missing.clone()], target, &WriteOptions::default()).unwrap_err();
        assert!(matches!(error, WriteError::Refused(_)), "{target}");
    }
    let mut large = nir("a");
    large.signals.get_mut("signal").unwrap().values[1] = 1e300;
    assert!(write_records(&[large], "galactic-spc", &WriteOptions::default()).is_err());
    assert!(write_records(&[nir("a")], "bruker-dpt", &WriteOptions::default()).is_err());
    assert!(write_records(&[], "csv", &WriteOptions::default()).is_err());
    assert!(write_records(&[nir("a")], "csv", &options(json!({"signal": "other"}))).is_err());
    assert!(write_records(&[nir("a")], "csv", &options(json!({"extension": "../x"}))).is_err());
    assert!(write_records(&[nir("a")], "csv", &options(json!({"delimiter": "|"}))).is_err());
    assert!(matches!(
        write_records(&[nir("a")], "opus-native", &WriteOptions::default()),
        Err(WriteError::UnknownTarget(_))
    ));
    let mut broken = nir("a");
    broken.signals.get_mut("signal").unwrap().axis.values.pop();
    assert!(write_records(&[broken], "csv", &WriteOptions::default()).is_err());
}

fn with_unit(unit: &str, kind: SignalType) -> SpectralRecord {
    let mut record = nir("a");
    record.signal_type = kind.clone();
    let signal = record.signals.get_mut("signal").unwrap();
    signal.signal_type = kind;
    signal.unit = Some(unit.to_string());
    record
}

#[test]
fn scaled_units_are_never_written_as_plain_ones() {
    for (unit, kind, yunits, code) in [
        ("AU", SignalType::Absorbance, "ABSORBANCE", 2),
        ("mAU", SignalType::Absorbance, "ARBITRARY UNITS", 0),
        (
            "Reflectance (percent)",
            SignalType::Reflectance,
            "PERCENT REFLECTANCE",
            129,
        ),
        (
            "%T",
            SignalType::Transmittance,
            "PERCENT TRANSMITTANCE",
            128,
        ),
        ("Cnt/sec", SignalType::RawCounts, "ARBITRARY UNITS", 0),
        ("counts", SignalType::RawCounts, "ARBITRARY UNITS", 4),
    ] {
        let records = [with_unit(unit, kind)];
        let output = write_records(&records, "jcamp-dx", &WriteOptions::default()).unwrap();
        let text = String::from_utf8(output.files[0].bytes.clone()).unwrap();
        assert!(
            text.contains(&format!("##YUNITS={yunits}\r\n")),
            "{unit}: {text}"
        );
        let spc = write_records(&records, "galactic-spc", &WriteOptions::default()).unwrap();
        assert_eq!(spc.files[0].bytes[29], code, "{unit}");
        if !matches!(unit, "AU" | "counts") {
            let back = open_bytes("x.spc", &spc.files[0].bytes).unwrap();
            let label = back[0].metadata["galactic_spc"]["y_label"].to_string();
            assert!(
                label.contains("percent") || label.contains(unit),
                "{unit}: {label}"
            );
            assert_eq!(
                back[0].metadata["galactic_spc"]["x_label"],
                "Nanometers (nm)"
            );
            assert!(
                output
                    .losses
                    .iter()
                    .any(|loss| loss.code.starts_with("signal_")),
                "{unit}"
            );
        }
    }
}

#[test]
fn identifiers_come_from_the_record_or_are_declared() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples");
    for (path, id) in [
        ("bruker_opus/BF_lo_01_soil_cal.1", "BF_lo_01_soil_cal"),
        ("jcamp_dx/ethanol_ir.jdx", "ETHANOL"),
        ("foss_winisi/synthetic.cal", "S001"),
    ] {
        let records = open_path(root.join(path)).unwrap();
        let output = write_records(&records, "csv", &WriteOptions::default()).unwrap();
        let text = String::from_utf8(output.files[0].bytes.clone()).unwrap();
        assert!(
            text.lines().nth(1).unwrap().starts_with(&format!("{id},")),
            "{path}"
        );
        assert!(loss(&output, "identifier_generated").is_none(), "{path}");
    }
    let mut anonymous = nir("x");
    anonymous.metadata.clear();
    let mut titled = anonymous.clone();
    titled.metadata.insert("title".to_string(), json!("Map"));
    let output = write_records(&[titled.clone(), titled], "csv", &WriteOptions::default()).unwrap();
    let text = String::from_utf8(output.files[0].bytes.clone()).unwrap();
    assert!(
        text.contains("\r\na_1,") && text.contains("\r\na_2,"),
        "{text}"
    );
    assert!(loss(&output, "identifier_generated").is_some());
    let single = write_records(&[anonymous], "jcamp-dx", &WriteOptions::default()).unwrap();
    assert_eq!(single.files[0].name, "a.jdx");
}

#[test]
fn numbered_sources_keep_distinct_file_names() {
    let mut record = nir("x");
    record.provenance.sources = vec![SourceFile::from_bytes("d/s.0", b"", "primary")];
    let output = write_records(&[record], "jcamp-dx", &WriteOptions::default()).unwrap();
    assert_eq!(output.files[0].name, "s.0.jdx");
}

#[test]
fn raman_is_not_labelled_infrared() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples");
    let records = open_path(root.join("galactic_spc/RAMAN.SPC")).unwrap();
    let jcamp = write_records(&records, "jcamp-dx", &WriteOptions::default()).unwrap();
    assert!(String::from_utf8_lossy(&jcamp.files[0].bytes).contains("##DATA TYPE=RAMAN SPECTRUM"));
    let spc = write_records(&records, "galactic-spc", &WriteOptions::default()).unwrap();
    assert_eq!(spc.files[0].bytes[28], 13);
    assert_eq!(spc.files[0].bytes[2], 11);
    let ftir = open_path(root.join("galactic_spc/resolutionPro.spc")).unwrap();
    let output = write_records(&ftir, "jcamp-dx", &WriteOptions::default()).unwrap();
    assert!(
        String::from_utf8_lossy(&output.files[0].bytes).contains("##DATA TYPE=INFRARED SPECTRUM")
    );
    let spc = write_records(&ftir, "galactic-spc", &WriteOptions::default()).unwrap();
    assert_eq!((spc.files[0].bytes[2], spc.files[0].bytes[28]), (0, 1));
    let counts = record(
        "c",
        &[500.0, 501.0],
        "nm",
        &[1.0, 2.0],
        SignalType::RawCounts,
    );
    let output = write_records(&[counts], "jcamp-dx", &WriteOptions::default()).unwrap();
    assert!(loss(&output, "technique_not_known").is_some());
    let unknown = record("u", &[1.0, 2.0], "cm-1", &[1.0, 2.0], SignalType::RawCounts);
    let output = write_records(&[unknown], "jcamp-dx", &WriteOptions::default()).unwrap();
    assert!(loss(&output, "technique_not_known").is_some());
}

#[test]
fn interferograms_never_become_jcamp_spectra() {
    let mut record = nir("a");
    record.signals.get_mut("signal").unwrap().signal_type = SignalType::Interferogram;
    let records = [record];
    assert!(write_records(&records, "jcamp-dx", &WriteOptions::default()).is_err());
    assert!(write_records(&records, "galactic-spc", &WriteOptions::default()).is_ok());
}

#[test]
fn spc_log_size_covers_its_header() {
    let output = write_records(&[nir("a")], "galactic-spc", &WriteOptions::default()).unwrap();
    let bytes = &output.files[0].bytes;
    let word = |at: usize| i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let offset = word(248);
    assert_eq!(offset + word(offset), bytes.len());
}

#[test]
fn large_and_structured_targets_are_not_altered_silently() {
    let mut record = nir("a");
    record
        .targets
        .insert("barcode".to_string(), json!(9007199254740993u64));
    record.targets.insert("list".to_string(), json!([1, 2]));
    record.targets.insert("1450".to_string(), json!(1));
    record.targets.insert("target_1450".to_string(), json!(2));
    let records = [record];
    let csv = write_records(&records, "csv", &WriteOptions::default()).unwrap();
    let text = String::from_utf8(csv.files[0].bytes.clone()).unwrap();
    assert!(text.contains("9007199254740993"), "{text}");
    let header: Vec<&str> = text.lines().next().unwrap().split(',').collect();
    assert_eq!(header.iter().collect::<BTreeSet<_>>().len(), header.len());
    for target in ["csv", "jcamp-dx", "galactic-spc"] {
        let output = write_records(&records, target, &WriteOptions::default()).unwrap();
        assert!(
            loss(&output, "structured_targets_omitted").is_some(),
            "{target}"
        );
    }
}

#[test]
fn every_sample_jcamp_line_follows_the_specification() {
    for (path, records) in sample_records() {
        let Ok(output) = write_records(&records, "jcamp-dx", &WriteOptions::default()) else {
            continue;
        };
        for file in &output.files {
            let text = std::str::from_utf8(&file.bytes).unwrap();
            assert!(text.is_ascii());
            for line in text.lines() {
                assert!(line.len() <= 80, "{}: {line}", path.display());
                let body = line.split_once('=').map_or(line, |(_, value)| value);
                for token in body
                    .split([' ', ','])
                    .filter(|token| token.parse::<f64>().is_ok())
                {
                    if let Some((_, exponent)) = token.split_once('E') {
                        let digits = exponent.strip_prefix(['+', '-']);
                        assert!(
                            digits.is_some_and(|digits| (2..=3).contains(&digits.len())),
                            "{}: {line}",
                            path.display()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn json_honours_the_signal_selection() {
    let mut record = nir("a");
    let other = record.signals["signal"].clone();
    record.signals.insert("other".to_string(), other);
    let output = write_records(
        &[record],
        "records-json",
        &options(json!({"signal": "other"})),
    )
    .unwrap();
    let back: Vec<SpectralRecord> = serde_json::from_slice(&output.files[0].bytes).unwrap();
    assert_eq!(back[0].signals.keys().collect::<Vec<_>>(), ["other"]);
    assert!(loss(&output, "signals_omitted").is_some());
}

#[test]
fn cubes_flatten_with_named_pixels() {
    let mut cube = nir("cube");
    let axis = SpectralAxis::new(vec![1000.0, 1010.0], "nm", AxisKind::Wavelength).unwrap();
    let signal = SpectralArray::new_nd(
        vec![2, 2, 3],
        vec!["row".into(), "x".into(), "col".into()],
        axis,
        BTreeMap::from([
            ("row".to_string(), SpectralAxis::index(2)),
            ("col".to_string(), SpectralAxis::index(3)),
        ]),
        (0..12).map(f64::from).collect(),
        SignalType::Reflectance,
        None,
        "cube",
        "test",
    )
    .unwrap();
    cube.signals.insert("signal".to_string(), signal);
    let output = write_records(&[cube], "csv", &WriteOptions::default()).unwrap();
    let text = String::from_utf8(output.files[0].bytes.clone()).unwrap();
    assert_eq!(text.lines().nth(1).unwrap(), "cube_row0_col0,0,3");
    assert_eq!(text.lines().nth(6).unwrap(), "cube_row1_col2,8,11");
    assert!(loss(&output, "dimensions_flattened").is_some());
}

#[test]
fn hostile_names_stay_inside_labels_and_files() {
    let mut records = vec![nir("a\r\n##END=\r\n../é"), nir("a\r\n##END=\r\n../é")];
    records[0]
        .targets
        .insert("p\n##X".to_string(), json!("v$$w"));
    let output = write_records(&records, "jcamp-dx", &WriteOptions::default()).unwrap();
    let names: BTreeSet<&str> = output.files.iter().map(|file| file.name.as_str()).collect();
    assert_eq!(names.len(), 2);
    assert!(names.iter().all(|name| !name.contains(['/', '\\'])));
    let text = String::from_utf8(output.files[0].bytes.clone()).unwrap();
    assert_eq!(text.matches("##END=").count(), 1);
    assert!(text.is_ascii());
    let book = write_records(&records, "xlsx", &WriteOptions::default()).unwrap();
    decode_xlsx_cells(
        &book.files[0].bytes,
        "x.xlsx",
        XlsxDecodeLimits::unlimited(),
    )
    .unwrap();
}

#[test]
fn presets_and_archives_resolve_to_known_writers() {
    for profile in software_catalog() {
        assert!(writer_catalog()
            .iter()
            .any(|entry| entry.format == profile.format));
        for alternative in profile.alternatives {
            assert!(writer_catalog()
                .iter()
                .any(|entry| entry.format == *alternative));
        }
        assert!(!writer_catalog()
            .iter()
            .any(|entry| entry.format == profile.id));
        let mut record = nir("a");
        record.targets.insert("protein".to_string(), json!(12.5));
        let output = write_records(&[record], profile.id, &WriteOptions::default()).unwrap();
        assert!(output.files[0]
            .name
            .ends_with(&format!(".{}", profile.extension)));
    }
    let zipped = write_records(
        &[nir("a"), nir("b")],
        "jcamp-dx",
        &options(json!({"zip": true, "base_name": "set"})),
    )
    .unwrap();
    assert_eq!(zipped.files.len(), 1);
    assert_eq!(zipped.files[0].name, "set.zip");
    let again = write_records(
        &[nir("a"), nir("b")],
        "jcamp-dx",
        &options(json!({"zip": true, "base_name": "set"})),
    )
    .unwrap();
    assert_eq!(zipped.files, again.files);
}

fn samples_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .join(name)
}

fn nir_grid(id: &str, start: f64, points: usize) -> SpectralRecord {
    let axis: Vec<f64> = (0..points).map(|i| start + 2.0 * i as f64).collect();
    let values: Vec<f64> = (0..points).map(|i| 0.25 + i as f64 / 4096.0).collect();
    record(id, &axis, "nm", &values, SignalType::Absorbance)
}

#[test]
fn foss_targets_read_back_every_representable_sample() {
    let mut checked = BTreeMap::<&str, usize>::new();
    for (path, records) in sample_records() {
        for target in ["foss-winisi-nir", "foss-nsas-da"] {
            let label = format!("{target} {}", path.display());
            let Ok(output) = write_records(&records, target, &WriteOptions::default()) else {
                continue;
            };
            let axis_error = loss(&output, "axis_rounded_f32").unwrap_or(0.0);
            let value_error = loss(&output, "ordinate_rounded_f32").unwrap_or(0.0);
            let expected = expected(&records);
            let mut read = Vec::new();
            for file in output
                .files
                .iter()
                .filter(|file| !file.name.ends_with(".cn"))
            {
                read.extend(
                    open_bytes(&file.name, &file.bytes)
                        .unwrap_or_else(|error| panic!("{label}: {error}")),
                );
            }
            assert_eq!(read.len(), expected.len(), "{label}");
            if records.iter().all(|record| record.signals.len() == 1) && output.files.len() <= 2 {
                for (record, (signal, values)) in read.iter().zip(&expected) {
                    let back = record.signals.values().next().unwrap();
                    close(&signal.axis.values, &back.axis.values, axis_error, &label);
                    close(values, &back.values, value_error, &label);
                }
            }
            *checked.entry(target).or_default() += read.len();
        }
    }
    eprintln!("checked {checked:?}");
    assert!(checked.len() == 2 && checked.values().all(|count| *count > 5));
}

#[test]
fn winisi_files_follow_the_native_layout() {
    let mut records = vec![
        nir_grid("sample-with-a-long-name", 400.0, 1050),
        nir_grid("short", 400.0, 1050),
        nir_grid("sample-with-a-long-name", 400.0, 1050),
        nir_grid("sample-with-another-name", 400.0, 1050),
    ];
    records[0].targets = BTreeMap::from([
        ("protein".to_string(), json!(12.5)),
        ("a_very_long_constituent_name".to_string(), json!(3)),
        ("a_very_long_constituent_other".to_string(), json!(0)),
        ("variety".to_string(), json!("x")),
    ]);
    let output = write_records(&records, "winisi-cal", &WriteOptions::default()).unwrap();
    let file = &output.files[0];
    let bytes = &file.bytes;
    let word = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    // 896-byte header, 128-byte aligned blocks, then one 16-byte entry each.
    assert_eq!(bytes.len(), 0x380 + 4 * (4608 + 16));
    assert_eq!(
        (word(0), word(2), word(4), word(6), word(8)),
        (2, 4, 0, 1050, 3)
    );
    // Conventions of WinISI files for spectra of other instruments.
    assert_eq!(
        (word(0x80), word(0xA0), word(0xA2), word(0xCA)),
        (11, 1, 1050, 1)
    );
    assert_eq!(&bytes[0x82..0x8A], b"Unknown\0");
    let index = &bytes[0x380 + 4 * 4608..];
    let numbers: Vec<&[u8]> = index.chunks(16).map(|entry| &entry[..12]).collect();
    assert_eq!(numbers[0], b"sample-with-");
    assert_eq!(numbers[0], numbers[2]);
    assert_eq!(numbers[3], b"sample-wit~2");
    assert_eq!(index[13], 1);
    for code in [
        "identifiers_shortened",
        "target_names_shortened",
        "targets_omitted",
        "missing_targets_as_zero",
        "zero_targets_read_as_missing",
        "moisture_basis_assumed",
        "date_not_known",
    ] {
        assert!(loss(&output, code).is_some(), "{code}");
    }
    assert_eq!(
        &bytes[0x380 + 29..0x380 + 29 + 23],
        b"sample-with-a-long-name"
    );

    let back = open_bytes(&file.name, bytes).unwrap();
    assert_eq!(back.len(), 4);
    assert_eq!(back[0].provenance.format, "foss-winisi-cal");
    assert_eq!(back[1].metadata["sample_number"], "short");
    assert_eq!(back[0].targets["protein"], 12.5);
    assert_eq!(back[0].targets.len(), 3);
    let read = &back[3].signals["absorbance"];
    assert_eq!(read.values, records[3].signals["signal"].values);
    assert_eq!(read.axis.values, records[3].signals["signal"].axis.values);

    let nir = write_records(&records, "winisi", &WriteOptions::default()).unwrap();
    assert!(loss(&nir, "targets_omitted").is_some());
    assert_eq!(nir.files[0].bytes[0], 1);
    assert!(write_records(&records[1..], "winisi-cal", &WriteOptions::default()).is_err());
}

#[test]
fn winisi_single_records_of_any_length_read_back() {
    for points in [5, 64, 100, 125, 128, 700, 1557] {
        let mut record = nir_grid("a b", 900.0, points);
        record.targets.insert("protein".to_string(), json!(12.5));
        let output = write_records(
            std::slice::from_ref(&record),
            "winisi-cal",
            &WriteOptions::default(),
        )
        .unwrap();
        let back = open_bytes("x.cal", &output.files[0].bytes).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].targets["protein"], 12.5, "{points} points");
        assert_eq!(
            back[0].signals["absorbance"].values,
            record.signals["signal"].values
        );
        assert_eq!(back[0].metadata["sample_number"], "a b");
    }
}

#[test]
fn winisi_axes_become_segments_or_are_refused() {
    let axis: Vec<f64> = (0..10)
        .map(|i| 400.0 + 2.0 * i as f64)
        .chain((0..10).map(|i| 800.0 + 0.5 * i as f64))
        .collect();
    let values: Vec<f64> = (0..20).map(|i| i as f64).collect();
    let segmented = record("s", &axis, "nm", &values, SignalType::Absorbance);
    let output = write_records(&[segmented], "winisi", &WriteOptions::default()).unwrap();
    assert_eq!(output.files[0].bytes[0xA0], 2);
    assert!(loss(&output, "point_count_not_observed").is_some());
    let back = open_bytes("x.nir", &output.files[0].bytes).unwrap();
    assert_eq!(back[0].signals["absorbance"].axis.values, axis);

    let jitter = record(
        "j",
        &(0..40)
            .map(|i| 900.0 + 6.2 * i as f64 + if i % 2 == 0 { 0.1 } else { 0.0 })
            .collect::<Vec<_>>(),
        "nm",
        &[0.5; 40],
        SignalType::Absorbance,
    );
    assert!(write_records(
        std::slice::from_ref(&jitter),
        "winisi",
        &WriteOptions::default()
    )
    .is_err());
    let forced = write_records(
        &[jitter],
        "winisi",
        &options(json!({"axis_tolerance": 0.2})),
    )
    .unwrap();
    assert!(loss(&forced, "axis_regularized").unwrap() <= 0.2);

    let descending = record(
        "d",
        &[2500.0, 2498.0, 2496.0],
        "nm",
        &[1.0, 2.0, 3.0],
        SignalType::Absorbance,
    );
    assert!(write_records(&[descending], "winisi", &WriteOptions::default()).is_err());
}

#[test]
fn foss_targets_hold_absorbance_along_nanometres_only() {
    for target in ["foss-winisi-nir", "foss-nsas-da"] {
        let mut reflectance = nir_grid("r", 1100.0, 700);
        reflectance.signals.get_mut("signal").unwrap().signal_type = SignalType::Reflectance;
        assert!(write_records(&[reflectance], target, &WriteOptions::default()).is_err());
        let mut percent = nir_grid("p", 1100.0, 700);
        percent.signals.get_mut("signal").unwrap().unit = Some("mAU".to_string());
        assert!(write_records(&[percent], target, &WriteOptions::default()).is_err());
        let wavenumbers = record(
            "w",
            &[1.0, 2.0],
            "cm-1",
            &[1.0, 2.0],
            SignalType::Absorbance,
        );
        assert!(write_records(&[wavenumbers], target, &WriteOptions::default()).is_err());
        let mut unknown = nir_grid("u", 1100.0, 700);
        unknown.signals.get_mut("signal").unwrap().signal_type = SignalType::Unknown;
        let output = write_records(&[unknown], target, &WriteOptions::default()).unwrap();
        assert!(loss(&output, "signal_type_assumed").is_some());
    }
    let grid = write_records(
        &[nir_grid("g", 1000.0, 700)],
        "foss-nsas-da",
        &WriteOptions::default(),
    );
    assert!(matches!(grid, Err(WriteError::Refused(_))));
}

#[test]
fn nsas_files_pair_spectra_and_constituents() {
    let mut records = vec![
        nir_grid("identifier-one", 1100.0, 700),
        nir_grid("identifier-two", 1100.0, 700),
    ];
    records[0]
        .metadata
        .insert("acquired_local".to_string(), json!("1997-09-18T06:30:49"));
    for (index, record) in records.iter_mut().enumerate() {
        record
            .targets
            .insert("oil".to_string(), json!(index as f64));
    }
    records[1].targets.insert("water".to_string(), json!(5.0));
    let output = write_records(&records, "foss-nsas-da", &WriteOptions::default()).unwrap();
    let names: Vec<&str> = output.files.iter().map(|file| file.name.as_str()).collect();
    assert_eq!(names, ["a.da", "a.cn"]);
    assert_eq!(output.files[0].bytes.len(), 2 * 2944);
    assert_eq!(output.files[1].bytes.len(), 384 + 2 * 128);
    for code in [
        "identifiers_shortened",
        "incomplete_targets_omitted",
        "acquisition_words_defaulted",
        "date_not_known",
    ] {
        assert!(loss(&output, code).is_some(), "{code}");
    }
    let directory = tempfile::tempdir().unwrap();
    let paths = output.save(directory.path(), false).unwrap();
    let back = open_path(&paths[0]).unwrap();
    assert_eq!(back[0].metadata["sample_id"], "identifier");
    assert_eq!(back[1].metadata["sample_id"], "identifi~2");
    assert_eq!(back[0].metadata["acquired_local"], "1997-09-18T06:30:49");
    assert!(!back[1].metadata.contains_key("acquired_local"));
    assert_eq!(
        back[1].targets,
        BTreeMap::from([("oil".to_string(), json!(1.0))])
    );
    let read = &back[1].signals["spectrum"];
    assert_eq!(read.values, records[1].signals["signal"].values);
    assert_eq!(read.axis.values, records[1].signals["signal"].axis.values);
    assert!(write_records(
        &records,
        "foss-nsas-da",
        &options(json!({"extension": "CN"}))
    )
    .is_err());
}

#[test]
fn nsas_sources_are_rewritten_byte_for_byte() {
    for name in [
        "synthetic.da",
        "references700.da",
        "variant1700.da",
        "variant4200.da",
    ] {
        let path = samples_file("foss_nsas_da").join(name);
        let records = open_path(&path).unwrap();
        let output = write_records(&records, "foss-nsas-da", &WriteOptions::default()).unwrap();
        assert!(
            output.files[0].bytes == std::fs::read(&path).unwrap(),
            "{name}"
        );
        if name == "references700.da" {
            let cn = std::fs::read(path.with_extension("cn")).unwrap();
            assert!(output.files[1].bytes == cn, "{name} companion");
        }
    }
}

#[test]
fn edited_nsas_records_never_keep_source_identity_or_references() {
    let path = samples_file("foss_nsas_da/references700.da");
    let mut records = open_path(&path).unwrap();
    for (index, record) in records.iter_mut().enumerate() {
        record
            .metadata
            .insert("sample_id".to_string(), json!(format!("NEW{index}")));
        record
            .metadata
            .insert("acquired_local".to_string(), json!("2020-05-06T07:08:09"));
    }
    records[1].metadata.remove("acquired_local");
    records[2].targets.remove("oil");
    let output = write_records(&records, "foss-nsas-da", &WriteOptions::default()).unwrap();
    assert!(loss(&output, "incomplete_targets_omitted").is_some());
    let cn = &output.files[1].bytes;
    assert!(!cn
        .windows(3)
        .any(|window| window.eq_ignore_ascii_case(b"oil")));
    let directory = tempfile::tempdir().unwrap();
    let paths = output.save(directory.path(), false).unwrap();
    let back = open_path(&paths[0]).unwrap();
    assert_eq!(back[2].metadata["sample_id"], "NEW2");
    assert_eq!(back[2].metadata["acquired_local"], "2020-05-06T07:08:09");
    assert!(!back[1].metadata.contains_key("acquired_local"));
    assert_eq!(back[2].targets.len(), 1);
    assert_eq!(back[2].targets["water"], records[2].targets["water"]);
}

#[test]
fn winisi_zeros_stay_missing_in_nsas_files() {
    let mut records = vec![nir_grid("a", 1100.0, 700), nir_grid("b", 1100.0, 700)];
    for (record, protein) in records.iter_mut().zip([12.5, 0.0]) {
        record.targets.insert("protein".to_string(), json!(protein));
        record.targets.insert("fat".to_string(), json!(1.5));
    }
    let cal = write_records(&records, "winisi-cal", &WriteOptions::default()).unwrap();
    let source = open_bytes("x.cal", &cal.files[0].bytes).unwrap();
    let output = write_records(&source, "foss-nsas-da", &WriteOptions::default()).unwrap();
    assert!(loss(&output, "incomplete_targets_omitted").is_some());
    assert_eq!(&output.files[1].bytes[..4], b"fat ");
    assert!(output.files[1].bytes[10..384].iter().all(|byte| *byte == 0));
}

#[test]
fn similar_constituent_names_keep_their_values() {
    let mut record = nir_grid("a", 1100.0, 700);
    for (index, name) in ["Prot %", "prot", "PROT", "x y", "x_y"].iter().enumerate() {
        record.targets.insert(name.to_string(), json!(index + 1));
    }
    let output = write_records(&[record], "winisi-cal", &WriteOptions::default()).unwrap();
    let back = open_bytes("x.cal", &output.files[0].bytes).unwrap();
    let mut values: Vec<f64> = back[0]
        .targets
        .values()
        .map(|value| value.as_f64().unwrap())
        .collect();
    values.sort_by(f64::total_cmp);
    assert_eq!(values, [1.0, 2.0, 3.0, 4.0, 5.0]);
}

#[test]
fn odd_host_records_are_refused_without_panicking() {
    for axis in [&[1.0, 2.0, 3.0, 1e39][..], &[1e-46][..]] {
        let values = vec![0.5; axis.len()];
        let odd = record("a", axis, "nm", &values, SignalType::Absorbance);
        assert!(write_records(&[odd], "winisi", &WriteOptions::default()).is_err());
    }
    let mut records = open_path(samples_file("foss_nsas_da/references700.da")).unwrap();
    for record in &mut records {
        let columns = record.metadata["cn_constituents"].as_array().unwrap();
        let duplicated = json!([columns[0], columns[0]]);
        record
            .metadata
            .insert("cn_constituents".to_string(), duplicated);
    }
    let output = write_records(&records, "foss-nsas-da", &WriteOptions::default()).unwrap();
    assert_eq!(output.files.len(), 2);
}
