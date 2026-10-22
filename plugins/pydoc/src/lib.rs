//! A little plugin that adds a page of reference for each Python package and
//! module in the folders it reads, from their docstrings.

// Built for anything but WebAssembly, the plugin is only its tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod docstring;
mod python;

use std::path::Path;

use little_api_reference::pages::{Added, join};
use little_api_reference::{markdown, model, settings};

use crate::markdown::{Language, Listed};
use crate::model::{Kind, Object};

const PYTHON: Language = Language {
    name: "python",
    separator: ".",
};

/// What a site sets for the plugin, in its `little.yml`.
#[derive(Debug, PartialEq)]
struct Settings {
    /// The folders it documents, as it sees them, like `mypackage`.
    read: Vec<String>,
    /// What the site calls the page of the one package or module it
    /// documents, like `API reference`, if it's not its name.
    title: Option<String>,
    /// The folder its pages go in, from the source's, like `api`, or empty
    /// for the source's own.
    url: String,
}

impl Settings {
    /// The settings in `json`, as little gives them to the plugin.
    fn parse(json: &str) -> Result<Settings, String> {
        let mut read = Vec::new();
        let mut title = None;
        let mut url = String::new();
        for (name, value) in settings::mapping(json)? {
            match name.as_str() {
                "read" => read = settings::folders(&value),
                "title" => {
                    let page = "the page of the package or module it documents";
                    title = Some(settings::title(&value, page)?);
                }
                "url" => url = settings::url(&value)?,
                _ => {
                    return Err(format!(
                        "it has no setting `{name}`, only `read`, the folders it documents, \
                         `title`, what the site calls the page of the package or module it \
                         documents, and `url`, the folder their pages go in"
                    ));
                }
            }
        }
        if read.is_empty() {
            return Err("name the folders it documents in `read`, like `read: ./mypackage`".into());
        }
        Ok(Settings { read, title, url })
    }
}

/// What the plugin adds, as it adds it.
#[derive(Default)]
struct Adding(Added);

