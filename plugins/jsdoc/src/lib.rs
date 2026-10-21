//! A little plugin that adds a page of reference for a JavaScript or
//! TypeScript package and each of its modules, from their doc comments.

// Built for anything but WebAssembly, the plugin is only its tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod comment;
mod files;
mod links;
mod objects;
mod print;
mod reader;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use little_api_reference::pages::{Added, join, relative};
use little_api_reference::{markdown, model, settings};

use crate::files::{Layout, Source};
use crate::links::{Scope, Target};
use crate::markdown::{Language, Listed};
use crate::model::{Item, Object, Section};
use crate::reader::Module;

/// What a site sets for the plugin, in its `little.yml`.
#[derive(Debug, PartialEq)]
struct Settings {
    /// The folder the package's code is in, as the plugin sees it, like
    /// `src`.
    read: String,
    /// The package's name, as its `package.json` names it, like `my-lib`
    /// or `@scope/my-lib`.
    package: String,
    /// What the site calls the package's page, like `API reference`, if
    /// it's not its name.
    title: Option<String>,
    /// The folder its pages go in, from the source's, like `my-lib`, or
    /// empty for the source's own.
    url: String,
}

impl Settings {
    /// The settings in `json`, as little gives them to the plugin.
    fn parse(json: &str) -> Result<Settings, String> {
        let mut read = Vec::new();
        let mut package = None;
        let mut title = None;
        let mut url = None;
        for (name, value) in settings::mapping(json)? {
            match name.as_str() {
                "read" => read = settings::folders(&value),
                "package" => {
                    let name = (value.as_str()).filter(|name| {
                        !name.is_empty() && settings::folder(name).as_deref() == Some(name)
                    });
                    package = Some(name.map(String::from).ok_or(
                        "`package` must be the package's name, like `my-lib` or `@scope/my-lib`",
                    )?);
                }
                "title" => title = Some(settings::title(&value, "the package's page")?),
                "url" => url = Some(settings::url(&value)?),
                _ => {
                    return Err(format!(
                        "it has no setting `{name}`, only `read`, the folder the package's code \
                         is in, `package`, its name, `title`, what the site calls its page, and \
                         `url`, the folder its pages go in"
                    ));
                }
            }
        }
        let read = settings::one_folder(read, "package", "the package's code")?;
        let package = package.ok_or("name the package in `package`, like `package: my-lib`")?;
        let url = url.unwrap_or_else(|| package.clone());
        Ok(Settings {
            read,
            package,
            title,
            url,
        })
    }
}

/// The pages of the package in the folder `settings` reads: its own, and
/// one for each of its folders and modules that has something documented.
fn document(settings: &Settings) -> Added {
    let mut warnings = Vec::new();
    let sources = files::walk(&settings.read, &mut warnings);
    if sources.as_ref().is_some_and(Vec::is_empty) {
        warnings.push(format!(
            "`{}` holds no `.js`, `.ts`, or other JavaScript or TypeScript files",
            settings.read
        ));
    }
    let sources = sources.unwrap_or_default();
    if sources.is_empty() {
        return Added {
            pages: Vec::new(),
            warnings,
        };
    }
    let modules: Vec<Module> = (sources.iter())
        .map(|source| reader::read(&settings.read, source, &mut warnings))
        .collect();
    let documented: Vec<bool> = modules.iter().map(Module::is_documented).collect();
    if !documented.contains(&true) {
        warnings.push(format!(
            "`{}` has nothing documented: no doc comment, `/** ... */`, on a module or on what \
             one exports",
            settings.read
        ));
    }
    let layout = files::layout(&settings.package, &sources, &documented);
    let objects: Vec<Object> = (0..layout.pages.len())
        .map(|page| shown(&layout, &modules, page))
        .collect();
    let pages = Pages {
        settings,
        sources: &sources,
        modules: &modules,
        layout: &layout,
        scope: Scope::new(&settings.package, &sources, &modules, &layout),
        ids: (objects.iter())
            .map(|object| markdown::ids(object, &language("js")))
            .collect(),
        objects,
    };
    let mut added = Added {
        pages: (0..layout.pages.len())
            .map(|page| pages.page(page))
            .collect(),
        warnings,
    };
    // The package's page is the first in its layout.
    if let (Some(title), Some((_, page))) = (&settings.title, added.pages.first_mut()) {
        *page = markdown::titled(title, page);
    }
    added
}

/// What the page at `index` shows: the module it shows, if it shows one,
/// titled as the page is.
fn shown(layout: &Layout, modules: &[Module], index: usize) -> Object {
    let page = &layout.pages[index];
    let mut object = match page.module {
        Some(module) => modules[module].object.clone(),
        None => Object::new(&page.title, page.kind),
    };
    object.name = page.title.clone();
    object.kind = page.kind;
    object
}

/// JavaScript, or TypeScript, as a page with code blocks in `name`, like
/// `ts`, writes it.
fn language(name: &'static str) -> Language {
    Language {
        name,
        separator: ".",
    }
}

/// A package's pages, as they're written.
struct Pages<'a> {
    settings: &'a Settings,
    sources: &'a [Source],
    modules: &'a [Module],
    layout: &'a Layout,
    scope: Scope<'a>,
    /// What each page shows, in the order of the pages, with its links as
    /// its comments write them.
    objects: Vec<Object>,
    /// The id of each heading on each page, by the name it shows, like
    /// `Config.load`.
    ids: Vec<HashMap<String, String>>,
}

