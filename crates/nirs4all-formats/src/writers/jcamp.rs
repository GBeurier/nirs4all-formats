//! JCAMP-DX 4.24, one uncompressed AFFN spectrum per file.

use nirs4all_formats_core::{AxisKind, SignalType, SpectralRecord};

use super::select::{
    even_spacing, report_even_spacing, report_record_losses, spectra, target_text, Scale, Spectrum,
    TargetSupport, Technique,
};
use super::{conversions, exponent_text, number, refused, Context, WriteError, WrittenFile};

/// Largest integer ordinate; readers commonly use 32-bit signed integers.
const MAX_ORDINATE: f64 = i32::MAX as f64;
const LINE_WIDTH: usize = 80;

pub(crate) fn write(
    records: &[SpectralRecord],
    context: &mut Context<'_>,
) -> Result<Vec<WrittenFile>, WriteError> {
    let mut spectra = spectra(records, context)?;
    // Interferograms need RA parameters (RUNITS, RESOLUTION...) records do not hold.
    let total = spectra.len();
    spectra.retain(|spectrum| spectrum.signal.signal_type != SignalType::Interferogram);
    if spectra.is_empty() {
        return Err(refused(
            "JCAMP-DX interferograms need acquisition parameters these records do not hold",
        ));
    }
    if spectra.len() < total {
        context.losses.add(
            "interferograms_omitted",
            format!(
                "{} interferogram(s) are not written: JCAMP-DX needs acquisition parameters the records do not hold",
                total - spectra.len()
            ),
        );
    }
    report_record_losses(records, context, TargetSupport::Comments);
    let single = spectra.len() == 1;
    spectra
        .iter()
        .map(|spectrum| {
            spectrum.require_finite("JCAMP-DX")?;
            let text = spectrum_text(spectrum, context);
            let id = if single { "" } else { spectrum.id.as_str() };
            Ok(context.file(&[id, &spectrum.signal_label], text.into_bytes()))
        })
        .collect()
}

fn spectrum_text(spectrum: &Spectrum<'_>, context: &mut Context<'_>) -> String {
    let axis = &spectrum.axis.values;
    let values = &spectrum.values;
    let even = even_spacing(axis, context.options.axis_tolerance);
    let mut labels: Vec<(String, String)> = Vec::new();
    let mut ldr = |label: &str, value: String| labels.push((label.to_string(), value));
    let title = ascii(&spectrum.id, context);
    ldr(
        "TITLE",
        if spectrum.signal_label.is_empty() {
            title
        } else {
            format!("{title} [{}]", ascii(&spectrum.signal_label, context))
        },
    );
    ldr("JCAMP-DX", "4.24".to_string());
    ldr("DATA TYPE", data_type(spectrum, context).to_string());
    ldr(
        "ORIGIN",
        format!(
            "nirs4all-formats {} from {}",
            env!("CARGO_PKG_VERSION"),
            ascii(&spectrum.record.provenance.format, context)
        ),
    );
    ldr("OWNER", "UNKNOWN".to_string());
    ldr("XUNITS", x_units(spectrum, context));
    ldr("YUNITS", y_units(spectrum, context));

    let (min_x, max_x) = bounds(axis);
    let (min_y, max_y) = bounds(values);
    let exponent = y_exponent(values);
    let factor = format!("1E{}", exponent_text(exponent));
    ldr("XFACTOR", "1.0".to_string());
    ldr(
        "YFACTOR",
        if even.is_some() {
            factor.clone()
        } else {
            "1.0".to_string()
        },
    );
    ldr("FIRSTX", number(axis[0]));
    ldr("LASTX", number(axis[axis.len() - 1]));
    if even.is_some() {
        let delta = (axis[axis.len() - 1] - axis[0]) / (axis.len() - 1) as f64;
        ldr("DELTAX", number(delta));
    }
    ldr("MINX", number(min_x));
    ldr("MAXX", number(max_x));
    ldr("MINY", number(min_y));
    ldr("MAXY", number(max_y));
    ldr("NPOINTS", axis.len().to_string());
    // FIRSTY checks the table, so it is the ordinate the table holds.
    let first = if even.is_some() {
        scaled(values[0], exponent) as f64 * decimal_factor(exponent)
    } else {
        values[0]
    };
    ldr("FIRSTY", number(first));
    for (key, value) in &spectrum.record.targets {
        if let Some(text) = target_text(value) {
            let key = fit(&ascii(key, context).replace('=', " "), 40, context);
            ldr(&format!("$TARGET {key}"), ascii(&text, context));
        }
    }
    let applied = conversions::applied(spectrum.record, spectrum.signal_name);
    for (index, conversion) in applied.iter().enumerate() {
        let label = format!("$CONVERSION {}", index + 1);
        ldr(&label, ascii(&conversion.step, context));
        if let Some(method) = &conversion.method_id {
            ldr(&format!("{label} METHOD"), ascii(method, context));
        }
        if let Some(version) = &conversion.methods_version {
            ldr(
                &format!("{label} NIRS4ALL-METHODS"),
                ascii(version, context),
            );
        }
        for (name, value) in conversion.param_lines() {
            let name = fit(&ascii(&name, context).replace('=', " "), 30, context);
            ldr(
                &format!("{label} {}", name.to_ascii_uppercase()),
                ascii(&value, context),
            );
        }
    }
    let mut out = String::new();
    for (label, value) in &labels {
        let value = fit(value, LINE_WIDTH - label.len() - 3, context);
        out.push_str(&format!("##{label}={value}\r\n"));
    }

    match even {
        Some(deviation) => {
            report_even_spacing(context, axis, deviation);
            out.push_str("##XYDATA=(X++(Y..Y))\r\n");
            let error = push_xydata(&mut out, axis, values, exponent);
            // Rounding of the decimal factor itself stays below this bound.
            let scale = values
                .iter()
                .fold(0.0f64, |acc, value| acc.max(value.abs()));
            if error > 4.0 * f64::EPSILON * scale {
                context.losses.add_error(
                    "ordinate_quantized",
                    "ordinates are stored as 32-bit integers times YFACTOR",
                    Some(error),
                );
            }
        }
        None => {
            out.push_str("##XYPOINTS=(XY..XY)\r\n");
            for (x, y) in axis.iter().zip(values.iter()) {
                out.push_str(&format!("{},{}\r\n", number(*x), number(*y)));
            }
        }
    }
    out.push_str("##END=\r\n");
    out
}

