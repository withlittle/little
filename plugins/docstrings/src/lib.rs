//! A little plugin that adds a page of reference for each Python package and
//! module in the folders it reads, from their docstrings.

// Built for anything but WebAssembly, the plugin is only its tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod docstring;
mod markdown;
mod model;
mod python;

use std::path::Path;

use serde_json::Value;

use crate::markdown::Listed;
use crate::model::{Kind, Object};

/// What a site sets for the plugin, in its `little.yml`.
#[derive(Debug, PartialEq)]
struct Settings {
    /// The folders it documents, as it sees them, like `mypackage`.
    read: Vec<String>,
    /// The folder its pages go in, from the source's, like `reference`, or
    /// empty for the source's own.
    url: String,
}

impl Settings {
    /// The settings in `json`, as little gives them to the plugin.
    fn parse(json: &str) -> Result<Settings, String> {
        let Ok(Value::Object(settings)) = serde_json::from_str(json) else {
            return Err(format!("its settings must be a mapping, not {json}"));
        };
        let mut read = Vec::new();
        let mut url = "reference".to_string();
        for (name, value) in settings {
            match name.as_str() {
                "read" => {
                    let folders = value.as_array().into_iter().flatten();
                    read = folders
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect();
                }
                "url" => {
                    url = (value.as_str().and_then(folder)).ok_or(
                        "`url` must be the folder its pages go in, like `/reference`, with no \
                         part `.` or `..`",
                    )?;
                }
                _ => {
                    return Err(format!(
                        "it has no setting `{name}`, only `read`, the folders it documents, \
                         and `url`, the folder their pages go in"
                    ));
                }
            }
        }
        if read.is_empty() {
            return Err("name the folders it documents in `read`, like `read: ./mypackage`".into());
        }
        Ok(Settings { read, url })
    }
}

/// `url` as a folder from the source's, with `/` between its parts, or none
/// if a part is empty, `.`, or `..`.
fn folder(url: &str) -> Option<String> {
    let url = url.trim_matches('/');
    let parts_ok =
        (url.split('/')).all(|part| !matches!(part, "" | "." | "..") && !part.contains('\\'));
    (url.is_empty() || parts_ok).then(|| url.to_string())
}

/// What the plugin adds: each page's path and Markdown, and what it warns
/// of.
#[derive(Debug, Default)]
struct Added {
    pages: Vec<(String, String)>,
    warnings: Vec<String>,
}

/// The pages of the packages and modules in the folders `settings` reads.
fn document(settings: &Settings) -> Result<Added, String> {
    let mut added = Added::default();
    for folder in &settings.read {
        if is_file(&join(folder, "__init__.py")) {
            let name = package_name(folder).ok_or_else(|| {
                format!(
                    "`{folder}` holds an `__init__.py`, so it's a package, but it has no name \
                     there; read the folder it's in, or the folder in it its code is in"
                )
            })?;
            added.package(folder, name, &join(&settings.url, name));
            continue;
        }
        let entries = added.entries(folder);
        if entries.is_empty() {
            added.warn(format!(
                "`{folder}` holds no Python package or module: a package is a folder with an \
                 `__init__.py`"
            ));
        }
        added.add(folder, &entries, &settings.url);
    }
    Ok(added)
}

/// A package or module a folder holds, by its name.
enum Entry {
    Package(String),
    Module(String),
}

impl Added {
    /// Add the pages of the package in `folder`, which is called `name`, in
    /// `dir`, and give back its summary.
    fn package(&mut self, folder: &str, name: &str, dir: &str) -> String {
        let init = join(folder, "__init__.py");
        let package = (self.read(&init, name, Kind::Package))
            .unwrap_or_else(|| Object::new(name, Kind::Package));
        let entries = self.entries(folder);
        let listed = self.add(folder, &entries, dir);
        let markdown = markdown::page(&package, &init, "python", &listed);
        self.pages.push((join(dir, "README.md"), markdown));
        docstring::summary(&package.docs)
    }

    /// Add the pages of `entries`, which are in `folder`, in `dir`, and give
    /// back what a package's page lists of them.
    fn add(&mut self, folder: &str, entries: &[Entry], dir: &str) -> Vec<Listed> {
        let mut listed = Vec::new();
        for entry in entries {
            let (name, link, summary) = match entry {
                Entry::Package(name) => {
                    let summary = self.package(&join(folder, name), name, &join(dir, name));
                    (name, format!("{name}/README.md"), summary)
                }
                Entry::Module(name) => {
                    let file = join(folder, &format!("{name}.py"));
                    let Some(module) = self.read(&file, name, Kind::Module) else {
                        continue;
                    };
                    let markdown = markdown::page(&module, &file, "python", &[]);
                    self.pages
                        .push((join(dir, &format!("{name}.md")), markdown));
                    (name, format!("{name}.md"), docstring::summary(&module.docs))
                }
            };
            listed.push(Listed {
                name: name.clone(),
                link,
                summary,
            });
        }
        listed
    }

