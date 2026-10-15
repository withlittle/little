//! A little plugin that adds a page of reference for a Rust crate and each
//! of its modules, from their doc comments.

// Built for anything but WebAssembly, the plugin is only its tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod docs;
mod modules;
mod rust;

use std::collections::HashMap;

use little_api_reference::{markdown, model};
use serde_json::Value;

use crate::markdown::{Language, Listed};
use crate::model::{Object, Section};
use crate::modules::{Module, Scopes, Target, join};
use crate::rust::{Context, Documented};

const RUST: Language = Language {
    name: "rust",
    separator: "::",
};

/// What a site sets for the plugin, in its `little.yml`.
#[derive(Debug, PartialEq)]
struct Settings {
    /// The folder the crate's `lib.rs` or `main.rs` is in, as the plugin
    /// sees it, like `src`.
    read: String,
    /// The crate's name, as its code names it, like `my_crate`.
    krate: String,
    /// The folder its pages go in, from the source's, like
    /// `reference/my_crate`, or empty for the source's own.
    url: String,
}

impl Settings {
    /// The settings in `json`, as little gives them to the plugin.
    fn parse(json: &str) -> Result<Settings, String> {
        let Ok(Value::Object(settings)) = serde_json::from_str(json) else {
            return Err(format!("its settings must be a mapping, not {json}"));
        };
        let mut read = Vec::new();
        let mut krate = None;
        let mut url = None;
        for (name, value) in settings {
            match name.as_str() {
                "read" => {
                    let folders = value.as_array().into_iter().flatten();
                    read = folders
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect();
                }
                "crate" => {
                    let name = value.as_str().filter(|name| docs::is_identifier(name));
                    krate = Some(name.map(String::from).ok_or(
                        "`crate` must be the crate's name, as its code names it, like `my_crate`, \
                         with `_` for each `-` in its package's name",
                    )?);
                }
                "url" => {
                    url = Some((value.as_str().and_then(folder)).ok_or(
                        "`url` must be the folder its pages go in, like `/reference`, with no \
                         part `.` or `..`",
                    )?);
                }
                _ => {
                    return Err(format!(
                        "it has no setting `{name}`, only `read`, the folder the crate's code is \
                         in, `crate`, its name, and `url`, the folder its pages go in"
                    ));
                }
            }
        }
        let read = match <[String; 1]>::try_from(read) {
            Ok([folder]) => folder,
            Err(read) if read.is_empty() => {
                return Err(
                    "name the folder the crate's `lib.rs` or `main.rs` is in, in `read`, like \
                     `read: ./src`"
                        .into(),
                );
            }
            Err(_) => {
                return Err(
                    "it documents one crate, in one folder, so `read` names one; for each other \
                     crate, add another entry under `plugins`, with another name and the same \
                     `wasm`"
                        .into(),
                );
            }
        };
        let krate = krate.ok_or("name the crate in `crate`, like `crate: my_crate`")?;
        let url = url.unwrap_or_else(|| format!("reference/{krate}"));
        Ok(Settings { read, krate, url })
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

/// The pages of the crate in the folder `settings` reads, and of each of
/// its modules.
fn document(settings: &Settings) -> Added {
    let mut warnings = Vec::new();
    let modules = modules::read(&settings.read, &settings.krate, &mut warnings);
    let scopes = Scopes::new(&settings.krate, &modules);
    let documented = rust::document(&modules, &scopes, &mut warnings);
    let pages = Pages {
        modules: &modules,
        scopes: &scopes,
        paths: (modules.iter())
            .map(|module| page_path(&settings.url, module))
            .collect(),
        ids: (documented.objects.iter())
            .map(|object| markdown::ids(object, &RUST))
            .collect(),
        documented: &documented,
    };
    Added {
        pages: (0..modules.len()).map(|index| pages.page(index)).collect(),
        warnings,
    }
}

/// The path of the page of `module`, in the folder `url`: the crate's, and
/// that of a module that declares others, is its folder's `README.md`.
fn page_path(url: &str, module: &Module) -> String {
    let path = module.path.join("/");
    match (path.as_str(), module.children.is_empty()) {
        ("", _) => join(url, "README.md"),
        (path, false) => join(url, &format!("{path}/README.md")),
        (path, true) => join(url, &format!("{path}.md")),
    }
}

/// The relative link from the page at `from` to the one at `to`, each a
/// path from the source's folder, like `../config.md`.
fn relative(from: &str, to: &str) -> String {
    let folders: Vec<&str> = from.split('/').collect();
    let folders = &folders[..folders.len() - 1];
    let parts: Vec<&str> = to.split('/').collect();
    let shared = (folders.iter().zip(&parts[..parts.len() - 1]))
        .take_while(|(folder, part)| folder == part)
        .count();
    let up = std::iter::repeat_n("..", folders.len() - shared);
    up.chain(parts[shared..].iter().copied())
        .collect::<Vec<_>>()
        .join("/")
}

/// A crate's pages, as they're written.
struct Pages<'a> {
    modules: &'a [Module],
    scopes: &'a Scopes,
    documented: &'a Documented,
    /// The path of each module's page, in the order of the modules, like
    /// `reference/my_crate/config.md`.
    paths: Vec<String>,
    /// The id of each heading on each module's page, by the name it shows,
    /// like `Config::load`.
    ids: Vec<HashMap<String, String>>,
}

impl Pages<'_> {
    /// The page of the module at `index`: its path, and its Markdown.
    fn page(&self, index: usize) -> (String, String) {
        let from = &self.paths[index];
        let mut object = self.documented.objects[index].clone();
        self.render(&mut object, index, "", from);
        let mut children = self.modules[index].children.clone();
        children.sort_by_key(|&child| &self.modules[child].name);
        let modules: Vec<Listed> = (children.into_iter())
            .map(|child| Listed {
                name: self.modules[child].name.clone(),
                link: Some(relative(from, &self.paths[child])),
                summary: self.summary(&Target::Module(child), from),
            })
            .collect();
        let re_exports: Vec<Listed> = (self.documented.re_exports[index].iter())
            .map(|re_export| {
                let target = self.scopes.resolve(index, None, &re_export.path);
                let path = re_export.path.join("::");
                let name = match &re_export.name {
                    None => format!("{path}::*"),
                    Some(name) if re_export.path.last() == Some(name) => path,
                    Some(name) => format!("{path} as {name}"),
                };
                Listed {
                    name,
                    link: (target.as_ref()).and_then(|target| self.url(target, from)),
                    summary: (target.as_ref())
                        .map(|target| self.summary(target, from))
                        .unwrap_or_default(),
                }
            })
            .collect();
        let file = &self.modules[index].file;
        let lists: [(&str, &[Listed]); 2] = [("Modules", &modules), ("Re-exports", &re_exports)];
        (from.clone(), markdown::page(&object, file, &RUST, &lists))
    }

