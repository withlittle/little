//! Helpers that the tests of little's modules share.

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use time::OffsetDateTime;
use walkdir::WalkDir;

use crate::config::{Markdown, SiteUrl};
use crate::plugin::PagePlugins;
use crate::render::{Rendered, parse_document};

/// A fresh scratch folder holding `files`, each a (path, contents) pair.
/// It's deleted when the [`TempDir`] is dropped, so a test keeps it, as
/// `_tmp`, not `_`, while it uses the folder.
pub(crate) fn scratch(files: &[(&str, &str)]) -> (TempDir, PathBuf) {
    // tempfile's own names start with a `.`, which hides a file from little.
    let dir = tempfile::Builder::new()
        .prefix("little-")
        .tempdir()
        .unwrap();
    let root = dir.path().to_path_buf();
    write(&root, files);
    (dir, root)
}

/// Write `files`, each a (path, contents) pair, in `root`, with the folders
/// they're in.
pub(crate) fn write(root: &Path, files: &[(&str, &str)]) {
    for (file, contents) in files {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
}

/// The files under `dir`, from it, with each `/`.
pub(crate) fn files_in(dir: &Path) -> Vec<String> {
    let mut files: Vec<String> = WalkDir::new(dir)
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            let path = entry.path().strip_prefix(dir).unwrap();
            let names: Vec<_> = path.iter().map(|name| name.to_string_lossy()).collect();
            names.join("/")
        })
        .collect();
    files.sort();
    files
}

/// Render `md`, a page in the folder at `folder`, like `/guide/`, of a site
/// at `url`, with `markdown`, and no plugins. Its table of contents has
/// little's levels, every heading but the title. It's built at the start of
/// 2026, for its diagrams.
pub(crate) fn render_at(
    md: &str,
    folder: &str,
    url: &SiteUrl,
    markdown: &Markdown,
) -> Result<Rendered> {
    let now = OffsetDateTime::from_unix_timestamp(1_767_225_600).unwrap();
    parse_document(
        md,
        folder,
        &(2..=6),
        url,
        markdown,
        now,
        &mut PagePlugins::none(),
    )
}

/// Where little's manual is published: under a path, `/little`.
pub(crate) fn site_url() -> SiteUrl {
    SiteUrl {
        origin: "https://withlittle.github.io".into(),
        path: "/little".into(),
        shared: None,
    }
}
