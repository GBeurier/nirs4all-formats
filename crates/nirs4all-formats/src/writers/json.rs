//! Complete normalized records as JSON.

use nirs4all_formats_core::SpectralRecord;

use super::{refused, Context, WriteError, WrittenFile};

pub(crate) fn write(
    records: &[SpectralRecord],
    context: &mut Context<'_>,
) -> Result<Vec<WrittenFile>, WriteError> {
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
