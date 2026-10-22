//! little, a static documentation site generator: the `little` command.
//! `little init` starts a site, as [`init`] does; `little build` builds a
//! site, its Markdown and notebooks, with its theme, into HTML, as [`site`]
//! does; `little serve` serves it, building it again as it changes, with
//! [`server`]; and `little config` prints its [`config`].
//!
//! It's a command, not a library, so its modules are for each other, and
//! for those who work on little, not for other crates.

mod config;
mod download;
mod git;
mod highlight;
mod init;
mod links;
mod math;
mod mermaid;
mod notebook;
mod parts;
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
    /// Start a site: a little.yml, named after its folder, a home page in
    /// docs/, and /dist/ in its .gitignore
    Init {
        /// Folder to start the site in, made if it isn't there (defaults to
        /// the current directory)
        path: Option<PathBuf>,
    },
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
        Commands::Init { path } => {
            // PATH is from the folder -C names, as git -C's paths are.
            let dir = match path {
                Some(path) => cli.dir.join(path),
                None => cli.dir,
            };
            println!("{}", init::init(&dir)?);
        }
        Commands::Build { strict } => {
            let built = site::build(&cli.dir)?;
            for warning in &built.warnings {
                eprintln!("warning: {}", warning.message);
            }
            let broken = links::check(&built, links::Scope::All)?;
            for link in &broken {
                eprintln!("warning: {link}");
            }
            if strict {
                // Each kind, counted, in the order it's first warned of.
                let mut counts: Vec<(&str, usize)> = Vec::new();
                let kinds = built.warnings.iter().map(|warning| warning.kind);
                for kind in kinds.chain(broken.iter().map(|_| "broken link(s)")) {
                    match counts.iter_mut().find(|(seen, _)| *seen == kind) {
                        Some((_, count)) => *count += 1,
                        None => counts.push((kind, 1)),
                    }
                }
                let problems: Vec<_> = (counts.iter())
                    .map(|(kind, count)| format!("{count} {kind}"))
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
