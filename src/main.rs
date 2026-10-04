mod config;
mod download;
mod highlight;
mod links;
mod math;
mod notebook;
mod render;
mod sass;
mod search;
mod server;
mod site;
mod theme;

use anyhow::{Result, bail};
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
    /// Build the site into its destination folder (dist/ by default), and
    /// warn of each formula it can't render, and each broken link
    Build {
        /// Fail on a formula that can't be rendered, or a broken link, rather
        /// than warn of it, as for CI
        #[arg(long)]
        strict: bool,
    },
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
        Commands::Build { strict } => {
            let built = site::build(&cli.dir)?;
            for formula in &built.unrendered_math {
                eprintln!("warning: {formula}");
            }
            let broken = links::check(&built, links::Scope::All)?;
            for link in &broken {
                eprintln!("warning: {link}");
            }
            if strict {
                let problems: Vec<_> = [
                    (
                        built.unrendered_math.len(),
                        "formula(s) that can't be rendered",
                    ),
                    (broken.len(), "broken link(s)"),
                ]
                .into_iter()
                .filter(|&(count, _)| count > 0)
                .map(|(count, problem)| format!("{count} {problem}"))
                .collect();
                if !problems.is_empty() {
                    bail!("{}", problems.join(" and "));
                }
            }
        }
        Commands::Serve { host, port } => server::serve(&cli.dir, host, port)?,
        Commands::Config => {
            let cfg = config::load(&cli.dir)?;
            print!("{}", serde_yaml::to_string(&cfg.values)?);
        }
    }
    Ok(())
}