    /// The module in `file`, which is called `name`, or none, warning why,
    /// if it can't be read.
    fn read(&mut self, file: &str, name: &str, kind: Kind) -> Option<Object> {
        let read = std::fs::read_to_string(file)
            .map_err(|error| format!("can't read `{file}`: {error}"))
            .and_then(|source| {
                python::module(name, kind, &source)
                    .map_err(|error| format!("can't parse `{file}`: {error}"))
            });
        read.map_err(|warning| self.warn(warning)).ok()
    }

    /// The packages and modules in `folder`, in order of name, but those
    /// whose names start with `_` or `.`; warns if it can't be read.
    fn entries(&mut self, folder: &str) -> Vec<Entry> {
        let names = std::fs::read_dir(folder).map(|entries| {
            let names = entries.filter_map(|entry| entry.ok()?.file_name().into_string().ok());
            names.collect::<Vec<_>>()
        });
        let mut names = match names {
            Ok(names) => names,
            Err(error) => {
                self.warn(format!("can't read `{folder}`: {error}"));
                return Vec::new();
            }
        };
        names.sort();
        let names = names
            .into_iter()
            .filter(|name| !name.starts_with(['_', '.']));
        names
            .filter_map(|name| {
                let path = join(folder, &name);
                match name.strip_suffix(".py") {
                    Some(module) if is_file(&path) => Some(Entry::Module(module.to_string())),
                    _ if is_file(&join(&path, "__init__.py")) => Some(Entry::Package(name)),
                    _ => None,
                }
            })
            .collect()
    }

