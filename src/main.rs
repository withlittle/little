mod config;
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
        /// Address to bind
        #[arg(long, default_value = "127.0.0.1:8080")]
        addr: String,
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
        Commands::Serve { addr } => server::serve(&cli.dir, &addr)?,
        Commands::Config => {
            let cfg = config::load(&cli.dir)?;
            print!("{}", serde_yaml::to_string(&cfg.values)?);
        }
    }
    Ok(())
}
