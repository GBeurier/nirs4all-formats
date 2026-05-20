use std::error::Error;
use std::path::{Path, PathBuf};

use nirs4all_io::{open_path, SpectralRecord};
use serde_json::{json, Value};

const CASES: &[(&str, &str)] = &[
    ("asd_legacy_float", "samples/asd/3L9257.000"),
    ("asd_v6_double", "samples/asd/v6sample00000.asd"),
    ("asd_v7_field_double", "samples/asd/v7_field_44231B009.asd"),
    ("asd_v8_double", "samples/asd/v8sample00001.asd"),
    ("csv_synthetic", "samples/csv_tsv/synthetic_nirs.csv"),
    ("bruker_dpt_synthetic", "samples/bruker_dpt/synthetic.dpt"),
    ("avantes_wave_table", "samples/avantes/avantes_export.ttt"),
    ("avantes_irradiance", "samples/avantes/irr_820_1941.IRR"),
    ("jcamp_nist_water", "samples/jcamp_dx/nist_water_ir.jdx"),
    ("galactic_spc_benzene", "samples/galactic_spc/BENZENE.SPC"),
    ("galactic_spc_s_xy", "samples/galactic_spc/s_xy.spc"),
    ("galactic_spc_nir_multi", "samples/galactic_spc/nir.spc"),
    ("galactic_spc_m_xyxy", "samples/galactic_spc/m_xyxy.spc"),
    (
        "spectral_evolution_sed",
        "samples/spectral_evolution/1566060_09506_working.sed",
    ),
    ("svc_sig_moc", "samples/svc_ger/BNL13001_000_moc.sig"),
];

#[test]
fn reader_outputs_match_golden_summaries() -> Result<(), Box<dyn Error>> {
    for (name, relative_path) in CASES {
        let records = open_path(workspace_file(relative_path))?;
        let actual = serde_json::to_string_pretty(&summarize_records(&records))? + "\n";
        let golden_path = golden_file(name);

        if std::env::var("NIRS4ALL_IO_ACCEPT_GOLDENS").as_deref() == Ok("1") {
            std::fs::create_dir_all(golden_path.parent().expect("golden parent"))?;
            std::fs::write(&golden_path, actual)?;
            continue;
        }

        let expected = std::fs::read_to_string(&golden_path)?;
        assert_eq!(actual, expected, "golden mismatch for {name}");
    }
    Ok(())
}

fn summarize_records(records: &[SpectralRecord]) -> Value {
    json!({
        "record_count": records.len(),
        "records": records.iter().map(summarize_record).collect::<Vec<_>>(),
    })
}

fn summarize_record(record: &SpectralRecord) -> Value {
    let signals = record
        .signals
        .iter()
        .map(|(name, signal)| {
            json!({
                "name": name,
                "axis_kind": &signal.axis.kind,
                "axis_order": &signal.axis.order,
                "axis_unit": signal.axis.unit,
                "axis_len": signal.axis.values.len(),
                "axis_first": round6(signal.axis.values[0]),
                "axis_last": round6(signal.axis.values[signal.axis.values.len() - 1]),
                "signal_type": &signal.signal_type,
                "unit": signal.unit,
                "role": signal.role,
                "values_len": signal.values.len(),
                "value_first": round6(signal.values[0]),
                "value_last": round6(signal.values[signal.values.len() - 1]),
                "value_sum": round6(signal.values.iter().sum::<f64>()),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "format": record.provenance.format,
        "reader": record.provenance.reader,
        "record_signal_type": &record.signal_type,
        "signal_count": record.signals.len(),
        "signals": signals,
        "target_keys": record.targets.keys().collect::<Vec<_>>(),
        "metadata_keys": record.metadata.keys().collect::<Vec<_>>(),
        "quality_flags": record.quality_flags,
        "warnings": record.provenance.warnings,
        "source_sha256": record.provenance.sources.first().map(|source| &source.sha256),
    })
}

fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn workspace_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn golden_file(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens")
        .join(format!("{name}.summary.json"))
}
