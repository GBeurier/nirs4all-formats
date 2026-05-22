use std::path::PathBuf;

/// Error type shared by readers, validators, and bindings.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported format for {path}")]
    UnsupportedFormat { path: PathBuf },

    #[error("ambiguous format for {path}: {candidates:?}")]
    AmbiguousFormat {
        path: PathBuf,
        candidates: Vec<String>,
    },

    #[error("invalid spectral record: {0}")]
    InvalidRecord(String),

    #[error("I/O error while reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("sidecar '{path}' is not available: {reason}")]
    UnsupportedSidecar { path: PathBuf, reason: String },

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
