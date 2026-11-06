//! Helpers for the plugins' tests, behind the `testing` feature, which each
//! plugin turns on for its tests alone, so none is in its `.wasm` file.

use crate::pages::Added;

/// A folder for the test called `test`, holding `files`, each its path in
/// the folder and what it holds. Its name starts with `prefix`, like
/// `little-pydoc`, so tests of different plugins don't share one.
pub fn folder_with(prefix: &str, test: &str, files: &[(&str, &str)]) -> String {
    let id = std::process::id();
    let root = std::env::temp_dir().join(format!("{prefix}-{test}-{id}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    root.to_str().unwrap().to_string()
}

/// The paths of the pages `added` adds, in order.
pub fn paths(added: &Added) -> Vec<&str> {
    let mut paths: Vec<_> = added.pages.iter().map(|(path, _)| path.as_str()).collect();
    paths.sort();
    paths
}

/// The Markdown of the page `added` adds at `path`.
pub fn markdown<'a>(added: &'a Added, path: &str) -> &'a str {
    let page = added.pages.iter().find(|(page, _)| page == path);
    &page.unwrap_or_else(|| panic!("no page at {path}")).1
}

/// Assert that `page` has each of `parts`, in order.
#[track_caller]
pub fn assert_has(page: &str, parts: &[&str]) {
    let mut rest = page;
    for part in parts {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => panic!("no {part:?}, in order, in:\n{page}"),
        }
    }
}
