//! The folders of a Go module that hold its packages, and the page that
//! shows each.

use std::collections::{BTreeSet, HashMap};

use little_api_reference::model::Kind;
use little_api_reference::pages::{join, last, names, parent};

/// A folder of the module's that holds Go files, other than tests.
#[derive(Debug, PartialEq)]
pub struct Folder {
    /// Its path from the module's, like `internal/config`, or empty for
    /// the module's own.
    pub path: String,
    /// The names of its Go files, but tests, in order.
    pub files: Vec<String>,
}

/// The folders of the module at `root` that hold Go files, other than
/// tests, each before the folders in it, as the go command finds its
/// packages: not in `testdata` or `vendor`, nor in a folder whose name
/// starts with `.` or `_`, nor in another module, with a `go.mod` of its
/// own. None, warning why, if `root` can't be read.
pub fn walk(root: &str, warnings: &mut Vec<String>) -> Option<Vec<Folder>> {
    let mut folders = Vec::new();
    visit(root, "", &mut folders, warnings)?;
    Some(folders)
}

/// Add `folder`, in `root`, if it holds Go files, and the folders in it,
/// to `folders`; or none, warning why, if `folder` can't be read.
fn visit(
    root: &str,
    folder: &str,
    folders: &mut Vec<Folder>,
    warnings: &mut Vec<String>,
) -> Option<()> {
    let path = match folder {
        "" => root.to_string(),
        folder => join(root, folder),
    };
    let names = names(&path)
        .map_err(|warning| warnings.push(warning))
        .ok()?;
    let mut files = Vec::new();
    let mut inner = Vec::new();
    for name in names {
        if name.starts_with(['.', '_']) {
            continue;
        }
        let Ok(metadata) = std::fs::metadata(join(&path, &name)) else {
            continue;
        };
        if metadata.is_dir() {
            let module = std::fs::metadata(join(&join(&path, &name), "go.mod"));
            if !matches!(name.as_str(), "testdata" | "vendor") && !module.is_ok_and(|m| m.is_file())
            {
                inner.push(name);
            }
        } else if name.ends_with(".go") && !name.ends_with("_test.go") {
            files.push(name);
        }
    }
    if !files.is_empty() {
        folders.push(Folder {
            path: folder.to_string(),
            files,
        });
    }
    for name in inner {
        visit(root, &join(folder, &name), folders, warnings);
    }
    Some(())
}

/// A page of the reference.
#[derive(Debug, PartialEq)]
pub struct Page {
    /// Its path, from the folder the pages go in, like `config/README.md`.
    pub path: String,
    /// The folder it shows, from the module's, like `config`.
    pub folder: String,
    /// Its title: its package's, or else, for the module's own folder, the
    /// module's path, and for another, its name.
    pub title: String,
    /// `Package`, for a folder with a package, or else `Module`, for the
    /// module's own, and `Directory`, for another.
    pub kind: Kind,
    /// The package it shows, by its index, if its folder has one.
    pub package: Option<usize>,
    /// The pages of the folders in its folder, by their indexes, in order
    /// of their titles.
    pub children: Vec<usize>,
}

/// The pages of a module's reference.
#[derive(Debug)]
pub struct Layout {
    /// Its pages, the module's own first.
    pub pages: Vec<Page>,
    /// The page of each folder that has one, by its path.
    pub folder_page: HashMap<String, usize>,
}

/// The pages of the module whose path is `module`, and whose packages are
/// `packages`, each its folder and its title, of which those `documented`
/// are shown: the module's own, `README.md`, and the `README.md` of each
/// folder with a package shown, and of each folder those are in.
pub fn layout(module: &str, packages: &[(&str, &str)], documented: &[bool]) -> Layout {
    let in_folder: HashMap<&str, usize> = (packages.iter().enumerate())
        .map(|(index, &(folder, _))| (folder, index))
        .collect();
    let mut folders = BTreeSet::new();
    for (&(mut folder, _), _) in (packages.iter().zip(documented)).filter(|(_, shown)| **shown) {
        while !folder.is_empty() {
            folders.insert(folder);
            folder = parent(folder);
        }
    }
    let root = in_folder.get("").copied();
    let mut pages = vec![Page {
        path: "README.md".to_string(),
        folder: String::new(),
        title: root
            .map_or(module, |package| packages[package].1)
            .to_string(),
        kind: if root.is_some() {
            Kind::Package
        } else {
            Kind::Module
        },
        package: root,
        children: Vec::new(),
    }];
    let mut folder_page = HashMap::from([(String::new(), 0)]);
    // A folder comes after the one it's in, whose path starts its own.
    for folder in folders {
        let package = in_folder.get(folder).copied();
        let page = pages.len();
        pages[folder_page[parent(folder)]].children.push(page);
        folder_page.insert(folder.to_string(), page);
        pages.push(Page {
            path: format!("{folder}/README.md"),
            folder: folder.to_string(),
            title: package
                .map_or(last(folder), |package| packages[package].1)
                .to_string(),
            kind: if package.is_some() {
                Kind::Package
            } else {
                Kind::Directory
            },
            package,
            children: Vec::new(),
        });
    }
    for index in 0..pages.len() {
        let mut children = std::mem::take(&mut pages[index].children);
        children.sort_by_key(|&page| (&pages[page].title, &pages[page].path));
        pages[index].children = children;
    }
    Layout { pages, folder_page }
}

