//! little, a static documentation site generator: the `little` command.
//! `little build` builds a site, its Markdown and notebooks, with its theme,
//! into HTML, as [`site`] does; `little serve` serves it, building it again
//! as it changes, with [`server`]; and `little config` prints its
//! [`config`].
//!
//! It's a command, not a library, so its modules are for each other, and
//! for those who work on little, not for other crates.

mod config;
mod download;
mod git;
mod highlight;
mod links;
mod math;
mod mermaid;
mod notebook;
mod plugin;
mod render;
mod sass;
mod search;
mod server;
mod site;
mod theme;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

// musl's own allocator is slow when many threads allocate at once, as a
// build's do, so little's Linux binaries use jemalloc, as Cargo.toml says.
#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

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
    /// warn of each formula it can't render, each diagram it can't draw,
    /// each code block a plugin can't render, each warning from a plugin,
    /// and each broken link
    Build {
        /// Fail on a formula that can't be rendered, a diagram that can't be
        /// drawn, a code block a plugin can't render, a warning from a
        /// plugin, or a broken link, rather than warn of it, as for CI
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
            for warning in &built.plugin_warnings {
                eprintln!("warning: {warning}");
            }
            for formula in &built.unrendered_math {
                eprintln!("warning: {formula}");
            }
            for diagram in &built.unrendered_diagrams {
                eprintln!("warning: {diagram}");
            }
            for block in &built.unrendered_blocks {
                eprintln!("warning: {block}");
            }
            let broken = links::check(&built, links::Scope::All)?;
            for link in &broken {
                eprintln!("warning: {link}");
            }
            if strict {
                let problems: Vec<_> = [
                    (built.plugin_warnings.len(), "warning(s) from plugins"),
                    (
                        built.unrendered_math.len(),
                        "formula(s) that can't be rendered",
                    ),
                    (
                        built.unrendered_diagrams.len(),
                        "diagram(s) that can't be drawn",
                    ),
                    (
                        built.unrendered_blocks.len(),
                        "code block(s) that plugins can't render",
                    ),
                    (broken.len(), "broken link(s)"),
                ]
                .into_iter()
                .filter(|&(count, _)| count > 0)
                .map(|(count, problem)| format!("{count} {problem}"))
                .collect();
                match problems.as_slice() {
                    [] => {}
                    [problem] => bail!("{problem}"),
                    [first, last] => bail!("{first} and {last}"),
                    [rest @ .., last] => bail!("{}, and {last}", rest.join(", ")),
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
