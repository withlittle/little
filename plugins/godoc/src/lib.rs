//! A little plugin that adds a page of reference for a Go module and each
//! of its packages, from their doc comments.

// Built for anything but WebAssembly, the plugin is only its tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod comment;
mod files;
mod package;
mod reader;
mod source;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use little_api_reference::pages::{Added, heading, join, relative};
use little_api_reference::{markdown, model, settings};

use crate::files::Layout;
use crate::markdown::{Language, Listed};
use crate::model::{Object, Section};
use crate::package::Package;

/// Go, as a page's code blocks write it.
const GO: Language = Language {
    name: "go",
    separator: ".",
};

/// What a site sets for the plugin, in its `little.yml`.
#[derive(Debug, PartialEq)]
struct Settings {
    /// The folder of the module's `go.mod`, as the plugin sees it, like `.`.
    read: String,
    /// What the site calls the module's page, like `API reference`, if
    /// it's not its title.
    title: Option<String>,
    /// The folder its pages go in, from the source's, like `api`, or empty
    /// for the source's own; or none, for the last element of the module's
    /// path, but a major version, like `v2`.
    url: Option<String>,
}

impl Settings {
    /// The settings in `json`, as little gives them to the plugin.
    fn parse(json: &str) -> Result<Settings, String> {
        let mut read = Vec::new();
        let mut title = None;
        let mut url = None;
        for (name, value) in settings::mapping(json)? {
            match name.as_str() {
                "read" => read = settings::folders(&value),
                "title" => title = Some(settings::title(&value, "the module's page")?),
                "url" => url = Some(settings::url(&value)?),
                _ => {
                    return Err(format!(
                        "it has no setting `{name}`, only `read`, the folder of the module's \
                         `go.mod`, `title`, what the site calls its page, and `url`, the folder \
                         its pages go in"
                    ));
                }
            }
        }
        if read.is_empty() {
            return Err(
                "name the folder of the module's `go.mod` in `read`, like `read: .`".into(),
            );
        }
        let read = settings::one_folder(read, "module", "the module's `go.mod`")?;
        Ok(Settings { read, title, url })
    }
}

/// The path of the module whose `go.mod` is in `read`, from its `module`
/// line, or why there's none.
fn module_path(read: &str) -> Result<String, String> {
    let path = join(read, "go.mod");
    let text = std::fs::read_to_string(&path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => format!(
            "`{read}` has no `go.mod`: `read` names the folder of a Go module, with its `go.mod`"
        ),
        _ => format!("can't read `{path}`: {error}"),
    })?;
    module_line(&text).ok_or_else(|| {
        format!("`{path}` doesn't name the module's path in a `module` line, like `module example.com/m`")
    })
}

/// The module's path that the first `module` line of `go_mod` names, if
/// it's one Go would import from: its elements made of ASCII letters,
/// digits, and `-._~+`, but none `.` or `..`.
fn module_line(go_mod: &str) -> Option<String> {
    let path = go_mod.lines().find_map(|line| {
        let line = line.split("//").next()?.trim();
        let path = line.strip_prefix("module")?;
        path.starts_with([' ', '\t']).then(|| path.trim())
    })?;
    let unquoted = ["\"", "`"]
        .iter()
        .find_map(|quote| path.strip_prefix(quote)?.strip_suffix(quote));
    let path = unquoted.unwrap_or(path);
    let element = |element: &str| {
        !matches!(element, "" | "." | "..")
            && (element.chars()).all(|c| c.is_ascii_alphanumeric() || "-._~+".contains(c))
    };
    path.split('/').all(element).then(|| path.to_string())
}

/// The pages of the module whose `go.mod` is in the folder `settings`
/// reads: its own, and one for each of its packages that has something
/// documented, and for each folder they're in; or why there are none.
fn document(settings: &Settings) -> Result<Added, String> {
    let module = module_path(&settings.read)?;
    let url = (settings.url.clone()).unwrap_or_else(|| package::short_name(&module).to_string());
    let mut warnings = Vec::new();
    let folders = files::walk(&settings.read, &mut warnings);
    if folders.as_ref().is_some_and(Vec::is_empty) {
        warnings.push(format!(
            "`{}` holds no `.go` files, other than tests",
            settings.read
        ));
    }
    let folders = folders.unwrap_or_default();
    if folders.is_empty() {
        return Ok(Added {
            pages: Vec::new(),
            warnings,
        });
    }
    let packages: Vec<Package> = (folders.iter())
        .filter_map(|folder| package::read(&settings.read, &module, folder, &mut warnings))
        .collect();
    let objects: Vec<Object> = packages.iter().map(Package::object).collect();
    let documented: Vec<bool> = (objects.iter())
        .map(|object| object.is_documented() || !object.members.is_empty())
        .collect();
    if !documented.contains(&true) {
        warnings.push(format!(
            "`{}` has nothing documented: no doc comment on a package, or on what one exports",
            settings.read
        ));
    }
    let titles: Vec<(&str, &str)> = (packages.iter().zip(&objects))
        .map(|(package, object)| (package.folder.as_str(), object.name.as_str()))
        .collect();
    let layout = files::layout(&module, &titles, &documented);
    let shown: Vec<Object> = (layout.pages.iter())
        .map(|page| match page.package {
            Some(package) => objects[package].clone(),
            None => Object::new(&page.title, page.kind),
        })
        .collect();
    let pages = Pages {
        settings,
        module: &module,
        url: &url,
        layout: &layout,
        packages: &packages,
        ids: (shown.iter())
            .map(|object| markdown::ids(object, &GO))
            .collect(),
        objects: shown,
    };
    let mut added = Added {
        pages: (0..layout.pages.len())
            .map(|page| pages.page(page))
            .collect(),
        warnings,
    };
    // The module's page is the first in its layout.
    if let (Some(title), Some((_, page))) = (&settings.title, added.pages.first_mut()) {
        *page = markdown::titled(title, page);
    }
    Ok(added)
}