#[cfg(test)]
mod tests {
    use little_api_reference::testing::folder_with;

    use super::*;

    #[test]
    fn the_go_files_of_each_package_are_found_as_the_go_command_finds_them() {
        let files = [
            "go.mod",
            "b.go",
            "a.go",
            "a_test.go",
            "README.md",
            ".hidden.go",
            "_draft.go",
            "config/config.go",
            "config/testdata/x.go",
            "config/internal/load/load.go",
            "docs/guide.md",
            "vendor/example.com/x/x.go",
            "_examples/x.go",
            ".git/x.go",
            "tools/go.mod",
            "tools/tools.go",
            "tests/only_test.go",
        ]
        .map(|path| (path, ""));
        let root = folder_with("little-godoc-files", "walk", &files);
        let mut warnings = Vec::new();
        let folders = walk(&root, &mut warnings).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        let found: Vec<(&str, Vec<&str>)> = (folders.iter())
            .map(|folder| {
                (
                    folder.path.as_str(),
                    folder.files.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                ("", vec!["a.go", "b.go"]),
                ("config", vec!["config.go"]),
                ("config/internal/load", vec!["load.go"]),
            ]
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_module_that_cant_be_read_is_warned_of() {
        let mut warnings = Vec::new();
        assert_eq!(walk("/no/such/module", &mut warnings), None);
        assert!(warnings[0].starts_with("can't read `/no/such/module`: "));
    }

    #[test]
    fn each_folder_with_a_package_shown_has_a_page_as_do_the_folders_its_in() {
        let packages = [
            ("", "demo"),
            ("config", "config"),
            ("encoding/json", "json"),
            ("encoding/xml", "xml"),
            ("internal/secret", "secret"),
            ("cmd/demo", "demo"),
            ("cmd", "cmd"),
        ];
        let documented = [true, true, true, false, false, true, false];
        let layout = layout("example.com/demo", &packages, &documented);
        let pages: Vec<(&str, &str, Kind, Option<usize>)> = (layout.pages.iter())
            .map(|page| {
                (
                    page.path.as_str(),
                    page.title.as_str(),
                    page.kind,
                    page.package,
                )
            })
            .collect();
        assert_eq!(
            pages,
            [
                ("README.md", "demo", Kind::Package, Some(0)),
                ("cmd/README.md", "cmd", Kind::Package, Some(6)),
                ("cmd/demo/README.md", "demo", Kind::Package, Some(5)),
                ("config/README.md", "config", Kind::Package, Some(1)),
                ("encoding/README.md", "encoding", Kind::Directory, None),
                ("encoding/json/README.md", "json", Kind::Package, Some(2)),
            ]
        );
        assert_eq!(layout.pages[0].children, [1, 3, 4]);
        assert_eq!(layout.pages[1].children, [2]);
        assert_eq!(layout.pages[4].children, [5]);
        assert_eq!(layout.folder_page["encoding/json"], 5);
        assert_eq!(layout.folder_page.get("encoding/xml"), None);
    }

    #[test]
    fn a_module_with_no_package_of_its_own_is_titled_with_its_path() {
        let module = layout("example.com/demo", &[("a", "a")], &[false]);
        assert_eq!(module.pages.len(), 1);
        assert_eq!(module.pages[0].title, "example.com/demo");
        assert_eq!(module.pages[0].kind, Kind::Module);
        assert_eq!(module.pages[0].package, None);
        let package = layout("example.com/demo", &[("", "demo")], &[false]);
        assert_eq!(package.pages[0].title, "demo");
        assert_eq!(package.pages[0].package, Some(0));
    }
}
