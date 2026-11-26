//! Conversion plans name what a target needs; they never compute values.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nirs4all_formats::{
    export_targets, open_path, plan_conversion, target_requirements, write_records, writer_catalog,
    AxisKind, ConversionPlan, EvenAxis, PlanOptions, PlanStatus, Provenance, SignalType,
    SourceFile, SpectralArray, SpectralAxis, SpectralRecord, WriteOptions, CONVERSIONS_KEY,
};
use serde_json::json;

fn sample(name: &str) -> Vec<SpectralRecord> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .join(name);
    open_path(&path).unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn options(value: serde_json::Value) -> PlanOptions {
    serde_json::from_value(value).unwrap()
}

fn plan(
    records: &[SpectralRecord],
    target: Option<&str>,
    value: serde_json::Value,
) -> ConversionPlan {
    plan_conversion(records, target, &options(value)).unwrap()
}

fn steps(plan: &ConversionPlan) -> Vec<(&str, Option<&str>)> {
    plan.steps
        .iter()
        .map(|step| (step.step, step.method_id))
        .collect()
}

fn signal(axis: &[f64], unit: &str, values: &[f64], kind: SignalType, name: &str) -> SpectralArray {
    let axis_kind = if unit == "cm-1" {
        AxisKind::Wavenumber
    } else {
        AxisKind::Wavelength
    };
    SpectralArray::new(
        SpectralAxis::new(axis.to_vec(), unit, axis_kind).unwrap(),
        values.to_vec(),
        vec!["x".to_string()],
        kind,
        None,
        name,
        "test",
    )
    .unwrap()
}

