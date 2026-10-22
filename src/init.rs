//! `little init`: starts a site in a folder, so `little serve` serves it at
//! once. It writes a `little.yml` that names the site after its folder, a
//! home page in `docs/`, unless there's one, and adds `/dist/` to the
//! folder's `.gitignore`, unless it's there, as `cargo init` adds `/target`.
//! It won't start a site where there's a `little.yml` already.

use crate::config::CONFIG_FILE;
use crate::site;
use anyhow::{Context, Result, bail};
use serde_yaml::{Mapping, Value};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

/// The folder of the site's pages: little's default `source`.
const SOURCE: &str = "docs";

/// The line that keeps the built site out of git: little's default
/// `destination`, from the folder of the `.gitignore`.
const IGNORED: &str = "/dist/";

/// The name of a site whose folder has none, like the root of a drive: the
/// built-in theme's.
const NAMELESS: &str = "Documentation";

/// The top of the `little.yml` `little init` writes, above the site's name.
const CONFIG_TOP: &str = "\
# The site's settings, over its theme's and little's own. See
# https://withlittle.github.io/little/getting-started/configuring
";

/// The rest of the `little.yml` `little init` writes, below the site's
/// name: what a site sets next, ready to uncomment.
const CONFIG_REST: &str = "\
# description: What these docs are for
# Where the site's published, and the repository it's in, on GitHub:
# url: https://<owner>.github.io/<repo>
# repo: https://github.com/<owner>/<repo>
";

/// The home page `little init` writes, unless the site has one. It has no
/// links within the site, so a new site builds with `--strict`.
const HOME_PAGE: &str = "\
# Welcome

This is the home page of your site, `docs/README.md`. Edit it, and save:
`little serve` reloads it in your browser.

Each Markdown file in `docs/` is a page, at its path without `.md`, like
`docs/guide.md` at `/guide`, and a folder's `README.md` is the folder's own
page, as on GitHub. Link to a page by its file, like `[Guide](guide.md)`.

Set the site's name, and more, in `little.yml`. `little build` builds the
site into `dist/`, for any static host.

[little's manual](https://withlittle.github.io/little) has the rest.
";

/// What `little init` did with a file, by its path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// It wrote the file, which wasn't there.
    Created(PathBuf),
    /// It added `/dist/` to the end of the file, a `.gitignore`.
    Added(PathBuf),
    /// It left the file as it was, as it was what the site needs already: a
    /// home page, or a `.gitignore` that ignores `dist/`.
    Kept(PathBuf),
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Created(path) => write!(f, "created {}", shown(path).display()),
            Action::Added(path) => write!(f, "added {IGNORED} to {}", shown(path).display()),
            Action::Kept(path) => write!(f, "kept {}", shown(path).display()),
        }
    }
}

/// A site that `little init` has started, which it prints: what it did
/// with each file, then how to serve it.
#[derive(Clone, Debug)]
pub struct Started {
    /// The site's folder, as it was given.
    pub dir: PathBuf,
    /// What it did with the site's `little.yml`, home page, and
    /// `.gitignore`, in that order.
    pub files: Vec<Action>,
}

impl fmt::Display for Started {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for file in &self.files {
            writeln!(f, "{file}")?;
        }
        match shown(&self.dir).display().to_string() {
            dir if dir.is_empty() => write!(f, "next: little serve"),
            dir if dir.contains(char::is_whitespace) => {
                write!(f, "next: cd \"{dir}\" && little serve")
            }
            dir => write!(f, "next: cd {dir} && little serve"),
        }
    }
}

/// Starts a site in `dir`, making the folder if it isn't there.
///
/// It stops, writing nothing, if `dir` has a `little.yml`, and writes its
/// own last, so a run that fails part of the way through can be run again.
pub fn init(dir: &Path) -> Result<Started> {
    let config = dir.join(CONFIG_FILE);
    if config.exists() {
        bail!(
            "{} is already there: little init won't start a site over another",
            shown(&config).display()
        );
    }
    let source = dir.join(SOURCE);
    fs::create_dir_all(&source)
        .with_context(|| format!("creating {}", shown(&source).display()))?;
    let home = home_page(&source)?;
    let ignore = ignore_destination(dir)?;
    write_new(&config, &config_for(&name_of(dir)?)?)?;
    Ok(Started {
        dir: dir.to_path_buf(),
        files: vec![Action::Created(config), home, ignore],
    })
}

