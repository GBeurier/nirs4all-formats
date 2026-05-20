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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Probe { path } => {
            let probes = nirs4all_io::probe_path(&path)
                .with_context(|| format!("failed to probe {}", path.display()))?;
            println!("{}", serde_json::to_string_pretty(&probes)?);
        }
    }
    Ok(())
}