/// A module's pages, as they're written.
struct Pages<'a> {
    settings: &'a Settings,
    /// The module's path, like `example.com/m`.
    module: &'a str,
    /// The folder the pages go in, from the source's.
    url: &'a str,
    layout: &'a Layout,
    packages: &'a [Package],
    /// What each page shows, in the order of the pages, with its links as
    /// its comments write them.
    objects: Vec<Object>,
    /// The id of each heading on each page, by the name it shows, like
    /// `Config.Load`.
    ids: Vec<HashMap<String, String>>,
}

impl Pages<'_> {
    /// The path of the page at `index`, from the source's folder.
    fn path(&self, index: usize) -> String {
        join(self.url, &self.layout.pages[index].path)
    }

    /// The page at `index`: its path, and its Markdown.
    fn page(&self, index: usize) -> (String, String) {
        let from = self.path(index);
        let page = &self.layout.pages[index];
        let mut object = self.objects[index].clone();
        self.render(&mut object, &from);
        if let Some(package) = page.package.map(|package| &self.packages[package])
            && package.name != "main"
        {
            let code = format!("import \"{}\"\n", package.import_path);
            let fence = markdown::fence_for(&code);
            let text = format!("{fence}go\n{code}{fence}");
            object.docs.insert(0, Section::Text { title: None, text });
        }
        let children: Vec<Listed> = (page.children.iter())
            .map(|&child| Listed {
                name: self.layout.pages[child].title.clone(),
                link: Some(relative(&from, &self.path(child))),
                summary: self.summary(child, &from),
            })
            .collect();
        let file = match page.folder.as_str() {
            "" => self.settings.read.clone(),
            folder => join(&self.settings.read, folder),
        };
        let lists: [(&str, &[Listed]); 1] = [("Packages", &children)];
        (from, markdown::page(&object, &file, &GO, &lists))
    }

    /// Lead the links in the docs of `object`, and of what it defines,
    /// where they do from the page at `from`.
    fn render(&self, object: &mut Object, from: &str) {
        object.docs = self.sections(&object.docs, from);
        for member in &mut object.members {
            self.render(member, from);
        }
    }

    /// `docs`, with each link to Go's code leading where it does from the
    /// page at `from`, or, if it leads to no page, as code.
    fn sections(&self, docs: &[Section], from: &str) -> Vec<Section> {
        let resolve = |target: &str| self.url(target, from);
        let render = |text: &str| comment::render(text, &resolve);
        (docs.iter())
            .map(|section| section.map_text(render))
            .collect()
    }

    /// The link from the page at `from` to `target`, a package's import
    /// path, then `#`, then the name of what it declares, if any, like
    /// `Config.Load`: to its page, at the heading of what it names, or of
    /// what that's in, if it has none; or none, if it's not in the module,
    /// or not on a page.
    fn url(&self, target: &str, from: &str) -> Option<String> {
        let (path, name) = target.split_once('#')?;
        let folder = match path.strip_prefix(self.module)? {
            "" => "",
            rest => rest.strip_prefix('/')?,
        };
        let page = *self.layout.folder_page.get(folder)?;
        let to = self.path(page);
        if name.is_empty() {
            return Some(relative(from, &to));
        }
        let mut name = name;
        let id = loop {
            if let Some(id) = self.ids[page].get(name) {
                break id;
            }
            name = &name[..name.rfind('.')?];
        };
        Some(heading(from, &to, id))
    }

    /// The first paragraph of the docs of what the page at `index` shows,
    /// with its links leading where they do from the page at `from`; empty
    /// if it has none.
    fn summary(&self, index: usize, from: &str) -> String {
        let docs = &self.objects[index].docs;
        markdown::summary(&self.sections(&docs[..docs.len().min(1)], from))
    }
}

little_api_reference::plugin!(document);
