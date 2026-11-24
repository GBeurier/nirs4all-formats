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
        let output = write_records(&[nir("a")], profile.id, &WriteOptions::default()).unwrap();
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