fn record(signals: Vec<(&str, SpectralArray)>, format: &str) -> SpectralRecord {
    let signal_type = signals[0].1.signal_type.clone();
    SpectralRecord {
        signals: signals
            .into_iter()
            .map(|(name, signal)| (name.to_string(), signal))
            .collect(),
        signal_type,
        targets: BTreeMap::new(),
        metadata: BTreeMap::from([("sample_id".to_string(), json!("s1"))]),
        provenance: Provenance {
            format: format.to_string(),
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

fn nm(start: f64, step: f64, points: usize) -> Vec<f64> {
    (0..points).map(|i| start + step * i as f64).collect()
}

#[test]
fn every_writer_lists_its_requirements() {
    let targets = export_targets();
    let listed = targets["requirements"].as_array().unwrap();
    assert_eq!(listed.len(), writer_catalog().len());
    for entry in writer_catalog() {
        assert!(
            target_requirements(entry.format).is_some(),
            "{}",
            entry.format
        );
    }
    let winisi = target_requirements("foss-winisi-nir").unwrap();
    assert_eq!(winisi.signal_types, vec![SignalType::Absorbance]);
    assert_eq!(winisi.axis_units, vec!["nm"]);
    assert_eq!(winisi.even_axis, EvenAxis::Segments);
    assert_eq!(winisi.max_segments, Some(7));
    let grids: Vec<(f64, f64, usize)> = winisi
        .grids
        .iter()
        .map(|grid| (grid.first, grid.last, grid.points))
        .collect();
    assert_eq!(grids, vec![(1100.0, 2498.0, 700), (400.0, 2498.0, 1050)]);
    let nsas = target_requirements("foss-nsas-da").unwrap();
    let grids: Vec<(f64, f64, f64, usize)> = nsas
        .grids
        .iter()
        .map(|grid| (grid.first, grid.step, grid.last, grid.points))
        .collect();
    assert_eq!(
        grids,
        vec![
            (1100.0, 2.0, 2498.0, 700),
            (400.0, 2.0, 2498.0, 1050),
            (800.0, 0.5, 1649.5, 1700),
            (400.0, 0.5, 2499.5, 4200),
        ]
    );
    assert_eq!(
        target_requirements("bruker-dpt").unwrap().axis_units,
        vec!["cm-1"]
    );
    assert!(target_requirements("jcamp-dx").unwrap().records_conversions);
}

#[test]
fn vendor_reflectance_becomes_absorbance_on_an_even_grid() {
    let records = sample("ocean_optics/whiteref.ProcSpec");
    let plan = plan(&records, Some("winisi"), json!({}));
    assert_eq!(plan.status, PlanStatus::Ready);
    assert_eq!(plan.source_signal.as_deref(), Some("reflectance"));
    assert_eq!(
        steps(&plan),
        vec![
            (
                "signal_conversion",
                Some("preprocessing.signal_conversion.to_absorbance")
            ),
            ("resample", Some("preprocessing.resampling.resampler")),
        ]
    );
    // The file declares percent, so nothing is asked.
    assert_eq!(plan.steps[0].params["is_percent"], json!(true));
    let grid = plan.grid.as_ref().unwrap();
    let range = plan.measured_range.unwrap();
    assert!(grid.first >= range[0] && grid.last <= range[1]);
    assert_eq!(plan.steps[1].params["extrapolate"], json!(false));
    assert_eq!(plan.steps[1].params["bounds_error"], json!(true));
    let output = plan.output.unwrap();
    assert_eq!(output.name, "absorbance (from reflectance)");
    assert_eq!(output.signal_type, SignalType::Absorbance);
}

#[test]
fn raw_counts_use_the_references_of_their_record() {
    let records = sample("ocean_optics/whiteref.ProcSpec");
    let plan = plan(&records, Some("winisi"), json!({ "signal": "sample" }));
    assert_eq!(plan.status, PlanStatus::Ready);
    let first = &plan.steps[0];
    assert_eq!(first.step, "reflectance_from_references");
    assert_eq!(first.inputs["white_reference"], json!("white_reference"));
    assert_eq!(first.inputs["dark_reference"], json!("dark_reference"));
    assert_eq!(plan.steps[1].params["is_percent"], json!(false));

    let refused = self::plan(
        &records,
        Some("winisi"),
        json!({ "signal": "white_reference" }),
    );
    assert_eq!(refused.status, PlanStatus::Refused);
    assert_eq!(refused.refusals[0].code, "reference_signal");

    let irradiance = sample("ocean_optics/irrad.JazIrrad");
    let refused = self::plan(&irradiance, Some("winisi"), json!({}));
    assert_eq!(refused.status, PlanStatus::Refused);
    assert!(refused
        .refusals
        .iter()
        .all(|refusal| refusal.code == "references_missing"));
    // Exported as they are, raw counts need no reference.
    let unchanged = self::plan(&irradiance, Some("jcamp-dx"), json!({ "signal": "sample" }));
    assert_eq!(unchanged.status, PlanStatus::Unchanged);
}

#[test]
fn overlapping_detectors_are_selected_before_resampling() {
    let records = sample("svc_ger/ACPL_D2_P1_B_1_001.sig");
    let plan = plan(&records, Some("winisi"), json!({}));
    assert_eq!(plan.status, PlanStatus::Ready);
    assert_eq!(
        steps(&plan)
            .iter()
            .map(|(step, _)| *step)
            .collect::<Vec<_>>(),
        vec!["signal_conversion", "select_points", "resample"]
    );
    let keep: Vec<[usize; 2]> =
        serde_json::from_value(plan.steps[1].inputs["keep"].clone()).unwrap();
    assert!(plan.steps[1].method_id.is_none());
    let axis = &records[0].signals["reflectance"].axis.values;
    let kept: Vec<f64> = keep
        .iter()
        .flat_map(|range| axis[range[0]..range[1]].iter().copied())
        .collect();
    assert!(kept.windows(2).all(|pair| pair[1] > pair[0]));
    assert!(kept.len() > axis.len() - 20);
    assert_eq!(plan.grid.as_ref().unwrap().id, "winisi-1050");
    // Radiances are turned into reflectance with the reference panel.
    let radiance = self::plan(&records, Some("winisi"), json!({ "signal": "target" }));
    assert_eq!(
        radiance.steps[0].inputs["white_reference"],
        json!("reference")
    );
    assert!(radiance.steps[0].inputs.get("dark_reference").is_none());
    // Detector joins are corrected once the overlap is dropped, before the
    // logarithm; each following detector starts a kept range.
    let order: Vec<&str> = radiance.steps.iter().map(|step| step.step).collect();
    assert_eq!(
        order,
        vec![
            "reflectance_from_references",
            "select_points",
            "splice_correction",
            "signal_conversion",
            "resample"
        ]
    );
    assert_eq!(
        radiance.steps[2].params["splices"],
        json!([axis[523], axis[769]])
    );
}

#[test]
fn vendor_reflectance_is_preferred_and_raw_ratios_need_equal_integration_times() {
    let records = sample("spectral_evolution/1566060_09506_working.sed");
    let plan = plan(&records, Some("winisi"), json!({}));
    assert_eq!(plan.source_signal.as_deref(), Some("reflect__%"));
    // Normalized DN are divided whatever the integration times.
    let raw = self::plan(
        &records,
        Some("winisi"),
        json!({ "signal": "norm__dn_target" }),
    );
    assert_eq!(
        raw.steps[0].inputs["white_reference"],
        json!("norm__dn_ref_")
    );
    assert!(raw.steps[0]
        .notes
        .iter()
        .any(|note| note.contains("'reflect__%'")));
    assert!(raw.notes.iter().any(|note| note.contains("not corrected")));
    let spliced = self::plan(
        &records,
        Some("winisi"),
        json!({ "signal": "norm__dn_target", "splices": [1001.0, 1801.0] }),
    );
    assert_eq!(spliced.steps[1].step, "splice_correction");

    let axis = nm(1000.0, 2.0, 8);
    let mut counts = record(
        vec![
            (
                "sample",
                signal(&axis, "nm", &[50.0; 8], SignalType::RawCounts, "sample"),
            ),
            (
                "white_reference",
                signal(
                    &axis,
                    "nm",
                    &[100.0; 8],
                    SignalType::RawCounts,
                    "white_reference",
                ),
            ),
        ],
        "test",
    );
    counts
        .metadata
        .insert("integration_time_target_ms".to_string(), json!([100.0]));
    counts
        .metadata
        .insert("integration_time_reference_ms".to_string(), json!([50.0]));
    let refused = self::plan(
        std::slice::from_ref(&counts),
        None,
        json!({ "signal_type": "reflectance" }),
    );
    assert_eq!(refused.refusals[0].code, "integration_times_differ");
    counts
        .metadata
        .insert("integration_time_reference_ms".to_string(), json!([100.0]));
    let ready = self::plan(&[counts], None, json!({ "signal_type": "reflectance" }));
    assert_eq!(ready.status, PlanStatus::Ready);
}

#[test]
fn writable_spectra_need_no_step() {
    let records = sample("foss_winisi/foss_xds_barleyground_sensAIfood.csv");
    for target in ["winisi", "foss-nsas-da", "jcamp-dx", "csv"] {
        let plan = plan(&records, Some(target), json!({}));
        assert_eq!(plan.status, PlanStatus::Unchanged, "{target}");
        assert!(plan.output.is_none());
    }
    let resampled = plan(&records, Some("winisi"), json!({ "grid": "winisi-700" }));
    assert_eq!(resampled.status, PlanStatus::Ready);
    assert_eq!(resampled.steps[0].params["tgt_n"], json!(700));
}

#[test]
fn wavenumber_axes_become_nanometres_then_a_known_grid() {
    let records = sample("bruker_opus/brukeropus_file.0");
    for (target, grid) in [("winisi", "winisi-700"), ("foss-nsas-da", "nsas-700")] {
        let plan = plan(&records, Some(target), json!({}));
        assert_eq!(plan.status, PlanStatus::Ready, "{target}");
        assert_eq!(plan.source_signal.as_deref(), Some("absorbance"));
        assert_eq!(plan.steps[0].step, "axis_unit_conversion");
        assert_eq!(plan.steps[0].params["source_unit"], json!("cm-1"));
        assert_eq!(plan.steps[0].params["target_unit"], json!("nm"));
        assert_eq!(plan.grid.as_ref().unwrap().id, grid);
    }
    // JCAMP-DX keeps wavenumbers.
    let plan = plan(&records, Some("opus"), json!({}));
    assert_eq!(plan.status, PlanStatus::Unchanged);
    let refused = self::plan(
        &records,
        Some("winisi"),
        json!({ "signal": "sample_interferogram" }),
    );
    assert_eq!(refused.refusals[0].code, "interferogram");
}

#[test]
fn missing_information_is_asked() {
    let records = sample("bruker_opus/test_spectra.0");
    let asked = plan(&records, Some("winisi"), json!({}));
    assert_eq!(asked.status, PlanStatus::NeedsAnswers);
    assert_eq!(asked.questions[0].id, "source_scale");
    let answered = plan(
        &records,
        Some("winisi"),
        json!({ "source_scale": "fraction" }),
    );
    assert_eq!(answered.status, PlanStatus::Ready);
    assert_eq!(answered.steps[0].params["is_percent"], json!(false));

    let unknown = sample("ocean_optics/OOusb4000.txt");
    let asked = plan(&unknown, Some("winisi"), json!({}));
    assert_eq!(asked.questions[0].id, "assume_signal_type");
    let answered = plan(
        &unknown,
        Some("winisi"),
        json!({ "assume_signal_type": "reflectance" }),
    );
    // A declared reflectance without unit still needs its scale.
    assert_eq!(answered.questions[0].id, "source_scale");
    let answered = plan(
        &unknown,
        Some("winisi"),
        json!({ "assume_signal_type": "reflectance", "source_scale": "percent" }),
    );
    assert_eq!(answered.status, PlanStatus::Ready);
    assert_eq!(answered.steps[0].step, "declare_signal_type");
    // Targets that store any signal take an unknown one as it is.
    assert_eq!(
        plan(&unknown, Some("jcamp-dx"), json!({})).status,
        PlanStatus::Unchanged
    );

    let absorbance = record(
        vec![(
            "signal",
            signal(
                &nm(1000.0, 2.0, 8),
                "nm",
                &[0.5; 8],
                SignalType::Absorbance,
                "signal",
            ),
        )],
        "test",
    );
    let asked = plan(
        std::slice::from_ref(&absorbance),
        None,
        json!({ "signal_type": "kubelka_munk" }),
    );
    assert_eq!(asked.questions[0].id, "absorbance_origin");
    let chained = plan(
        &[absorbance],
        None,
        json!({ "signal_type": "kubelka_munk", "absorbance_origin": "reflectance" }),
    );
    assert_eq!(
        steps(&chained),
        vec![
            (
                "signal_conversion",
                Some("preprocessing.signal_conversion.from_absorbance")
            ),
            (
                "signal_conversion",
                Some("preprocessing.signal_conversion.kubelka_munk")
            ),
        ]
    );
}

#[test]
fn what_cannot_be_converted_is_refused_with_a_fix() {
    let derivative = record(
        vec![(
            "signal",
            signal(
                &nm(1000.0, 2.0, 8),
                "nm",
                &[0.1; 8],
                SignalType::Derivative,
                "signal",
            ),
        )],
        "test",
    );
    let refused = plan(&[derivative], Some("winisi"), json!({}));
    assert_eq!(refused.status, PlanStatus::Refused);
    assert_eq!(refused.refusals[0].code, "preprocessed_signal");
    assert!(!refused.refusals[0].fix.is_empty());

    let raman = record(
        vec![(
            "signal",
            signal(
                &nm(200.0, 2.0, 8),
                "cm-1",
                &[1.0; 8],
                SignalType::Absorbance,
                "signal",
            ),
        )],
        "renishaw-wdf",
    );
    let refused = plan(&[raman], None, json!({ "axis_unit": "nm" }));
    assert_eq!(refused.refusals[0].code, "raman_shift");

    // No NSAS axis lies within 1000-1100 nm, and nothing is extrapolated.
    let narrow = record(
        vec![(
            "signal",
            signal(
                &nm(1000.0, 2.0, 51),
                "nm",
                &[0.3; 51],
                SignalType::Absorbance,
                "signal",
            ),
        )],
        "test",
    );
    let refused = plan(
        std::slice::from_ref(&narrow),
        Some("foss-nsas-da"),
        json!({}),
    );
    assert_eq!(refused.refusals[0].code, "no_known_grid");
    // WinISI writes this even axis as one segment.
    assert_eq!(
        plan(std::slice::from_ref(&narrow), Some("winisi"), json!({})).status,
        PlanStatus::Unchanged
    );
    let uneven = {
        let mut record = narrow.clone();
        let signal = record.signals.get_mut("signal").unwrap();
        signal.axis.values[3] += 0.5;
        record
    };
    let refused = plan(
        std::slice::from_ref(&uneven),
        Some("foss-nsas-da"),
        json!({}),
    );
    assert_eq!(refused.refusals[0].code, "no_known_grid");
    let refused = plan(
        std::slice::from_ref(&uneven),
        Some("winisi"),
        json!({ "grid": "winisi-1050" }),
    );
    assert_eq!(refused.refusals[0].code, "grid_outside_range");
    let refused = plan(
        std::slice::from_ref(&uneven),
        Some("winisi"),
        json!({ "grid": "even", "grid_min": 990.0 }),
    );
    assert_eq!(refused.refusals[0].code, "grid_outside_range");
    let even = plan(
        &[uneven],
        Some("winisi"),
        json!({ "grid": "even", "grid_step": 4.0, "grid_min": 1010.0 }),
    );
    let grid = even.grid.unwrap();
    assert_eq!((grid.first, grid.last, grid.points), (1012.0, 1100.0, 23));
    assert!(even.steps[0]
        .notes
        .iter()
        .any(|note| note.contains("never observed")));

    assert!(plan_conversion(
        std::slice::from_ref(&narrow),
        Some("nowhere"),
        &PlanOptions::default()
    )
    .is_err());
    assert!(plan_conversion(
        &[narrow],
        None,
        &options(json!({ "interpolation": "spline" }))
    )
    .is_err());
}

#[test]
fn resampling_never_leaves_the_measured_range() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples");
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                paths.push(path);
            }
        }
    }
    let mut planned = 0;
    for path in paths {
        let Ok(mut records) = open_path(&path) else {
            continue;
        };
        records.truncate(20);
        for target in ["winisi", "foss-nsas-da", "jcamp-dx", "bruker-dpt"] {
            let plan = plan_conversion(&records, Some(target), &PlanOptions::default())
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            if let (Some(grid), Some(range)) = (&plan.grid, plan.measured_range) {
                assert!(
                    grid.first >= range[0] && grid.last <= range[1],
                    "{}: {target}",
                    path.display()
                );
                planned += 1;
            }
            assert_eq!(
                plan.status == PlanStatus::Refused,
                !plan.refusals.is_empty()
            );
        }
    }
    assert!(planned > 10, "{planned}");
}

