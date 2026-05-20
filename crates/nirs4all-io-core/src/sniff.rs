use serde::{Deserialize, Serialize};

/// Reader confidence returned by magic-byte sniffers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    No,
    Possible,
    Likely,
    Definite,
}

/// Lightweight result of format probing before full parsing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormatProbe {
    pub format: String,
    pub reader: String,
    pub confidence: Confidence,
    pub reason: String,
}

impl FormatProbe {
    pub fn new(
        format: impl Into<String>,
        reader: impl Into<String>,
        confidence: Confidence,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            format: format.into(),
            reader: reader.into(),
            confidence,
            reason: reason.into(),
        }
    }
}
