//! Two-column text, one spectrum per file.

use nirs4all_formats_core::{AxisKind, SpectralRecord};

use super::select::{report_record_losses, spectra, Spectrum, TargetSupport};
use super::{number, refused, Context, WriteError, WrittenFile};

pub(crate) fn write_xy(
    records: &[SpectralRecord],
    context: &mut Context<'_>,
) -> Result<Vec<WrittenFile>, WriteError> {
    let delimiter = context.options.delimiter.unwrap_or('\t');
    let spectra = spectra(records, context)?;
    write(records, &spectra, context, delimiter)
}

/// OPUS data point tables carry no unit, so only wavenumber axes are written.
pub(crate) fn write_dpt(
    records: &[SpectralRecord],
    context: &mut Context<'_>,
) -> Result<Vec<WrittenFile>, WriteError> {
    let spectra = spectra(records, context)?;
    for spectrum in &spectra {
        if spectrum.axis.kind != AxisKind::Wavenumber || spectrum.axis.unit != "cm-1" {
            return Err(refused(format!(
                "OPUS data point tables hold wavenumbers in cm-1; '{}' has a {:?} axis in '{}'",
                spectrum.id, spectrum.axis.kind, spectrum.axis.unit
            )));
        }
    }
    write(records, &spectra, context, ',')
}

fn write(
    records: &[SpectralRecord],
    spectra: &[Spectrum<'_>],
    context: &mut Context<'_>,
    delimiter: char,
) -> Result<Vec<WrittenFile>, WriteError> {
    report_record_losses(records, context, TargetSupport::None);
    context.losses.add(
        "axis_and_signal_not_described",
        "the file holds numbers only: axis unit, signal type and identifier are not stored",
    );
    let single = spectra.len() == 1;
    spectra
        .iter()
        .map(|spectrum| {
            spectrum.require_finite("two-column text")?;
            let mut out = String::new();
            for (x, y) in spectrum.axis.values.iter().zip(spectrum.values.iter()) {
                out.push_str(&format!("{}{delimiter}{}\r\n", number(*x), number(*y)));
            }
            let id = if single { "" } else { spectrum.id.as_str() };
            Ok(context.file(&[id, &spectrum.signal_label], out.into_bytes()))
        })
        .collect()
}