#[test]
fn writers_report_and_embed_applied_conversions() {
    let mut converted = record(
        vec![(
            "absorbance (from reflectance)",
            signal(
                &nm(1000.0, 2.0, 8),
                "nm",
                &[0.5; 8],
                SignalType::Absorbance,
                "converted",
            ),
        )],
        "test",
    );
    converted.metadata.insert(
        CONVERSIONS_KEY.to_string(),
        json!([{
            "signal": "absorbance (from reflectance)",
            "step": "signal_conversion",
            "method_id": "preprocessing.signal_conversion.to_absorbance",
            "methods_version": "1.5.1",
            "params": { "is_percent": true },
            "notes": ["apparent absorbance"]
        }]),
    );
    let records = [converted.clone()];
    let jcamp = write_records(&records, "jcamp-dx", &WriteOptions::default()).unwrap();
    let text = String::from_utf8(jcamp.files[0].bytes.clone()).unwrap();
    for line in [
        "##$CONVERSION 1=signal_conversion\r\n",
        "##$CONVERSION 1 METHOD=preprocessing.signal_conversion.to_absorbance\r\n",
        "##$CONVERSION 1 NIRS4ALL-METHODS=1.5.1\r\n",
    ] {
        assert!(text.contains(line), "{line}");
    }
    assert!(!jcamp
        .losses
        .iter()
        .any(|loss| loss.code == "text_truncated"));
    assert!(text.contains("##$CONVERSION 1 IS_PERCENT=true\r\n"));
    assert_eq!(jcamp.conversions.len(), 1);
    assert!(!jcamp
        .losses
        .iter()
        .any(|loss| loss.code == "conversions_not_written"));
    let back = nirs4all_formats::open_bytes(Path::new("a.jdx"), &jcamp.files[0].bytes).unwrap();
    assert_eq!(
        back[0].signals.values().next().unwrap().values,
        vec![0.5; 8]
    );

    let spc = write_records(&records, "galactic-spc", &WriteOptions::default()).unwrap();
    let bytes = &spc.files[0].bytes;
    let log = String::from_utf8_lossy(bytes);
    assert!(log.contains("CONVERSION1 = signal_conversion"));
    assert!(log.contains("CONVERSION1 is_percent = true"));

    for target in ["csv", "winisi"] {
        let output = write_records(&records, target, &WriteOptions::default()).unwrap();
        assert!(
            output
                .losses
                .iter()
                .any(|loss| loss.code == "conversions_not_written"),
            "{target}"
        );
    }

    // Conversions of a signal that is not exported are not reported.
    let mut both = converted.clone();
    both.signals.insert(
        "raw".to_string(),
        signal(
            &nm(1000.0, 2.0, 8),
            "nm",
            &[0.4; 8],
            SignalType::Absorbance,
            "raw",
        ),
    );
    let only_raw: WriteOptions = serde_json::from_value(json!({ "signal": "raw" })).unwrap();
    let output = write_records(&[both], "jcamp-dx", &only_raw).unwrap();
    assert!(output.conversions.is_empty());
    let text = String::from_utf8(output.files[0].bytes.clone()).unwrap();
    assert!(!text.contains("CONVERSION"));

    let mut malformed = converted;
    malformed
        .metadata
        .insert(CONVERSIONS_KEY.to_string(), json!("resampled"));
    assert!(write_records(&[malformed], "csv", &WriteOptions::default()).is_err());
}
