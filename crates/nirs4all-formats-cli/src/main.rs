use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand};
use nirs4all_formats::{
    walk_path, InMemorySidecars, SidecarResolver, WalkOptions, WalkOutcome, WalkStats,
};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Probe a file and print candidate readers as JSON.
    Probe {
        /// Input file.
        path: PathBuf,
    },
    /// Read a file and print normalized spectral records as JSON.
    ReadJson {
        /// Input file.
        path: PathBuf,
        /// Optional half-open cube row window, for example `10:20` or `10:`.
        #[arg(long, conflicts_with_all = ["pixel", "pixels_file"])]
        rows: Option<String>,
        /// Optional half-open cube column window, for example `30:40` or `30:`.
        #[arg(long, conflicts_with_all = ["pixel", "pixels_file"])]
        cols: Option<String>,
        /// Optional sparse pixel mask. Repeat the flag once per `ROW,COL` pair.
        #[arg(long = "pixel", value_name = "ROW,COL")]
        pixel: Vec<String>,
        /// Optional path to a sparse pixel mask file. Each non-empty,
        /// non-comment line must contain a single `ROW,COL` pair.
        #[arg(long = "pixels-file", value_name = "PATH")]
        pixels_file: Option<PathBuf>,
        /// Emit an image cube as a single N-dimensional record
        /// (`dims = ["row", "col", "x"]`) instead of one record per pixel.
        /// Works with `--rows`/`--cols` (sub-cube) but not with a sparse
        /// `--pixel`/`--pixels-file` mask.
        #[arg(long = "single-record", conflicts_with_all = ["pixel", "pixels_file"])]
        single_record: bool,
        /// Provide a sidecar file as `KEY=PATH`. `KEY` is the relative
        /// name the reader looks up (for example `cubescope-mini-cube.hdr`
        /// next to an ENVI Standard cube). Repeat the flag once per sidecar.
        #[arg(long = "sidecar", value_name = "KEY=PATH")]
        sidecar: Vec<String>,
        /// Read the primary payload from `--bytes-file PATH` instead of
        /// from `PATH`, exercising the in-memory `open_with_sidecars`
        /// entry point. Requires at least one `--sidecar` if the format
        /// needs companions.
        #[arg(long = "bytes-file", value_name = "PATH")]
        bytes_file: Option<PathBuf>,
    },
    /// Recursively scan a directory and report which files load successfully.
    Scan {
        /// Directory or file to walk.
        path: PathBuf,
        /// Limit recursion depth (default: unlimited).
        #[arg(long)]
        max_depth: Option<usize>,
        /// Include hidden entries (dot-prefixed names).
        #[arg(long)]
        include_hidden: bool,
        /// Follow filesystem symlinks (loops are not detected).
        #[arg(long)]
        follow_symlinks: bool,
        /// Include files with no matching reader in the output.
        #[arg(long)]
        include_unsupported: bool,
        /// Emit JSON instead of human-readable lines.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Probe { path } => {
            let probes = nirs4all_formats::probe_path(&path)
                .with_context(|| format!("failed to probe {}", path.display()))?;
            println!("{}", serde_json::to_string_pretty(&probes)?);
        }
        Command::ReadJson {
            path,
            rows,
            cols,
            pixel,
            pixels_file,
            single_record,
            sidecar,
            bytes_file,
        } => {
            let mut options = read_options(
                rows.as_deref(),
                cols.as_deref(),
                &pixel,
                pixels_file.as_deref(),
            )?;
            if single_record {
                options = options.single_record();
            }
            let records = if sidecar.is_empty() && bytes_file.is_none() {
                nirs4all_formats::open_path_with_options(&path, &options)
                    .with_context(|| format!("failed to read {}", path.display()))?
            } else {
                let primary_path = bytes_file.as_deref().unwrap_or(&path);
                let bytes = std::fs::read(primary_path).with_context(|| {
                    format!(
                        "failed to read primary bytes from {}",
                        primary_path.display()
                    )
                })?;
                let mut resolver = InMemorySidecars::new();
                for raw in &sidecar {
                    let (key, value) = raw.split_once('=').ok_or_else(|| {
                        anyhow::anyhow!("--sidecar must use KEY=PATH syntax: {raw}")
                    })?;
                    let sidecar_bytes = std::fs::read(value)
                        .with_context(|| format!("failed to read sidecar {value}"))?;
                    resolver.insert(PathBuf::from(key), sidecar_bytes);
                }
                let arc: Arc<dyn SidecarResolver> = Arc::new(resolver);
                nirs4all_formats::open_with_sidecars_and_options(&path, &bytes, arc, &options)
                    .with_context(|| format!("failed to decode {}", path.display()))?
            };
            println!("{}", serde_json::to_string_pretty(&records)?);
        }
        Command::Scan {
            path,
            max_depth,
            include_hidden,
            follow_symlinks,
            include_unsupported,
            json,
        } => {
            let options = WalkOptions {
                max_depth,
                skip_hidden: !include_hidden,
                follow_symlinks,
                skip_unsupported: !include_unsupported,
                read_options: nirs4all_formats::ReadOptions::default(),
            };
            let entries = walk_path(&path, &options)
                .with_context(|| format!("failed to scan {}", path.display()))?;
            if json {
                emit_scan_json(&entries)?;
            } else {
                emit_scan_text(&entries);
            }
        }
    }
    Ok(())
}

