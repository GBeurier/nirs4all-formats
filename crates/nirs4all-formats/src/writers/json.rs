//! Complete normalized records as JSON.

use nirs4all_formats_core::SpectralRecord;

use super::{refused, Context, WriteError, WrittenFile};

pub(crate) fn write(
    records: &[SpectralRecord],
    context: &mut Context<'_>,
) -> Result<Vec<WrittenFile>, WriteError> {
    let selected: Vec<SpectralRecord>;
    let records = match context.options.signal.as_deref() {
        None => records,
        Some(wanted) => {
            selected = records
                .iter()
                .filter_map(|record| {
                    let mut record = record.clone();
                    let before = record.signals.len();
                    record.signals.retain(|name, _| name == wanted);
                    if record.signals.len() < before {
                        context.losses.add(
                            "signals_omitted",
                            "signals other than the selected one are not written",
                        );
                    }
                    (!record.signals.is_empty()).then_some(record)
                })
                .collect();
            if selected.is_empty() {
                return Err(refused(format!("no record has signal '{wanted}'")));
            }
            &selected
        }
    };
    for record in records {
        let finite = record
            .signals
            .values()
            .all(|signal| signal.values.iter().all(|value| value.is_finite()));
        if !finite {
            context.losses.add(
                "nonfinite_as_null",
                "missing and non-finite values are written as null",
            );
        }
    }
    let bytes = serde_json::to_vec_pretty(records).map_err(|error| refused(error.to_string()))?;
    Ok(vec![context.file(&[], bytes)])
}
