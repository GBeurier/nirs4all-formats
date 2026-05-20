use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};

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
        #[arg(long)]
        rows: Option<String>,
        /// Optional half-open cube column window, for example `30:40` or `30:`.
        #[arg(long)]
        cols: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Probe { path } => {
            let probes = nirs4all_io::probe_path(&path)
                .with_context(|| format!("failed to probe {}", path.display()))?;
            println!("{}", serde_json::to_string_pretty(&probes)?);
        }
        Command::ReadJson { path, rows, cols } => {
            let options = read_options(rows.as_deref(), cols.as_deref())?;
            let records = nirs4all_io::open_path_with_options(&path, &options)
                .with_context(|| format!("failed to read {}", path.display()))?;
            println!("{}", serde_json::to_string_pretty(&records)?);
        }
    }
    Ok(())
}

fn read_options(
    rows: Option<&str>,
    cols: Option<&str>,
) -> anyhow::Result<nirs4all_io::ReadOptions> {
    if rows.is_none() && cols.is_none() {
        return Ok(nirs4all_io::ReadOptions::default());
    }
    let (row_start, row_end) = parse_window_range(rows.unwrap_or("0:"), "--rows")?;
    let (col_start, col_end) = parse_window_range(cols.unwrap_or("0:"), "--cols")?;
    Ok(
        nirs4all_io::ReadOptions::default().with_cube_window(nirs4all_io::CubeWindow::new(
            row_start, row_end, col_start, col_end,
        )),
    )
}

fn parse_window_range(value: &str, label: &str) -> anyhow::Result<(usize, Option<usize>)> {
    let (start, end) = value
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("{label} must use START:END syntax"))?;
    let start = start
        .parse::<usize>()
        .with_context(|| format!("{label} start is not an unsigned integer: {start}"))?;
    let end = if end.trim().is_empty() {
        None
    } else {
        Some(
            end.parse::<usize>()
                .with_context(|| format!("{label} end is not an unsigned integer: {end}"))?,
        )
    };
    Ok((start, end))
}