/// Writes a home page in `source`, unless it has one already, as a build
/// finds it: a Markdown file called `README`, in any case. A new site has no
/// plugins, so a `README` in a format one reads isn't one yet.
fn home_page(source: &Path) -> Result<Action> {
    let reading = || format!("reading {}", shown(source).display());
    for entry in fs::read_dir(source).with_context(reading)? {
        let path = entry.with_context(reading)?.path();
        let is_page = path.extension().is_some_and(|extension| extension == "md");
        if is_page && site::is_readme(&path) && path.is_file() {
            return Ok(Action::Kept(path));
        }
    }
    let path = source.join("README.md");
    write_new(&path, HOME_PAGE)?;
    Ok(Action::Created(path))
}

/// Adds `/dist/` to the `.gitignore` in `dir`, writing one if it isn't
/// there, unless it ignores `dist/` already.
fn ignore_destination(dir: &Path) -> Result<Action> {
    let path = dir.join(".gitignore");
    let ignored = match fs::read(&path) {
        Ok(ignored) => ignored,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            write_new(&path, &format!("{IGNORED}\n"))?;
            return Ok(Action::Created(path));
        }
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", shown(&path).display()));
        }
    };
    let ignores_dist = String::from_utf8_lossy(&ignored)
        .lines()
        .any(|line| matches!(line.trim(), "dist" | "dist/" | "/dist" | "/dist/"));
    if ignores_dist {
        return Ok(Action::Kept(path));
    }
    let newline = match ignored.last() {
        None | Some(b'\n') => "",
        Some(_) => "\n",
    };
    OpenOptions::new()
        .append(true)
        .open(&path)
        .and_then(|mut file| writeln!(file, "{newline}{IGNORED}"))
        .with_context(|| format!("writing {}", shown(&path).display()))?;
    Ok(Action::Added(path))
}

/// The name of the site in `dir`, which exists: its folder's, as it's
/// called, even when `dir` is `.`, else the built-in theme's.
fn name_of(dir: &Path) -> Result<String> {
    let dir = fs::canonicalize(dir).with_context(|| format!("reading {}", dir.display()))?;
    Ok(dir
        .file_name()
        .map_or(NAMELESS.into(), |name| name.to_string_lossy().into()))
}

/// The `little.yml` of a site called `name`, with the name written as YAML,
/// so one like `yes` or `#1` is read back as it is.
fn config_for(name: &str) -> Result<String> {
    let mut values = Mapping::new();
    values.insert("name".into(), Value::from(name));
    let name = serde_yaml::to_string(&values).context("writing the site's name")?;
    Ok(format!("{CONFIG_TOP}{name}{CONFIG_REST}"))
}

/// Writes `contents` to a new file at `path`, which mustn't be there.
fn write_new(path: &Path, contents: &str) -> Result<()> {
    fs::File::create_new(path)
        .and_then(|mut file| file.write_all(contents.as_bytes()))
        .with_context(|| format!("writing {}", shown(path).display()))
}