/// Append XYDATA lines and return the largest ordinate error.
fn push_xydata(out: &mut String, axis: &[f64], values: &[f64], exponent: i32) -> f64 {
    let step = (axis[axis.len() - 1] - axis[0]) / (axis.len() - 1) as f64;
    let factor = decimal_factor(exponent);
    let mut error = 0.0f64;
    let mut line = String::new();
    for (index, value) in values.iter().enumerate() {
        let ordinate = scaled(*value, exponent);
        error = error.max((ordinate as f64 * factor - value).abs());
        let token = format!(" {ordinate}");
        if line.len() + token.len() > LINE_WIDTH {
            out.push_str(&line);
            out.push_str("\r\n");
            line.clear();
        }
        if line.is_empty() {
            line = number(axis[0] + step * index as f64);
        }
        line.push_str(&token);
    }
    out.push_str(&line);
    out.push_str("\r\n");
    error
}

/// The factor as readers parse it from its decimal text.
fn decimal_factor(exponent: i32) -> f64 {
    format!("1E{exponent}").parse().expect("decimal factor")
}

/// Shorten text to the space its line has left.
fn fit(text: &str, width: usize, context: &mut Context<'_>) -> String {
    if text.len() <= width {
        return text.to_string();
    }
    context.losses.add(
        "text_truncated",
        "labels are shortened to the 80-character JCAMP-DX line",
    );
    // Labels are ASCII, so any byte position is a character boundary.
    text[..width].trim_end().to_string()
}

/// `value / 10^exponent`, rounded, through exact powers of ten when possible.
fn scaled(value: f64, exponent: i32) -> i64 {
    let scaled = if exponent <= 0 {
        value * 10f64.powi(-exponent)
    } else {
        value / 10f64.powi(exponent)
    };
    scaled.round() as i64
}

/// Smallest decimal exponent keeping every ordinate inside 32 bits.
fn y_exponent(values: &[f64]) -> i32 {
    let max = values
        .iter()
        .fold(0.0f64, |acc, value| acc.max(value.abs()));
    if max == 0.0 {
        return 0;
    }
    let mut exponent = (max / MAX_ORDINATE).log10().ceil() as i32;
    while scaled(max, exponent) as f64 > MAX_ORDINATE {
        exponent += 1;
    }
    exponent
}