    /// Lead the links in the docs of `object`, on the page of the module at
    /// `page`, whose heading shows `name`, and of what it defines, where
    /// they do from the page at `from`.
    fn render(&self, object: &mut Object, page: usize, name: &str, from: &str) {
        if let Some(context) = self.documented.contexts.get(&(page, name.to_string())) {
            object.docs = self.sections(&object.docs, context, from);
        }
        for member in &mut object.members {
            let name = match name {
                "" => member.name.clone(),
                name => format!("{name}::{}", member.name),
            };
            self.render(member, page, &name, from);
        }
    }

    /// `docs`, read where `context` says, with each link to a path leading
    /// where it does from the page at `from`: the text they start with is
    /// split into sections at its headings.
    fn sections(&self, docs: &[Section], context: &Context, from: &str) -> Vec<Section> {
        let resolve = |path: &[String]| {
            let target = (self.scopes).resolve(context.module, context.this.as_ref(), path)?;
            self.url(&target, from)
        };
        let mut sections = Vec::new();
        for section in docs {
            match section {
                Section::Text { title: None, text } => {
                    sections.extend(docs::render(text, &resolve));
                }
                Section::Text { title, text } => sections.push(Section::Text {
                    title: title.clone(),
                    text: docs::links(text, &resolve),
                }),
                Section::Items { kind, items } => {
                    let items = (items.iter())
                        .map(|item| model::Item {
                            description: docs::links(&item.description, &resolve),
                            ..item.clone()
                        })
                        .collect();
                    sections.push(Section::Items { kind: *kind, items });
                }
            }
        }
        sections
    }

    /// The link to what `target` is, from the page at `from`: its page, or
    /// its heading, or, for a member without one, like a variant, its
    /// type's; or none if it has neither.
    fn url(&self, target: &Target, from: &str) -> Option<String> {
        let (page, id) = match target {
            Target::Module(module) => (*module, None),
            Target::Item {
                module,
                name,
                member,
            } => {
                let ids = &self.ids[*module];
                let member =
                    (member.as_ref()).and_then(|member| ids.get(&format!("{name}::{member}")));
                (*module, Some(member.or_else(|| ids.get(name))?))
            }
        };
        let to = &self.paths[page];
        Some(match id {
            Some(id) if to == from => format!("#{id}"),
            Some(id) => format!("{}#{id}", relative(from, to)),
            None => relative(from, to),
        })
    }