/// `path` as it's shown, without the `./` that `-C`'s default puts before
/// it.
fn shown(path: &Path) -> &Path {
    path.strip_prefix(".").unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;

    /// An empty folder for a test, named for it.
    fn scratch(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("little-init-test-{}-{label}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    /// The site's name, as little reads it.
    fn name(dir: &Path) -> Value {
        config::load(dir).unwrap().values["name"].clone()
    }

    #[test]
    fn starts_a_site_that_builds() {
        let root = scratch("builds");
        let dir = root.join("nested").join("my-docs");

        let started = init(&dir).unwrap();

        let home = dir.join("docs").join("README.md");
        assert_eq!(
            started.files,
            [
                Action::Created(dir.join("little.yml")),
                Action::Created(home.clone()),
                Action::Created(dir.join(".gitignore")),
            ]
        );
        assert_eq!(fs::read_to_string(&home).unwrap(), HOME_PAGE);
        assert_eq!(
            fs::read_to_string(dir.join(".gitignore")).unwrap(),
            "/dist/\n"
        );
        assert_eq!(name(&dir), Value::from("my-docs"));
        site::build(&dir).unwrap();
        let built = fs::read_to_string(dir.join("dist").join("index.html")).unwrap();
        assert!(
            built.contains("<title>Welcome · my-docs</title>"),
            "{built}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn wont_start_a_site_over_another() {
        let dir = scratch("over");
        fs::write(dir.join("little.yml"), "name: Mine\n").unwrap();

        let error = init(&dir).unwrap_err().to_string();

        assert!(
            error.ends_with(
                "little.yml is already there: little init won't start a site over another"
            ),
            "{error}"
        );
        assert_eq!(
            fs::read_to_string(dir.join("little.yml")).unwrap(),
            "name: Mine\n"
        );
        // It wrote nothing else.
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn keeps_a_home_page_in_any_case() {
        let dir = scratch("home");
        let docs = dir.join("docs");
        fs::create_dir_all(&docs).unwrap();
        fs::write(docs.join("readme.md"), "# Mine\n").unwrap();
        // Not a page, so not a home page.
        fs::write(docs.join("README.txt"), "Mine\n").unwrap();

        let started = init(&dir).unwrap();

        assert_eq!(started.files[1], Action::Kept(docs.join("readme.md")));
        assert_eq!(
            fs::read_to_string(docs.join("readme.md")).unwrap(),
            "# Mine\n"
        );
        // No README.md beside it, to be the same page.
        let mut names: Vec<_> = fs::read_dir(&docs)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names, ["README.txt", "readme.md"]);
        site::build(&dir).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_notebook_isnt_a_home_page_without_a_plugin() {
        let dir = scratch("notebook");
        let docs = dir.join("docs");
        fs::create_dir_all(&docs).unwrap();
        let notebook = r#"{"cells": [], "metadata": {}, "nbformat": 4, "nbformat_minor": 5}"#;
        fs::write(docs.join("README.ipynb"), notebook).unwrap();

        let started = init(&dir).unwrap();

        // A new site has no plugin to read it.
        let home = docs.join("README.md");
        assert_eq!(started.files[1], Action::Created(home.clone()));
        assert_eq!(fs::read_to_string(&home).unwrap(), HOME_PAGE);
        // So it's copied, as any other file is.
        site::build(&dir).unwrap();
        let copied = dir.join("dist").join("README.ipynb");
        assert_eq!(fs::read_to_string(copied).unwrap(), notebook);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn adds_dist_to_a_gitignore_once() {
        for (before, after, added) in [
            ("", "/dist/\n", true),
            ("target/\n", "target/\n/dist/\n", true),
            ("target/", "target/\n/dist/\n", true),
            ("node_modules/\ndist/\n", "node_modules/\ndist/\n", false),
            ("# built\n  /dist \r\n", "# built\n  /dist \r\n", false),
            ("!dist/\n", "!dist/\n/dist/\n", true),
        ] {
            let dir = scratch("gitignore");
            let path = dir.join(".gitignore");
            fs::write(&path, before).unwrap();

            let started = init(&dir).unwrap();

            let action = if added {
                Action::Added(path.clone())
            } else {
                Action::Kept(path.clone())
            };
            assert_eq!(started.files[2], action, "{before:?}");
            assert_eq!(fs::read_to_string(&path).unwrap(), after, "{before:?}");
            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn names_a_site_after_its_folder_as_it_is() {
        let root = scratch("names");
        // Each would be read as something other than its name, unquoted.
        for folder in ["yes", "123", "null", "~", "#1", "- x", "[x]", "it's"] {
            let dir = root.join(folder);

            init(&dir).unwrap();

            assert_eq!(name(&dir), Value::from(folder), "{folder}");
        }
        // Started in the folder it's run in, it takes that folder's name.
        let dir = root.join("here");
        fs::create_dir_all(&dir).unwrap();
        init(&dir.join(".")).unwrap();
        assert_eq!(name(&dir), Value::from("here"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn says_what_it_did_then_how_to_serve_the_site() {
        let dir = Path::new(".").join("my docs");
        let started = Started {
            dir: dir.clone(),
            files: vec![
                Action::Created(dir.join("little.yml")),
                Action::Kept(dir.join("docs").join("README.md")),
                Action::Added(dir.join(".gitignore")),
            ],
        };
        let shown = Path::new("my docs");
        assert_eq!(
            started.to_string(),
            format!(
                "created {}\nkept {}\nadded /dist/ to {}\nnext: cd \"my docs\" && little serve",
                shown.join("little.yml").display(),
                shown.join("docs").join("README.md").display(),
                shown.join(".gitignore").display(),
            )
        );

        let here = Started {
            dir: PathBuf::from("."),
            files: vec![Action::Created(Path::new(".").join("little.yml"))],
        };
        assert_eq!(here.to_string(), "created little.yml\nnext: little serve");

        let there = Started {
            dir: PathBuf::from("/srv/my-docs"),
            files: vec![],
        };
        assert_eq!(there.to_string(), "next: cd /srv/my-docs && little serve");
    }
}