    fn warn(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

/// The name of the package in `folder`, or none if it's `.` or `..`.
fn package_name(folder: &str) -> Option<&str> {
    (folder.rsplit('/').next()).filter(|name| !matches!(*name, "" | "." | ".."))
}

/// `name` in `folder`, with `/` between them, or `name` alone in `.` or
/// the source's folder.
fn join(folder: &str, name: &str) -> String {
    match folder {
        "" | "." => name.to_string(),
        _ => format!("{folder}/{name}"),
    }
}

fn is_file(path: &str) -> bool {
    Path::new(path).is_file()
}

#[cfg(target_arch = "wasm32")]
mod plugin {
    use std::sync::Mutex;

    use super::{Settings, document};

    wit_bindgen::generate!({ path: "../../wit/plugin.wit", world: "plugin-with-pages" });

    use exports::little::plugin::code_blocks::{self, Block, Output};
    use exports::little::plugin::pages::{self, Added, Page};

    /// What the site set, from `setup`.
    static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);

    struct Plugin;

    impl code_blocks::Guest for Plugin {
        fn setup(settings: String) -> Result<Vec<String>, String> {
            *SETTINGS.lock().unwrap() = Some(Settings::parse(&settings)?);
            Ok(Vec::new())
        }

        fn render(_: Block) -> Result<Output, String> {
            Err("it renders no code blocks".to_string())
        }
    }

    impl pages::Guest for Plugin {
        fn pages() -> Result<Added, String> {
            let settings = SETTINGS.lock().unwrap();
            let added = document(settings.as_ref().expect("little calls `setup` first"))?;
            let pages = added.pages.into_iter();
            Ok(Added {
                pages: pages
                    .map(|(path, markdown)| Page { path, markdown })
                    .collect(),
                warnings: added.warnings,
            })
        }
    }

    export!(Plugin);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder for the test called `test`, holding `files`, each its path
    /// in the folder and what it holds.
    fn folder_with(test: &str, files: &[(&str, &str)]) -> String {
        let id = std::process::id();
        let root = std::env::temp_dir().join(format!("little-docstrings-{test}-{id}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        root.to_str().unwrap().to_string()
    }

    fn settings(read: &[&str], url: &str) -> Settings {
        Settings {
            read: read.iter().map(|folder| folder.to_string()).collect(),
            url: url.to_string(),
        }
    }

    /// The paths of the pages `added` adds, in order.
    fn paths(added: &Added) -> Vec<&str> {
        let mut paths: Vec<_> = added.pages.iter().map(|(path, _)| path.as_str()).collect();
        paths.sort();
        paths
    }

    fn markdown<'a>(added: &'a Added, path: &str) -> &'a str {
        let page = added.pages.iter().find(|(page, _)| page == path);
        &page.unwrap_or_else(|| panic!("no page at {path}")).1
    }

    #[test]
    fn settings_name_the_folders_read_and_where_the_pages_go() {
        let parsed = Settings::parse(r#"{"read":["mypackage","src"]}"#);
        assert_eq!(parsed, Ok(settings(&["mypackage", "src"], "reference")));
        let parsed = Settings::parse(r#"{"read":["."],"url":"/api/python/"}"#);
        assert_eq!(parsed, Ok(settings(&["."], "api/python")));
        let parsed = Settings::parse(r#"{"read":["."],"url":"/"}"#);
        assert_eq!(parsed, Ok(settings(&["."], "")));
    }

    #[test]
    fn settings_must_read_a_folder() {
        let error = Settings::parse(r#"{"url":"/api"}"#).unwrap_err();
        assert!(error.contains("`read: ./mypackage`"), "{error}");
        assert!(Settings::parse(r#"{"read":[]}"#).is_err());
    }

    #[test]
    fn settings_must_be_known() {
        let error = Settings::parse(r#"{"read":["."],"style":"google"}"#).unwrap_err();
        assert!(error.starts_with("it has no setting `style`"), "{error}");
        for url in [r#""../api""#, r#""api/./python""#, r#""api//python""#, "3"] {
            let json = format!(r#"{{"read":["."],"url":{url}}}"#);
            let error = Settings::parse(&json).unwrap_err();
            assert!(error.starts_with("`url` must be"), "{url}: {error}");
        }
    }

    #[test]
    fn a_package_has_a_page_and_one_for_each_of_its_modules_and_packages() {
        let root = folder_with(
            "package",
            &[
                (
                    "pkg/__init__.py",
                    "\"\"\"The package.\n\nMore of it.\"\"\"\n",
                ),
                (
                    "pkg/solving.py",
                    "\"\"\"Solve things.\"\"\"\n\ndef solve():\n    \"\"\"Solve.\"\"\"\n",
                ),
                ("pkg/_private.py", "\"\"\"Private.\"\"\"\n"),
                ("pkg/.hidden.py", "\"\"\"Hidden.\"\"\"\n"),
                ("pkg/broken.py", "def broken(:\n    pass\n"),
                ("pkg/notes.txt", "Not Python."),
                ("pkg/sub/__init__.py", ""),
                ("pkg/sub/deep.py", "\"\"\"Deep down.\"\"\"\n"),
                ("pkg/data/loose.py", "\"\"\"Not in a package.\"\"\"\n"),
                ("pkg/__pycache__/solving.cpython-312.pyc", ""),
            ],
        );
        let added = document(&settings(&[&format!("{root}/pkg")], "reference")).unwrap();
        assert_eq!(
            paths(&added),
            [
                "reference/pkg/README.md",
                "reference/pkg/solving.md",
                "reference/pkg/sub/README.md",
                "reference/pkg/sub/deep.md",
            ]
        );
        assert_eq!(added.warnings.len(), 1, "{:?}", added.warnings);
        let warning = &added.warnings[0];
        let broken = format!("can't parse `{root}/pkg/broken.py`: ");
        assert!(warning.starts_with(&broken), "{warning}");
        assert!(warning.ends_with(" on line 1"), "{warning}");

        let package = markdown(&added, "reference/pkg/README.md");
        assert!(package.starts_with("# pkg\n\n`package`\n\nThe package.\n\nMore of it.\n\n"));
        let modules = "| [`solving`](solving.md) | Solve things. |\n\
                       | [`sub`](sub/README.md) |  |\n";
        assert!(package.contains(modules), "{package}");
        let solving = markdown(&added, "reference/pkg/solving.md");
        assert!(solving.contains(&format!("<code>{root}/pkg/solving.py</code>")));
        let sub = markdown(&added, "reference/pkg/sub/README.md");
        assert!(sub.contains("| [`deep`](deep.md) | Deep down. |"), "{sub}");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_package_that_cant_be_parsed_still_lists_its_modules() {
        let root = folder_with(
            "unparsed",
            &[
                ("pkg/__init__.py", "import (\n"),
                ("pkg/module.py", "\"\"\"A module.\"\"\"\n"),
            ],
        );
        let added = document(&settings(&[&format!("{root}/pkg")], "")).unwrap();
        assert_eq!(paths(&added), ["pkg/README.md", "pkg/module.md"]);
        let warning = format!("can't parse `{root}/pkg/__init__.py`: ");
        assert!(
            added.warnings[0].starts_with(&warning),
            "{:?}",
            added.warnings
        );
        let package = markdown(&added, "pkg/README.md");
        assert!(
            package.contains("| [`module`](module.md) | A module. |"),
            "{package}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_folder_that_isnt_a_package_holds_packages_and_modules() {
        let root = folder_with(
            "folder",
            &[
                ("loose.py", "\"\"\"Loose.\"\"\"\n"),
                ("pkg/__init__.py", ""),
                ("not_a_package/module.py", ""),
            ],
        );
        let added = document(&settings(&[&root], "api")).unwrap();
        assert_eq!(paths(&added), ["api/loose.md", "api/pkg/README.md"]);
        assert!(added.warnings.is_empty(), "{:?}", added.warnings);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_folder_with_no_python_is_warned_of() {
        let root = folder_with("empty", &[("module/code.py", "")]);
        let added = document(&settings(&[&root], "reference")).unwrap();
        assert!(added.pages.is_empty());
        let warning = format!("`{root}` holds no Python package or module: ");
        assert!(
            added.warnings[0].starts_with(&warning),
            "{:?}",
            added.warnings
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_package_is_named_by_its_folder() {
        assert_eq!(package_name("src/mypackage"), Some("mypackage"));
        assert_eq!(package_name("mypackage"), Some("mypackage"));
        assert_eq!(package_name("."), None);
        assert_eq!(package_name("../.."), None);
    }
}