fn bounds(values: &[f64]) -> (f64, f64) {
    values
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| {
            (low.min(*value), high.max(*value))
        })
}

fn data_type(spectrum: &Spectrum<'_>, context: &mut Context<'_>) -> &'static str {
    match spectrum.technique() {
        Technique::Infrared => "INFRARED SPECTRUM",
        Technique::Raman => "RAMAN SPECTRUM",
        Technique::UvVis => "UV/VIS SPECTRUM",
        Technique::Unknown => {
            context.losses.add(
                "technique_not_known",
                "the technique is not known; DATA TYPE is written as INFRARED SPECTRUM because the format requires one",
            );
            "INFRARED SPECTRUM"
        }
    }
}

fn x_units(spectrum: &Spectrum<'_>, context: &mut Context<'_>) -> String {
    let unit = spectrum.axis.unit.trim();
    match (&spectrum.axis.kind, unit.to_ascii_lowercase().as_str()) {
        (AxisKind::Wavenumber, "cm-1" | "1/cm" | "cm^-1") => "1/CM".to_string(),
        (AxisKind::Wavelength, "nm") => "NANOMETERS".to_string(),
        (AxisKind::Wavelength, "um" | "µm") => "MICROMETERS".to_string(),
        (AxisKind::Time, "s") => "SECONDS".to_string(),
        (kind, _) => {
            context.losses.add(
                "axis_unit_non_standard",
                format!("axis unit '{unit}' ({kind:?}) is outside the JCAMP-DX 4.24 vocabulary"),
            );
            ascii(unit, context).to_ascii_uppercase()
        }
    }
}

fn y_units(spectrum: &Spectrum<'_>, context: &mut Context<'_>) -> String {
    let standard = match spectrum.signal.signal_type {
        SignalType::Absorbance => Some("ABSORBANCE"),
        SignalType::Reflectance => Some("REFLECTANCE"),
        SignalType::Transmittance => Some("TRANSMITTANCE"),
        SignalType::KubelkaMunk => Some("KUBELKA-MUNK"),
        _ => None,
    };
    let scale = spectrum.scale();
    match (standard, &scale) {
        (Some(label), Scale::Plain) => label.to_string(),
        // Ordinates are never rescaled; the label tells they are percentages.
        (Some(label), Scale::Percent) => {
            context.losses.add(
                "signal_unit_non_standard",
                "percent ordinates are labelled PERCENT, which some software ignores",
            );
            format!("PERCENT {label}")
        }
        (Some(_), Scale::Other(unit)) => {
            context.losses.add(
                "signal_unit_not_stored",
                format!(
                    "{} in '{unit}' is written as ARBITRARY UNITS",
                    spectrum.type_label()
                ),
            );
            "ARBITRARY UNITS".to_string()
        }
        (None, _) => {
            context.losses.add(
                "signal_type_not_stored",
                format!(
                    "signal type {} is outside the JCAMP-DX 4.24 vocabulary and is written as ARBITRARY UNITS",
                    spectrum.described()
                ),
            );
            "ARBITRARY UNITS".to_string()
        }
    }
}

/// JCAMP-DX text is single-line ASCII without record markers.
fn ascii(text: &str, context: &mut Context<'_>) -> String {
    let clean: String = text
        .chars()
        .map(|ch| match ch {
            ' '..='~' => ch,
            _ => '_',
        })
        .collect::<String>()
        .replace("##", "__")
        .replace("$$", "__");
    if clean != text {
        context.losses.add(
            "text_sanitized",
            "characters a label cannot hold are replaced by '_'",
        );
    }
    clean.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinates_fit_32_bits_at_any_scale() {
        for max in [1e-12, 0.9999, 2.147483647, 2.2, 65535.0, 3e9, 1e30] {
            let exponent = y_exponent(&[max, -max / 3.0]);
            assert!(scaled(max, exponent) as f64 <= MAX_ORDINATE, "{max}");
            assert!(scaled(max, exponent) >= 100_000_000, "{max}");
        }
        assert_eq!(y_exponent(&[0.0]), 0);
    }
}