/// The pages of the packages and modules in the folders `settings` reads.
fn document(settings: &Settings) -> Result<Added, String> {
    let mut added = Adding::default();
    // The packages and modules in the folders it reads, not in a package
    // there, of which `title` titles one.
    let mut tops = Vec::new();
    for folder in &settings.read {
        if is_file(&join(folder, "__init__.py")) {
            let name = package_name(folder).ok_or_else(|| {
                format!(
                    "`{folder}` holds an `__init__.py`, so it's a package, but it has no name \
                     there; read the folder it's in, or the folder in it its code is in"
                )
            })?;
            added.package(folder, name, &join(&settings.url, name));
            tops.push(Entry::Package(name.to_string()));
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
        tops.extend(entries);
    }
    if let Some(title) = &settings.title {
        added.title(title, &tops, &settings.url)?;
    }
    Ok(added.0)
}

/// A package or module a folder holds, by its name.
enum Entry {
    Package(String),
    Module(String),
}

impl Entry {
    fn name(&self) -> &str {
        match self {
            Entry::Package(name) | Entry::Module(name) => name,
        }
    }

    /// The path of its page, from the folder the pages of its folder go in.
    fn page(&self) -> String {
        match self {
            Entry::Package(name) => format!("{name}/README.md"),
            Entry::Module(name) => format!("{name}.md"),
        }
    }
}

impl Adding {
    /// Give the page of the one package or module in `tops`, whose pages
    /// are in `dir`, the title `title`; one for more than one is an error.
    fn title(&mut self, title: &str, tops: &[Entry], dir: &str) -> Result<(), String> {
        let [top] = tops else {
            if tops.is_empty() {
                // A folder with nothing in it is warned of already.
                return Ok(());
            }
            let names: Vec<String> = tops.iter().map(|top| format!("`{}`", top.name())).collect();
            return Err(format!(
                "`title` titles the page of the one package or module it documents, but `read` \
                 holds {}: give each package an entry of its own under `plugins`, with another \
                 name and the same `wasm`, or leave `title` out",
                names.join(", ")
            ));
        };
        let path = join(dir, &top.page());
        // A module that can't be parsed has no page, and is warned of.
        if let Some((_, page)) = self.0.pages.iter_mut().find(|(page, _)| *page == path) {
            *page = markdown::titled(title, page);
        }
        Ok(())
    }

    /// Add the pages of the package in `folder`, which is called `name`, in
    /// `dir`, and give back its summary.
    fn package(&mut self, folder: &str, name: &str, dir: &str) -> String {
        let init = join(folder, "__init__.py");
        let package = (self.read(&init, name, Kind::Package))
            .unwrap_or_else(|| Object::new(name, Kind::Package));
        let entries = self.entries(folder);
        let listed = self.add(folder, &entries, dir);
        let markdown = markdown::page(&package, &init, &PYTHON, &[("Modules", &listed)]);
        self.0.pages.push((join(dir, "README.md"), markdown));
        markdown::summary(&package.docs)
    }

    /// Add the pages of `entries`, which are in `folder`, in `dir`, and give
    /// back what a package's page lists of them.
    fn add(&mut self, folder: &str, entries: &[Entry], dir: &str) -> Vec<Listed> {
        let mut listed = Vec::new();
        for entry in entries {
            let (name, summary) = match entry {
                Entry::Package(name) => {
                    let summary = self.package(&join(folder, name), name, &join(dir, name));
                    (name, summary)
                }
                Entry::Module(name) => {
                    let file = join(folder, &format!("{name}.py"));
                    let Some(module) = self.read(&file, name, Kind::Module) else {
                        continue;
                    };
                    let markdown = markdown::page(&module, &file, &PYTHON, &[]);
                    self.0.pages.push((join(dir, &entry.page()), markdown));
                    (name, markdown::summary(&module.docs))
                }
            };
            listed.push(Listed {
                name: name.clone(),
                link: Some(entry.page()),
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
        self.0.warnings.push(warning);
    }
}

/// The name of the package in `folder`, or none if it's `.` or `..`.
fn package_name(folder: &str) -> Option<&str> {
    (folder.rsplit('/').next()).filter(|name| !matches!(*name, "" | "." | ".."))
}

fn is_file(path: &str) -> bool {
    Path::new(path).is_file()
}

#[cfg(target_arch = "wasm32")]
mod plugin {
    use std::sync::Mutex;

    use super::{Settings, document};

    wit_bindgen::generate!({
        path: "../../wit/plugin.wit",
        inline: "
            package little:pydoc;

            world pydoc {
                import little:plugin/warnings@0.2.1;
                export little:plugin/setup@0.2.1;
                export little:plugin/files@0.2.1;
            }
        ",
        world: "little:pydoc/pydoc",
        generate_all,
    });

    use exports::little::plugin::files::{self, Changes, File};
    use exports::little::plugin::setup;
    use little::plugin::warnings::warn;

    /// What the site set, from `setup`.
    static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);

    struct Plugin;

    impl setup::Guest for Plugin {
        fn setup(settings: String) -> Result<(), String> {
            *SETTINGS.lock().unwrap() = Some(Settings::parse(&settings)?);
            Ok(())
        }
    }

    impl files::Guest for Plugin {
        fn files(_: Vec<String>) -> Result<Changes, String> {
            let settings = SETTINGS.lock().unwrap();
            let added = document(settings.as_ref().expect("little calls `setup` first"))?;
            let add = added.files(warn).into_iter();
            Ok(Changes {
                add: add
                    .map(|(path, contents)| File { path, contents })
                    .collect(),
                remove: Vec::new(),
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
        let root = std::env::temp_dir().join(format!("little-pydoc-{test}-{id}"));
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
            title: None,
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
        assert_eq!(parsed, Ok(settings(&["mypackage", "src"], "")));
        let parsed = Settings::parse(r#"{"read":["."],"url":"/api/python/"}"#);
        assert_eq!(parsed, Ok(settings(&["."], "api/python")));
        let parsed = Settings::parse(r#"{"read":["."],"url":"/"}"#);
        assert_eq!(parsed, Ok(settings(&["."], "")));
        let parsed = Settings::parse(r#"{"read":["."],"title":"API"}"#);
        let titled = Settings {
            title: Some("API".to_string()),
            ..settings(&["."], "")
        };
        assert_eq!(parsed, Ok(titled));
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
        for title in [r#""""#, r#"" ""#, "3", "null"] {
            let json = format!(r#"{{"read":["."],"title":{title}}}"#);
            let error = Settings::parse(&json).unwrap_err();
            assert!(error.starts_with("`title` must be"), "{title}: {error}");
        }
    }

    #[test]
    fn a_title_is_in_the_frontmatter_of_the_one_package_or_module_it_documents() {
        let root = folder_with(
            "title",
            &[
                ("one/pkg/__init__.py", "\"\"\"The package.\"\"\"\n"),
                ("one/pkg/module.py", "\"\"\"A module.\"\"\"\n"),
                ("module/tool.py", "\"\"\"A tool.\"\"\"\n"),
                ("two/pkg/__init__.py", ""),
                ("two/loose.py", "\"\"\"Loose.\"\"\"\n"),
                ("none/notes.txt", "Not Python."),
            ],
        );
        let titled = |read: &[&str]| {
            let titled = Settings {
                title: Some("API: \"pkg\"".to_string()),
                ..settings(read, "api")
            };
            document(&titled)
        };
        let frontmatter = "---\ntitle: \"API: \\\"pkg\\\"\"\n---\n\n";
        for read in [format!("{root}/one/pkg"), format!("{root}/one")] {
            let added = titled(&[&read]).unwrap();
            let package = markdown(&added, "api/pkg/README.md");
            let heading = format!("{frontmatter}# pkg\n");
            assert!(package.starts_with(&heading), "{package}");
            let module = markdown(&added, "api/pkg/module.md");
            assert!(module.starts_with("# module\n"), "{module}");
        }
        let added = titled(&[&format!("{root}/module")]).unwrap();
        let tool = markdown(&added, "api/tool.md");
        assert!(
            tool.starts_with(&format!("{frontmatter}# tool\n")),
            "{tool}"
        );

        let error = titled(&[&format!("{root}/two")]).unwrap_err();
        assert!(error.contains("`read` holds `loose`, `pkg`: "), "{error}");
        let error = titled(&[&format!("{root}/one/pkg"), &format!("{root}/module")]).unwrap_err();
        assert!(error.contains("`read` holds `pkg`, `tool`: "), "{error}");
        let added = titled(&[&format!("{root}/none")]).unwrap();
        assert!(added.pages.is_empty());

        let added = document(&settings(&[&format!("{root}/one")], "api")).unwrap();
        let package = markdown(&added, "api/pkg/README.md");
        assert!(package.starts_with("# pkg\n"), "{package}");
        std::fs::remove_dir_all(root).unwrap();
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
