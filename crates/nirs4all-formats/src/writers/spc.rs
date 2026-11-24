//! Galactic SPC, new little-endian layout (0x4B) with 32-bit float ordinates.

use nirs4all_formats_core::{AxisKind, SignalType, SpectralRecord};

use super::select::{
    even_spacing, groups, report_even_spacing, report_record_losses, spectra, target_text, Scale,
    Spectrum, TargetSupport, Technique,
};
use super::{refused, Context, WriteError, WrittenFile};

const HEADER: usize = 512;
const SUBHEADER: usize = 32;
const LOG_HEADER: usize = 64;
const FLAG_MULTI: u8 = 0x04;
const FLAG_LABELS: u8 = 0x20;
const FLAG_XVALS: u8 = 0x80;
/// Exponent byte announcing IEEE 32-bit floats.
const FLOAT_EXPONENT: u8 = 0x80;
/// Subfile indices are 16-bit.
const MAX_SUBFILES: usize = 32_000;

pub(crate) fn write(
    records: &[SpectralRecord],
    context: &mut Context<'_>,
) -> Result<Vec<WrittenFile>, WriteError> {
    let spectra = spectra(records, context)?;
    report_record_losses(records, context, TargetSupport::Comments);
    for spectrum in &spectra {
        spectrum.require_finite("SPC")?;
    }
    let mut files = Vec::new();
    if context.options.multi {
        let groups = groups(&spectra);
        let several = groups.len() > 1;
        for (index, group) in groups.iter().enumerate() {
            for (part, chunk) in group.chunks(MAX_SUBFILES).enumerate() {
                let bytes = file_bytes(chunk, context)?;
                let grid = if several {
                    format!("grid{}", index + 1)
                } else {
                    String::new()
                };
                let part = if part > 0 {
                    format!("part{}", part + 1)
                } else {
                    String::new()
                };
                let label = chunk[0].signal_label.clone();
                files.push(context.file(&[&label, &grid, &part], bytes));
            }
        }
    } else {
        for spectrum in &spectra {
            let bytes = file_bytes(&[spectrum], context)?;
            let id = if spectra.len() == 1 {
                ""
            } else {
                spectrum.id.as_str()
            };
            files.push(context.file(&[id, &spectrum.signal_label], bytes));
        }
    }
    Ok(files)
}