    /// The first paragraph of the docs of what `target` is, with its links
    /// leading where they do from the page at `from`; empty if it has none.
    fn summary(&self, target: &Target, from: &str) -> String {
        let (page, name) = match target {
            Target::Module(module) => (*module, String::new()),
            Target::Item {
                module,
                name,
                member: None,
            } => (*module, name.clone()),
            Target::Item {
                module,
                name,
                member: Some(member),
            } => (*module, format!("{name}::{member}")),
        };
        let mut object = &self.documented.objects[page];
        for part in name.split("::").filter(|part| !part.is_empty()) {
            match object.members.iter().find(|member| member.name == part) {
                Some(member) => object = member,
                None => return String::new(),
            }
        }
        let Some(context) = self.documented.contexts.get(&(page, name)) else {
            return String::new();
        };
        let first = &object.docs[..object.docs.len().min(1)];
        markdown::summary(&self.sections(first, context, from))
    }
}

#[cfg(target_arch = "wasm32")]
mod plugin {
    use std::sync::Mutex;

    use super::{Settings, document};

    wit_bindgen::generate!({
        path: "../../wit/plugin.wit",
        inline: "
            package little:doc-comments;

            world doc-comments {
                import little:plugin/warnings@0.2.0;
                export little:plugin/setup@0.2.0;
                export little:plugin/files@0.2.0;
            }
        ",
        world: "little:doc-comments/doc-comments",
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
            let added = document(settings.as_ref().expect("little calls `setup` first"));
            for warning in &added.warnings {
                warn(warning);
            }
            let pages = added.pages.into_iter();
            Ok(Changes {
                add: pages
                    .map(|(path, markdown)| File {
                        path,
                        contents: markdown.into_bytes(),
                    })
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
        let root = std::env::temp_dir().join(format!("little-doc-comments-{test}-{id}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        root.to_str().unwrap().to_string()
    }

    fn settings(read: &str, krate: &str, url: &str) -> Settings {
        Settings {
            read: read.to_string(),
            krate: krate.to_string(),
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
    fn settings_name_the_folder_read_its_crate_and_where_the_pages_go() {
        let parsed = Settings::parse(r#"{"read":["src"],"crate":"my_crate"}"#);
        assert_eq!(
            parsed,
            Ok(settings("src", "my_crate", "reference/my_crate"))
        );
        let parsed = Settings::parse(r#"{"read":["."],"crate":"little","url":"/api/"}"#);
        assert_eq!(parsed, Ok(settings(".", "little", "api")));
        let parsed = Settings::parse(r#"{"read":["src"],"crate":"little","url":"/"}"#);
        assert_eq!(parsed, Ok(settings("src", "little", "")));
    }

    #[test]
    fn settings_must_read_one_folder_and_name_its_crate() {
        let error = Settings::parse(r#"{"crate":"little"}"#).unwrap_err();
        assert!(error.contains("`read: ./src`"), "{error}");
        let error = Settings::parse(r#"{"read":["a","b"],"crate":"little"}"#).unwrap_err();
        assert!(error.contains("another entry under `plugins`"), "{error}");
        let error = Settings::parse(r#"{"read":["src"]}"#).unwrap_err();
        assert!(error.contains("`crate: my_crate`"), "{error}");
        for name in [r#""my-crate""#, r#""""#, "3"] {
            let json = format!(r#"{{"read":["src"],"crate":{name}}}"#);
            let error = Settings::parse(&json).unwrap_err();
            assert!(error.starts_with("`crate` must be"), "{name}: {error}");
        }
    }

    #[test]
    fn settings_must_be_known() {
        let json = r#"{"read":["src"],"crate":"x","style":"google"}"#;
        let error = Settings::parse(json).unwrap_err();
        assert!(error.starts_with("it has no setting `style`"), "{error}");
        for url in [r#""../api""#, r#""api/./rust""#, r#""api//rust""#, "3"] {
            let json = format!(r#"{{"read":["src"],"crate":"x","url":{url}}}"#);
            let error = Settings::parse(&json).unwrap_err();
            assert!(error.starts_with("`url` must be"), "{url}: {error}");
        }
    }

    #[test]
    fn relative_links_go_up_to_the_folder_two_pages_share() {
        assert_eq!(relative("api/README.md", "api/config.md"), "config.md");
        assert_eq!(
            relative("api/site/versions.md", "api/config.md"),
            "../config.md"
        );
        assert_eq!(
            relative("api/config.md", "api/site/README.md"),
            "site/README.md"
        );
        assert_eq!(relative("README.md", "a/b/c.md"), "a/b/c.md");
        assert_eq!(relative("a/b/c.md", "README.md"), "../../README.md");
        assert_eq!(relative("api/config.md", "api/config.md"), "config.md");
    }

    #[test]
    fn each_module_has_a_page_where_rustc_finds_its_file() {
        let root = folder_with(
            "walk",
            &[
                (
                    "src/lib.rs",
                    "//! The crate.\n\nmod flat;\npub mod nested;\nmod inline {\n    //! Inline.\n\n    \
                     mod deeper;\n}\n#[path = \"elsewhere/named.rs\"]\nmod renamed;\nmod missing;\n\
                     mod broken;\n#[cfg(test)]\nmod tests;\n#[doc(hidden)]\npub mod hidden;\n\
                     #[cfg(unix)]\n#[path = \"unix.rs\"]\nmod platform;\n#[cfg(windows)]\n\
                     #[path = \"windows.rs\"]\nmod platform;\n",
                ),
                ("src/flat.rs", "//! Flat.\n"),
                ("src/nested/mod.rs", "//! Nested.\n\nmod child;\n"),
                ("src/nested/child.rs", ""),
                ("src/inline/deeper.rs", "//! Deeper.\n"),
                ("src/elsewhere/named.rs", "mod beside;\n"),
                ("src/elsewhere/beside.rs", ""),
                ("src/broken.rs", "fn broken(\n"),
                ("src/tests.rs", "fn broken(\n"),
                ("src/hidden.rs", ""),
                ("src/unix.rs", "//! Unix.\n"),
                ("src/windows.rs", "//! Windows.\n"),
            ],
        );
        let added = document(&settings(&format!("{root}/src"), "demo", "api"));
        assert_eq!(
            paths(&added),
            [
                "api/README.md",
                "api/flat.md",
                "api/inline/README.md",
                "api/inline/deeper.md",
                "api/nested/README.md",
                "api/nested/child.md",
                "api/platform.md",
                "api/renamed/README.md",
                "api/renamed/beside.md",
            ]
        );
        assert_eq!(added.warnings.len(), 2, "{:?}", added.warnings);
        let missing = format!(
            "can't find module `missing`, which `{root}/src/lib.rs` declares: there's no \
             `{root}/src/missing.rs` or `{root}/src/missing/mod.rs`"
        );
        assert_eq!(added.warnings[0], missing);
        let broken = format!("can't parse `{root}/src/broken.rs`: ");
        assert!(
            added.warnings[1].starts_with(&broken),
            "{:?}",
            added.warnings
        );

        let krate = markdown(&added, "api/README.md");
        assert!(
            krate.starts_with("# demo\n\n`crate`\n\nThe crate.\n\n"),
            "{krate}"
        );
        let modules = "| [`flat`](flat.md) | Flat. |\n\
                       | [`inline`](inline/README.md) | Inline. |\n\
                       | [`nested`](nested/README.md) | Nested. |\n\
                       | [`platform`](platform.md) | Unix. |\n\
                       | [`renamed`](renamed/README.md) |  |\n";
        assert!(krate.contains(modules), "{krate}");
        let inline = markdown(&added, "api/inline/README.md");
        assert!(
            inline.contains("| [`deeper`](deeper.md) | Deeper. |"),
            "{inline}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_folder_with_no_crate_is_warned_of() {
        let root = folder_with("empty", &[("src/config.rs", "")]);
        let added = document(&settings(&format!("{root}/src"), "demo", "api"));
        assert!(added.pages.is_empty());
        let warning = format!("`{root}/src` holds no `lib.rs` or `main.rs`");
        assert!(
            added.warnings[0].starts_with(&warning),
            "{:?}",
            added.warnings
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn links_lead_to_the_pages_and_headings_of_what_they_name() {
        let root = folder_with(
            "links",
            &[
                (
                    "src/main.rs",
                    "//! The crate, with its [`Config`] and [`site`].\n\npub mod config;\n\
                     pub mod site;\n\npub use config::Config;\npub use site::*;\n\
                     pub use serde::Serialize;\n",
                ),
                (
                    "src/config.rs",
                    "//! Its config.\n\n/// A config, which [`Config::load`] reads.\n\
                     #[derive(Clone, Debug)]\npub struct Config {\n    /// Where it's from, a \
                     [`Source`].\n    pub source: Source,\n    secret: u8,\n}\n\n\
                     /// Where a config is from.\npub enum Source {\n    /// A file.\n    File,\n}\n\n\
                     impl Config {\n    /// Read it, and [`Self::check`] it, from \
                     [`Source::File`].\n    pub fn load() -> Config {\n        todo!()\n    }\n\n    \
                     /// Check it, but not its [`secret`](Self::secret), or [`missing`].\n    \
                     pub fn check(&self) {}\n\n    fn private(&self) {}\n}\n\n\
                     impl Default for Config {\n    fn default() -> Config {\n        todo!()\n    \
                     }\n}\n\nimpl crate::site::Build for Config {}\n",
                ),
                (
                    "src/site/mod.rs",
                    "//! Builds a site from its [`Config`].\n\npub mod versions;\n\n\
                     use crate::config::Config;\n\n/// What builds.\npub trait Build {\n    \
                     /// Build it, with [`build`].\n    fn build(&self) {}\n}\n\n\
                     /// Build the site from its [`Config`], [as it says](Config::load).\n\
                     pub fn build(config: &Config) {}\n",
                ),
                (
                    "src/site/versions.rs",
                    "//! Versions, which [`super::build`] builds, in [`crate`].\n",
                ),
            ],
        );
        let added = document(&settings(&format!("{root}/src"), "demo", "api"));
        assert!(added.warnings.is_empty(), "{:?}", added.warnings);
        assert_eq!(
            paths(&added),
            [
                "api/README.md",
                "api/config.md",
                "api/site/README.md",
                "api/site/versions.md"
            ]
        );

        let krate = markdown(&added, "api/README.md");
        let docs =
            "The crate, with its [`Config`](config.md#config-1) and [`site`](site/README.md).";
        assert!(krate.contains(docs), "{krate}");
        let modules = "| [`config`](config.md) | Its config. |\n\
                       | [`site`](site/README.md) | Builds a site from its \
                       [`Config`](config.md#config-1). |\n";
        assert!(krate.contains(modules), "{krate}");
        let re_exports = "**Re-exports:**\n\n| Name | Description |\n| --- | --- |\n\
                          | [`config::Config`](config.md#config-1) | A config, which \
                          [`Config::load`](config.md#configload) reads. |\n\
                          | [`site::*`](site/README.md) | Builds a site from its \
                          [`Config`](config.md#config-1). |\n\
                          | `serde::Serialize` |  |\n";
        assert!(krate.contains(re_exports), "{krate}");

        let config = markdown(&added, "api/config.md");
        for written in [
            "A config, which [`Config::load`](#configload) reads.",
            "| `source` | `Source` | Where it's from, a [`Source`](#source). |",
            "**Implements:**\n\n`Clone`, `Debug`, `Default`, [`Build`](site/README.md#build)\n",
            "Read it, and [`Self::check`](#configcheck) it, from [`Source::File`](#source).",
            "Check it, but not its [`secret`](#config-1), or `missing`.",
            "pub struct Config {\n    pub source: Source,\n    /* private fields */\n}",
        ] {
            assert!(config.contains(written), "{written}\n\n{config}");
        }
        assert!(!config.contains("fn private"), "{config}");

        let site = markdown(&added, "api/site/README.md");
        for written in [
            "Builds a site from its [`Config`](../config.md#config-1).",
            "Build it, with [`build`](#build-1).",
            "Build the site from its [`Config`](../config.md#config-1), \
             [as it says](../config.md#configload).",
            "| [`versions`](versions.md) | Versions, which [`super::build`](#build-1) builds, in \
             [`crate`](../README.md). |",
        ] {
            assert!(site.contains(written), "{written}\n\n{site}");
        }
        let versions = markdown(&added, "api/site/versions.md");
        let docs = "Versions, which [`super::build`](README.md#build-1) builds, in \
                    [`crate`](../README.md).";
        assert!(versions.contains(docs), "{versions}");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn littles_own_code_is_documented() {
        let added = document(&settings("../../src", "little", "api"));
        assert!(added.warnings.is_empty(), "{:?}", added.warnings);
        let paths = paths(&added);
        for path in [
            "api/README.md",
            "api/config.md",
            "api/site.md",
            "api/plugin/README.md",
            "api/theme/builtin.md",
        ] {
            assert!(paths.contains(&path), "{path}: {paths:?}");
        }
    }
}
