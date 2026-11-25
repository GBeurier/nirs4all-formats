//! Rules shared by the fixed-layout FOSS targets.

use std::collections::{BTreeMap, BTreeSet};

use nirs4all_formats_core::{AxisKind, SignalType};
use serde_json::Value;

use super::select::{Scale, Spectrum};
use super::{ascii_text, refused, Context, WriteError};

/// Both formats hold absorbance along nanometres and say so nowhere.
pub(super) fn require_absorbance_nm(
    spectra: &[Spectrum<'_>],
    target: &str,
    context: &mut Context<'_>,
) -> Result<(), WriteError> {
    for spectrum in spectra {
        spectrum.require_finite(target)?;
        let axis = spectrum.axis;
        if axis.kind != AxisKind::Wavelength || axis.unit != "nm" {
            return Err(refused(format!(
                "{target} stores wavelengths in nm, spectrum '{}' has an axis in '{}'",
                spectrum.id, axis.unit
            )));
        }
        match (&spectrum.signal.signal_type, spectrum.scale()) {
            (SignalType::Absorbance, Scale::Plain) => {}
            (SignalType::Unknown, Scale::Plain) => context.losses.add(
                "signal_type_assumed",
                "spectra of unknown signal type are read as absorbance (log 1/R)",
            ),
            _ => {
                return Err(refused(format!(
                    "{target} stores absorbance (log 1/R), spectrum '{}' is {}",
                    spectrum.id,
                    spectrum.described()
                )))
            }
        }
    }
    Ok(())
}

/// Labels cut to `width` bytes. Equal labels stay equal, different labels stay
/// different.
pub(super) fn short_labels(
    labels: &[&str],
    width: usize,
    context: &mut Context<'_>,
) -> (Vec<String>, bool) {
    let clean: BTreeMap<&str, String> = labels
        .iter()
        .map(|label| {
            let text = ascii_text(label.trim(), context);
            (
                *label,
                if text.is_empty() {
                    "x".to_string()
                } else {
                    text
                },
            )
        })
        .collect();
    // Labels that fit unchanged keep their text.
    let mut used: BTreeSet<String> = clean
        .iter()
        .filter(|(label, text)| text.len() <= width && **label == text.as_str())
        .map(|(_, text)| text.clone())
        .collect();
    let mut short = BTreeMap::new();
    let mut next: BTreeMap<String, usize> = BTreeMap::new();
    let mut shortened = false;
    for label in labels {
        let text = &clean[label];
        if short.contains_key(label) {
            continue;
        }
        if text.len() <= width && *label == text.as_str() {
            short.insert(*label, text.clone());
            continue;
        }
        shortened |= text.len() > width;
        let mut name = text.chars().take(width).collect::<String>();
        let index = next.entry(name.clone()).or_insert(2);
        while !used.insert(name.clone()) {
            let suffix = format!("~{index}");
            name = text.chars().take(width - suffix.len()).collect();
            name.push_str(&suffix);
            *index += 1;
        }
        short.insert(*label, name);
    }
    (
        labels.iter().map(|label| short[label].clone()).collect(),
        shortened,
    )
}

/// Numeric target columns of a group, sorted by name.
pub(super) fn numeric_targets<'a>(
    group: &[&Spectrum<'a>],
    context: &mut Context<'_>,
) -> Vec<&'a str> {
    let mut names = BTreeSet::new();
    let mut text = false;
    for spectrum in group {
        for (name, value) in &spectrum.record.targets {
            match value {
                Value::Number(_) => {
                    names.insert(name.as_str());
                }
                Value::Null => {}
                _ => text = true,
            }
        }
    }
    if text {
        context.losses.add(
            "targets_omitted",
            "target values that are not numbers are not written",
        );
    }
    names.into_iter().collect()
}

/// Target as a 32-bit float, `None` when the record has no number for it.
///
/// WinISI files store missing constituents as 0, which the reader returns.
pub(super) fn target_f32(
    spectrum: &Spectrum<'_>,
    name: &str,
    context: &mut Context<'_>,
) -> Result<Option<f32>, WriteError> {
    let Some(Value::Number(number)) = spectrum.record.targets.get(name) else {
        return Ok(None);
    };
    let value = number.as_f64().unwrap_or(f64::NAN);
    let format = spectrum.record.provenance.format.as_str();
    if value == 0.0 && matches!(format, "foss-winisi-cal" | "foss-ds-cal") {
        return Ok(None);
    }
    let stored = value as f32;
    if !stored.is_finite() {
        return Err(refused(format!(
            "target '{name}' value {number} exceeds the 32-bit float range"
        )));
    }
    let error = (f64::from(stored) - value).abs();
    if error > 0.0 {
        context.losses.add_error(
            "target_rounded_f32",
            "target values are stored as 32-bit floats",
            Some(error),
        );
    }
    Ok(Some(stored))
}

/// Acquisition clock as stored in the record, without time-zone conversion.
#[derive(Debug, PartialEq)]
pub(super) struct Clock {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
}

impl Clock {
    pub fn of(spectrum: &Spectrum<'_>) -> Option<Self> {
        let metadata = &spectrum.record.metadata;
        ["acquired_local", "acquired_utc"]
            .iter()
            .find_map(|key| Self::parse(metadata.get(*key)?.as_str()?))
    }

    /// `YYYY-MM-DD HH:MM:SS`, with a space or a `T`.
    fn parse(text: &str) -> Option<Self> {
        let field = |range: std::ops::Range<usize>| {
            let digits = text.get(range)?;
            digits
                .bytes()
                .all(|byte| byte.is_ascii_digit())
                .then(|| digits.parse::<u16>().ok())?
        };
        let bytes = text.as_bytes();
        if bytes.len() < 19 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[13] != b':' {
            return None;
        }
        Self::from_words([
            field(8..10)?,
            field(5..7)?,
            field(0..4)?,
            field(11..13)?,
            field(14..16)?,
            field(17..19)?,
        ])
    }

    /// Day, month, year, hour, minute and second of a valid calendar date.
    pub fn from_words(words: [u16; 6]) -> Option<Self> {
        let [day, month, year, hour, minute, second] = words;
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days = match month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            _ => 0,
        };
        ((1..=9999).contains(&year)
            && (1..=days).contains(&day)
            && hour < 24
            && minute < 60
            && second < 60)
            .then_some(Self {
                year,
                month,
                day,
                hour,
                minute,
                second,
            })
    }

    pub fn words(&self) -> [u16; 6] {
        [
            self.day,
            self.month,
            self.year,
            self.hour,
            self.minute,
            self.second,
        ]
    }

    /// Packed DOS date, for the years it can hold.
    pub fn dos_date(&self) -> Option<u16> {
        (1980..=2107)
            .contains(&self.year)
            .then(|| ((self.year - 1980) << 9) | (self.month << 5) | self.day)
    }
}

/// Group label parts of a file name.
pub(super) fn grid_label(groups: usize, index: usize) -> String {
    if groups > 1 {
        format!("grid{}", index + 1)
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_parse_both_separators() {
        for text in ["2024-03-27 13:45:54", "2024-03-27T13:45:54Z"] {
            let clock = Clock::parse(text).unwrap();
            assert_eq!(clock.dos_date(), Some(0x587b));
            assert_eq!((clock.hour, clock.minute, clock.second), (13, 45, 54));
        }
        assert!(Clock::parse("2024-13-27 13:45:54").is_none());
        assert!(Clock::parse("2024-02-31 13:45:54").is_none());
        assert!(Clock::parse("+024-02-01 13:45:54").is_none());
        assert!(Clock::parse("27/03/2024 13:45:54").is_none());
    }
}
