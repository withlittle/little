mod config;
mod download;
mod render;
mod server;
mod site;
mod theme;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "little",
    version,
    about = "A little static documentation site generator"
)]
struct Cli {
    /// Site directory (defaults to the current directory)
    #[arg(short = 'C', long, default_value = ".", global = true)]
    dir: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the site into its destination folder (dist/ by default)
    Build,
    /// Serve the site locally with live reload
    Serve {
        /// Host to serve at, overriding the config's `host`
        #[arg(long)]
        host: Option<String>,
        /// Port to serve at, overriding the config's `port`
        #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
        port: Option<u16>,
    },
    /// Print the site's config: little's, its theme's, and its own, merged
    Config,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Build => {
            site::build(&cli.dir)?;
        }
        Commands::Serve { host, port } => server::serve(&cli.dir, host, port)?,
        Commands::Config => {
            let cfg = config::load(&cli.dir)?;
            print!("{}", serde_yaml::to_string(&cfg.values)?);
        }
    }
    Ok(())
}