impl Pages<'_> {
    /// The path of the page at `index`, from the source's folder.
    fn path(&self, index: usize) -> String {
        join(&self.settings.url, &self.layout.pages[index].path)
    }

    /// The page at `index`: its path, and its Markdown.
    fn page(&self, index: usize) -> (String, String) {
        let from = self.path(index);
        let page = &self.layout.pages[index];
        let mut object = self.objects[index].clone();
        if let Some(module) = page.module {
            self.render(&mut object, module, &from);
        }
        let modules: Vec<Listed> = (page.children.iter())
            .map(|&child| {
                self.listed(
                    &self.layout.pages[child].title,
                    Some(Target::Page(child)),
                    &from,
                )
            })
            .collect();
        let re_exports: Vec<Listed> = match page.module {
            Some(module) => (self.modules[module].re_exports.iter())
                .map(|re_export| {
                    let target = self.scope.import(module, &re_export.import);
                    self.listed(&re_export.shown, target, &from)
                })
                .collect(),
            None => Vec::new(),
        };
        let (file, name) = match page.module {
            Some(module) => (
                join(&self.settings.read, &self.sources[module].path),
                self.modules[module].language,
            ),
            None => (String::new(), "js"),
        };
        let lists: [(&str, &[Listed]); 2] = [("Modules", &modules), ("Re-exports", &re_exports)];
        (
            from,
            markdown::page(&object, &file, &language(name), &lists),
        )
    }

    /// `name`, as the page at `from` lists it, with a link to `target`, if
    /// it leads anywhere, and its summary.
    fn listed(&self, name: &str, target: Option<Target>, from: &str) -> Listed {
        Listed {
            name: name.to_string(),
            link: (target.as_ref()).and_then(|target| self.url(target, from)),
            summary: (target.as_ref())
                .map(|target| self.summary(target, from))
                .unwrap_or_default(),
        }
    }

    /// Lead the links in the docs of `object`, from the module at `module`,
    /// and of what it defines, where they do from the page at `from`.
    fn render(&self, object: &mut Object, module: usize, from: &str) {
        object.docs = self.sections(&object.docs, module, from);
        for member in &mut object.members {
            self.render(member, module, from);
        }
    }

    /// `docs`, from the module at `module`, with each `{@link}` leading
    /// where it does from the page at `from`.
    fn sections(&self, docs: &[Section], module: usize, from: &str) -> Vec<Section> {
        let resolve = |path: &str| {
            let target = self.scope.resolve(module, path)?;
            self.url(&target, from)
        };
        let render = |text: &str| links::render(text, &resolve);
        (docs.iter())
            .map(|section| match section {
                Section::Text { title, text } => Section::Text {
                    title: title.clone(),
                    text: render(text),
                },
                Section::Items { kind, items } => Section::Items {
                    kind: *kind,
                    items: (items.iter())
                        .map(|item| Item {
                            description: render(&item.description),
                            ..item.clone()
                        })
                        .collect(),
                },
            })
            .collect()
    }

    /// The link to what `target` is, from the page at `from`: its page, or
    /// its heading, or that of what it's in, if it has none; or none if it
    /// isn't on a page.
    fn url(&self, target: &Target, from: &str) -> Option<String> {
        let (page, id) = match target {
            Target::Page(page) => (*page, None),
            Target::Item { module, name } => {
                let page = self.layout.module_page[*module]?;
                let mut name = name.as_str();
                let id = loop {
                    if let Some(id) = self.ids[page].get(name) {
                        break id;
                    }
                    name = &name[..name.rfind('.')?];
                };
                (page, Some(id))
            }
        };
        let to = self.path(page);
        Some(match id {
            Some(id) if to == from => format!("#{id}"),
            Some(id) => format!("{}#{id}", relative(from, &to)),
            None => relative(from, &to),
        })
    }

    /// The first paragraph of the docs of what `target` is, with its links
    /// leading where they do from the page at `from`; empty if it has none.
    fn summary(&self, target: &Target, from: &str) -> String {
        let (page, name) = match target {
            Target::Page(page) => (Some(*page), ""),
            Target::Item { module, name } => (self.layout.module_page[*module], name.as_str()),
        };
        let Some(page) = page else {
            return String::new();
        };
        let Some(module) = self.layout.pages[page].module else {
            return String::new();
        };
        let mut object = &self.objects[page];
        for part in name.split('.').filter(|part| !part.is_empty()) {
            match object.members.iter().find(|member| member.name == part) {
                Some(member) => object = member,
                None => return String::new(),
            }
        }
        let first = &object.docs[..object.docs.len().min(1)];
        markdown::summary(&self.sections(first, module, from))
    }
}

#[cfg(target_arch = "wasm32")]
mod plugin {
    use std::sync::Mutex;

    use super::{Settings, document};

    wit_bindgen::generate!({
        path: "../../wit/plugin.wit",
        inline: "
            package little:jsdoc;

            world jsdoc {
                import little:plugin/warnings@0.2.0;
                export little:plugin/setup@0.2.0;
                export little:plugin/files@0.2.0;
            }
        ",
        world: "little:jsdoc/jsdoc",
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