fn file_bytes(spectra: &[&Spectrum<'_>], context: &mut Context<'_>) -> Result<Vec<u8>, WriteError> {
    let head = spectra[0];
    let axis = &head.axis.values;
    let points = i32::try_from(axis.len()).map_err(|_| refused("too many points for SPC"))?;
    let even = even_spacing(axis, context.options.axis_tolerance);
    let multi = spectra.len() > 1;
    let (x_code, x_label) = x_code(head);
    let (y_code, y_label) = y_code(head);
    if y_code == 0 {
        context.losses.add(
            "signal_type_not_stored",
            format!(
                "signal type {} has no SPC code and is written as an axis label",
                head.described()
            ),
        );
    }
    if multi {
        context.losses.add(
            "identifiers_in_log",
            "spectrum identifiers are written in the file log, which some software ignores",
        );
    }

    let mut flags = 0;
    if multi {
        flags |= FLAG_MULTI;
    }
    if even.is_none() {
        flags |= FLAG_XVALS;
    }
    if x_label.is_some() || y_label.is_some() {
        flags |= FLAG_LABELS;
    }

    let mut out = vec![0u8; HEADER];
    out[0] = flags;
    out[1] = 0x4B;
    out[2] = experiment(head);
    out[3] = FLOAT_EXPONENT;
    out[4..8].copy_from_slice(&points.to_le_bytes());
    out[8..16].copy_from_slice(&axis[0].to_le_bytes());
    out[16..24].copy_from_slice(&axis[axis.len() - 1].to_le_bytes());
    out[24..28].copy_from_slice(&(spectra.len() as i32).to_le_bytes());
    out[28] = x_code;
    out[29] = y_code;
    let memo = if multi {
        format!("{} spectra", spectra.len())
    } else {
        latin(&head.id, context)
    };
    if memo.len() > 129 {
        context.losses.add(
            "text_truncated",
            "identifiers are shortened to the 129-character SPC memo",
        );
    }
    put_text(&mut out[88..218], &memo);
    if x_label.is_some() || y_label.is_some() {
        // An empty label leaves the meaning of the standard code unchanged.
        let labels = [x_label.unwrap_or_default(), y_label.unwrap_or_default()];
        let text = labels
            .iter()
            .map(|label| latin(label, context))
            .collect::<Vec<_>>()
            .join("\0");
        if text.len() > 29 {
            context.losses.add(
                "axis_label_truncated",
                "axis labels exceed the 30-byte SPC field",
            );
        }
        put_text(&mut out[218..248], &text);
    }
    out[312..316].copy_from_slice(&1f32.to_le_bytes());

    match even {
        Some(deviation) => report_even_spacing(context, axis, deviation),
        None => push_f32(
            &mut out,
            axis,
            context,
            "axis_rounded_f32",
            "explicit abscissae are stored as 32-bit floats",
        )?,
    }
    for (index, spectrum) in spectra.iter().enumerate() {
        let mut sub = [0u8; SUBHEADER];
        sub[1] = FLOAT_EXPONENT;
        sub[2..4].copy_from_slice(&(index as i16).to_le_bytes());
        sub[4..8].copy_from_slice(&(index as f32).to_le_bytes());
        sub[8..12].copy_from_slice(&((index + 1) as f32).to_le_bytes());
        out.extend_from_slice(&sub);
        push_f32(
            &mut out,
            &spectrum.values,
            context,
            "ordinate_rounded_f32",
            "ordinates are stored as 32-bit floats",
        )?;
    }

    let log = log_text(spectra, context);
    let offset = i32::try_from(out.len()).map_err(|_| refused("SPC file exceeds 2 GiB"))?;
    out[248..252].copy_from_slice(&offset.to_le_bytes());
    let mut header = [0u8; LOG_HEADER];
    // Both sizes cover the whole block, its header included; the memory image
    // is rounded up to 4096 bytes by convention.
    let size = (LOG_HEADER + log.len()) as i32;
    header[0..4].copy_from_slice(&size.to_le_bytes());
    let memory = (size + 4095) / 4096 * 4096;
    header[4..8].copy_from_slice(&memory.to_le_bytes());
    header[8..12].copy_from_slice(&(LOG_HEADER as i32).to_le_bytes());
    out.extend_from_slice(&header);
    out.extend_from_slice(log.as_bytes());
    Ok(out)
}

fn push_f32(
    out: &mut Vec<u8>,
    values: &[f64],
    context: &mut Context<'_>,
    code: &'static str,
    message: &str,
) -> Result<(), WriteError> {
    let mut error = 0.0f64;
    for value in values {
        let stored = *value as f32;
        if !stored.is_finite() {
            return Err(refused(format!("{value} exceeds the 32-bit float range")));
        }
        error = error.max((f64::from(stored) - value).abs());
        out.extend_from_slice(&stored.to_le_bytes());
    }
    if error > 0.0 {
        context.losses.add_error(code, message, Some(error));
    }
    Ok(())
}

/// Log text: sample names and targets, one `KEY = value` line each.
fn log_text(spectra: &[&Spectrum<'_>], context: &mut Context<'_>) -> String {
    let mut out = format!(
        "SOURCE FORMAT = {}\r\nWRITER = nirs4all-formats {}\r\n",
        latin(&spectra[0].record.provenance.format, context),
        env!("CARGO_PKG_VERSION")
    );
    for (index, spectrum) in spectra.iter().enumerate() {
        let id = latin(&spectrum.id, context).replace('"', "'");
        out.push_str(&format!(
            "SUBFILE{} = {}, \"{id}\"\r\n",
            index + 1,
            index + 1
        ));
        for (key, value) in &spectrum.record.targets {
            if let Some(text) = target_text(value) {
                let key = latin(key, context).replace('=', " ");
                let text = latin(&text, context);
                out.push_str(&format!("TARGET{} {key} = {text}\r\n", index + 1));
            }
        }
    }
    out.push('\0');
    out
}

/// Plain single-line ASCII text; SPC fields have no declared encoding.
fn latin(text: &str, context: &mut Context<'_>) -> String {
    let clean: String = text
        .chars()
        .map(|ch| if (' '..='~').contains(&ch) { ch } else { '_' })
        .collect();
    if clean != text {
        context.losses.add(
            "text_sanitized",
            "characters a label cannot hold are replaced by '_'",
        );
    }
    clean
}

fn put_text(field: &mut [u8], text: &str) {
    // Keep the final NUL.
    let length = text.len().min(field.len() - 1);
    field[..length].copy_from_slice(&text.as_bytes()[..length]);
}

fn experiment(spectrum: &Spectrum<'_>) -> u8 {
    let nir = spectrum.axis.unit == "nm" && spectrum.axis.values.iter().all(|x| *x >= 700.0);
    match spectrum.technique() {
        Technique::Raman => 10,
        Technique::UvVis => 6,
        Technique::Infrared if nir => 5,
        _ => 0,
    }
}

/// X type code, with an explicit label when no standard code fits.
fn x_code(spectrum: &Spectrum<'_>) -> (u8, Option<String>) {
    let unit = spectrum.axis.unit.trim();
    let code = match (&spectrum.axis.kind, unit.to_ascii_lowercase().as_str()) {
        (AxisKind::Wavenumber, "cm-1" | "1/cm" | "cm^-1") => {
            if spectrum.technique() == Technique::Raman {
                13
            } else {
                1
            }
        }
        (_, "m/z") => 9,
        (AxisKind::Wavelength, "um" | "µm") => 2,
        (AxisKind::Wavelength, "nm") => 3,
        (AxisKind::Time, "s") => 4,
        (AxisKind::Time, "min") => 5,
        (AxisKind::Time, "ms") => 23,
        (AxisKind::Time, "us") => 24,
        (AxisKind::Time, "ns") => 25,
        (AxisKind::Time, "h") => 30,
        (AxisKind::Frequency, "hz") => 6,
        (AxisKind::Frequency, "khz") => 7,
        (AxisKind::Frequency, "mhz") => 8,
        (AxisKind::Frequency, "ghz") => 26,
        (AxisKind::Energy, "ev") => 14,
        _ => return (0, (!unit.is_empty()).then(|| unit.to_string())),
    };
    (code, None)
}

fn y_code(spectrum: &Spectrum<'_>) -> (u8, Option<String>) {
    let code = match spectrum.signal.signal_type {
        SignalType::Absorbance => 2,
        SignalType::Reflectance => 129,
        SignalType::Transmittance => 128,
        SignalType::KubelkaMunk => 3,
        SignalType::Interferogram => 1,
        SignalType::SingleBeam => 130,
        SignalType::RawCounts => 4,
        _ => 0,
    };
    match spectrum.scale() {
        Scale::Plain if code != 0 => (code, None),
        // Ordinates are never rescaled; the label tells they are percentages.
        Scale::Percent if code != 0 => (code, Some(spectrum.described())),
        // A code would announce the plain unit, so other units use a label only.
        _ => (0, Some(spectrum.described())),
    }
}
