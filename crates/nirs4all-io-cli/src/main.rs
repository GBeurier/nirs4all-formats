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
        Command::ReadJson { path } => {
            let records = nirs4all_io::open_path(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            println!("{}", serde_json::to_string_pretty(&records)?);
        }
    }
    Ok(())
}