fn emit_scan_text(entries: &[nirs4all_formats::WalkEntry]) {
    for entry in entries {
        match &entry.outcome {
            WalkOutcome::Parsed { format, records } => {
                println!(
                    "{}\tparsed\t{format}\t{} record(s)",
                    entry.path.display(),
                    records.len()
                );
            }
            WalkOutcome::Error {
                candidate_format,
                message,
            } => {
                let fmt = candidate_format.as_deref().unwrap_or("-");
                println!(
                    "{}\terror\t{fmt}\t{}",
                    entry.path.display(),
                    message.replace(['\t', '\n'], " ")
                );
            }
            WalkOutcome::Unsupported => {
                println!("{}\tunsupported\t-\t-", entry.path.display());
            }
        }
    }
    let stats = WalkStats::collect(entries);
    eprintln!(
        "scan summary: {} parsed, {} errored, {} unsupported, {} total",
        stats.parsed,
        stats.errored,
        stats.unsupported,
        stats.total()
    );
}

fn emit_scan_json(entries: &[nirs4all_formats::WalkEntry]) -> anyhow::Result<()> {
    let payload: Vec<_> = entries
        .iter()
        .map(|entry| match &entry.outcome {
            WalkOutcome::Parsed { format, records } => json!({
                "path": entry.path,
                "status": "parsed",
                "format": format,
                "records": records.len(),
            }),
            WalkOutcome::Error {
                candidate_format,
                message,
            } => json!({
                "path": entry.path,
                "status": "error",
                "candidate_format": candidate_format,
                "message": message,
            }),
            WalkOutcome::Unsupported => json!({
                "path": entry.path,
                "status": "unsupported",
            }),
        })
        .collect();
    let stats = WalkStats::collect(entries);
    let summary = json!({
        "entries": payload,
        "summary": {
            "parsed": stats.parsed,
            "errored": stats.errored,
            "unsupported": stats.unsupported,
            "total": stats.total(),
        },
    });
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn read_options(
    rows: Option<&str>,
    cols: Option<&str>,
    pixel: &[String],
    pixels_file: Option<&std::path::Path>,
) -> anyhow::Result<nirs4all_formats::ReadOptions> {
    let has_mask = !pixel.is_empty() || pixels_file.is_some();
    let has_window = rows.is_some() || cols.is_some();
    if has_window && has_mask {
        anyhow::bail!("--rows/--cols cannot be combined with --pixel/--pixels-file");
    }
    if has_mask {
        let mut pixels = Vec::new();
        for raw in pixel {
            pixels.push(parse_pixel(raw, "--pixel")?);
        }
        if let Some(path) = pixels_file {
            let contents = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read --pixels-file {}", path.display()))?;
            for (line_no, raw) in contents.lines().enumerate() {
                let trimmed = raw.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                pixels.push(parse_pixel(
                    trimmed,
                    &format!("{}:{}", path.display(), line_no + 1),
                )?);
            }
        }
        if pixels.is_empty() {
            anyhow::bail!("--pixel/--pixels-file did not provide any pixels");
        }
        return Ok(nirs4all_formats::ReadOptions::default()
            .with_cube_mask(nirs4all_formats::CubeMask::new(pixels)));
    }
    if !has_window {
        return Ok(nirs4all_formats::ReadOptions::default());
    }
    let (row_start, row_end) = parse_window_range(rows.unwrap_or("0:"), "--rows")?;
    let (col_start, col_end) = parse_window_range(cols.unwrap_or("0:"), "--cols")?;
    Ok(nirs4all_formats::ReadOptions::default().with_cube_window(
        nirs4all_formats::CubeWindow::new(row_start, row_end, col_start, col_end),
    ))
}

fn parse_window_range(value: &str, label: &str) -> anyhow::Result<(usize, Option<usize>)> {
    let (start, end) = value
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("{label} must use START:END syntax"))?;
    let start = start.trim();
    let end = end.trim();
    let start = start
        .parse::<usize>()
        .with_context(|| format!("{label} start is not an unsigned integer: {start}"))?;
    let end = if end.is_empty() {
        None
    } else {
        Some(
            end.parse::<usize>()
                .with_context(|| format!("{label} end is not an unsigned integer: {end}"))?,
        )
    };
    Ok((start, end))
}

fn parse_pixel(value: &str, label: &str) -> anyhow::Result<(usize, usize)> {
    let (row, col) = value
        .split_once(',')
        .ok_or_else(|| anyhow::anyhow!("{label} must use ROW,COL syntax: {value}"))?;
    let row = row
        .trim()
        .parse::<usize>()
        .with_context(|| format!("{label} row is not an unsigned integer: {row}"))?;
    let col = col
        .trim()
        .parse::<usize>()
        .with_context(|| format!("{label} column is not an unsigned integer: {col}"))?;
    Ok((row, col))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_window_range_trims_bounds() {
        assert_eq!(
            parse_window_range(" 10 : 20 ", "--rows").unwrap(),
            (10, Some(20))
        );
        assert_eq!(parse_window_range("5: ", "--rows").unwrap(), (5, None));
    }

    #[test]
    fn read_options_rejects_mixed_window_and_mask() {
        let pixels = vec!["1,2".to_string()];
        let err = read_options(Some("0:3"), None, &pixels, None).unwrap_err();
        assert!(err.to_string().contains("--rows/--cols cannot be combined"));
    }

    #[test]
    fn read_options_rejects_empty_mask() {
        let path = std::env::temp_dir().join(format!(
            "nirs4all-formats-empty-pixels-{}.txt",
            std::process::id()
        ));
        std::fs::write(&path, b"\n# only comments\n").unwrap();
        let err = read_options(None, None, &[], Some(&path)).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(err.to_string().contains("did not provide any pixels"));
    }
}
