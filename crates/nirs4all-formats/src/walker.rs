//! Auto-discovery walker over directories of spectroscopy files.
//!
//! The walker probes each file with the registry's signature/extension sniffers
//! and reads any file with at least one positive candidate, returning a flat
//! list of per-path outcomes (`parsed`, `error`, or `unsupported`).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use nirs4all_formats_core::{Error, Result, SpectralRecord};

use crate::registry::{builtin_probes, open_path_with_options, ReadOptions};

const PROBE_HEAD_BYTES: usize = 8192;

/// Walk a single file or a directory tree and decode every supported file.
#[derive(Clone, Debug)]
pub struct WalkOptions {
    /// Maximum recursion depth. `None` = unlimited.
    pub max_depth: Option<usize>,
    /// Skip entries whose file name begins with a dot.
    pub skip_hidden: bool,
    /// Follow filesystem symlinks. Off by default to avoid loops.
    pub follow_symlinks: bool,
    /// Omit `Unsupported` outcomes from the output list.
    pub skip_unsupported: bool,
    /// Forwarded to per-file `open_path_with_options` for cube readers etc.
    pub read_options: ReadOptions,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            max_depth: None,
            skip_hidden: true,
            follow_symlinks: false,
            skip_unsupported: true,
            read_options: ReadOptions::default(),
        }
    }
}

/// One walk outcome.
#[derive(Debug)]
pub struct WalkEntry {
    pub path: PathBuf,
    pub outcome: WalkOutcome,
}

/// What happened for one file.
#[derive(Debug)]
pub enum WalkOutcome {
    /// Reader accepted the file and produced records. `format` is the matched probe.
    Parsed {
        format: String,
        records: Vec<SpectralRecord>,
    },
    /// Reader recognized the file but refused or failed.
    Error {
        candidate_format: Option<String>,
        message: String,
    },
    /// No reader claimed the file (head sniff produced no candidates).
    Unsupported,
}

impl WalkOutcome {
    pub fn is_parsed(&self) -> bool {
        matches!(self, Self::Parsed { .. })
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    pub fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported)
    }
}

/// Walk a path and return one entry per visited file.
pub fn walk_path(root: impl AsRef<Path>, options: &WalkOptions) -> Result<Vec<WalkEntry>> {
    let root = root.as_ref();
    let mut entries = Vec::new();
    let meta = match fs::symlink_metadata(root) {
        Ok(meta) => meta,
        Err(source) => {
            return Err(Error::Io {
                path: root.to_path_buf(),
                source,
            })
        }
    };
    if meta.file_type().is_symlink() && !options.follow_symlinks {
        return Ok(entries);
    }
    if meta.is_file() {
        visit_file(root, options, &mut entries);
    } else if meta.is_dir() {
        visit_dir(root, 0, options, &mut entries)?;
    }
    Ok(entries)
}

fn visit_dir(
    dir: &Path,
    depth: usize,
    options: &WalkOptions,
    entries: &mut Vec<WalkEntry>,
) -> Result<()> {
    if let Some(max) = options.max_depth {
        if depth > max {
            return Ok(());
        }
    }
    let read_dir = fs::read_dir(dir).map_err(|source| Error::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    let mut children: Vec<PathBuf> = read_dir
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .collect();
    children.sort();

    for child in children {
        if options.skip_hidden && is_hidden(&child) {
            continue;
        }
        let meta = match fs::symlink_metadata(&child) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() && !options.follow_symlinks {
            continue;
        }
        if meta.is_dir() {
            visit_dir(&child, depth + 1, options, entries)?;
        } else if meta.is_file() {
            visit_file(&child, options, entries);
        }
    }
    Ok(())
}

fn visit_file(path: &Path, options: &WalkOptions, entries: &mut Vec<WalkEntry>) {
    let head = match read_head(path, PROBE_HEAD_BYTES) {
        Ok(head) => head,
        Err(message) => {
            entries.push(WalkEntry {
                path: path.to_path_buf(),
                outcome: WalkOutcome::Error {
                    candidate_format: None,
                    message,
                },
            });
            return;
        }
    };
    let probes = builtin_probes(&head, path);
    if probes.is_empty() {
        if !options.skip_unsupported {
            entries.push(WalkEntry {
                path: path.to_path_buf(),
                outcome: WalkOutcome::Unsupported,
            });
        }
        return;
    }
    let best_format = probes.first().map(|probe| probe.format.clone());
    let outcome = match open_path_with_options(path, &options.read_options) {
        Ok(records) => {
            let format = best_format
                .clone()
                .or_else(|| records.first().map(|r| r.provenance.format.clone()))
                .unwrap_or_else(|| "unknown".to_string());
            WalkOutcome::Parsed { format, records }
        }
        Err(err) => WalkOutcome::Error {
            candidate_format: best_format,
            message: err.to_string(),
        },
    };
    entries.push(WalkEntry {
        path: path.to_path_buf(),
        outcome,
    });
}

fn read_head(path: &Path, limit: usize) -> std::result::Result<Vec<u8>, String> {
    let mut file = fs::File::open(path).map_err(|err| err.to_string())?;
    let mut buf = vec![0u8; limit];
    let mut total = 0;
    while total < limit {
        let n = file
            .read(&mut buf[total..])
            .map_err(|err| err.to_string())?;
        if n == 0 {
            break;
        }
        total += n;
    }
    buf.truncate(total);
    Ok(buf)
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
}

/// Aggregate counters for human/JSON summaries.
#[derive(Clone, Copy, Debug, Default)]
pub struct WalkStats {
    pub parsed: usize,
    pub errored: usize,
    pub unsupported: usize,
}

impl WalkStats {
    pub fn collect(entries: &[WalkEntry]) -> Self {
        let mut stats = Self::default();
        for entry in entries {
            match &entry.outcome {
                WalkOutcome::Parsed { .. } => stats.parsed += 1,
                WalkOutcome::Error { .. } => stats.errored += 1,
                WalkOutcome::Unsupported => stats.unsupported += 1,
            }
        }
        stats
    }

    pub fn total(&self) -> usize {
        self.parsed + self.errored + self.unsupported
    }
}
