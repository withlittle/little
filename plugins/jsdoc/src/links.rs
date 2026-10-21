//! The links in doc comments, like `{@link Config}` or
//! `{@link module:my-lib/strings.pad | pad}`, and where each leads: to what
//! a module shows, by the name the module calls it, exports it as, or
//! imports it as, or to a module's page.

use std::collections::HashMap;
use std::ops::Range;

use little_api_reference::pages::join;
use pulldown_cmark::{Event, Options, Parser, Tag};

use crate::files::{self, Layout, Source};
use crate::reader::{Export, Import, Module};

/// Where a link leads.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// A page, by its index.
    Page(usize),
    /// What the module at `module` shows under `name`, like `Config.load`.
    Item { module: usize, name: String },
}

/// What the links in a package's doc comments can lead to.
pub struct Scope<'a> {
    /// The package's name, like `my-lib`.
    package: &'a str,
    sources: &'a [Source],
    modules: &'a [Module],
    layout: &'a Layout,
    /// Each module, by its index, by its id, like `util/strings`.
    ids: HashMap<String, usize>,
}

impl<'a> Scope<'a> {
    /// What the links in the package called `package`, whose modules are
    /// `modules`, read from `sources`, can lead to, on the pages of
    /// `layout`.
    pub fn new(
        package: &'a str,
        sources: &'a [Source],
        modules: &'a [Module],
        layout: &'a Layout,
    ) -> Scope<'a> {
        let ids = (sources.iter().enumerate())
            .map(|(index, source)| (source.id.clone(), index))
            .collect();
        Scope {
            package,
            sources,
            modules,
            layout,
            ids,
        }
    }

    /// Where a link to `path`, in a doc comment in the module at `module`,
    /// leads, if it leads anywhere: a module's, like `module:my-lib/config`,
    /// or a name, as the module calls it, exports it, or imports it, or else
    /// as only one module of the package shows it, then the names of what
    /// it defines, after `.`, `#`, or `~`, like `Config#load`.
    pub fn resolve(&self, module: usize, path: &str) -> Option<Target> {
        let path = path.trim();
        let path = path.strip_suffix("()").unwrap_or(path);
        if let Some(path) = path.strip_prefix("module:") {
            return self.module_path(path);
        }
        let (first, rest) = split(path);
        let found = &self.modules[module];
        let target = if let Some(heading) = found.locals.get(first) {
            Target::Item {
                module,
                name: heading.clone(),
            }
        } else if found.exports.contains_key(first) {
            self.export(module, first, &mut Vec::new())?
        } else if let Some(import) = found.imports.get(first) {
            self.import(module, import)?
        } else {
            self.anywhere(first)?
        };
        self.within(target, rest)
    }

    /// Where `import`, in the module at `module`, leads: to what the
    /// module it imports from exports, or, for all of it, its page.
    pub fn import(&self, module: usize, import: &Import) -> Option<Target> {
        self.imported(module, import, &mut Vec::new())
    }

    /// Where `import`, in the module at `module`, leads, but through none of
    /// the modules `seen`.
    fn imported(&self, module: usize, import: &Import, seen: &mut Vec<usize>) -> Option<Target> {
        let path = &self.sources[module].path;
        let from = files::resolve(path, &import.specifier, &self.ids)?;
        match &import.name {
            Some(name) => self.export(from, name, seen),
            None => self.page(from),
        }
    }

    /// Where what the module at `module` exports as `name` leads, but
    /// through none of the modules `seen`: to what it shows, or to what it
    /// exports from another module, or else from the modules it exports
    /// all of, which never export a default.
    fn export(&self, module: usize, name: &str, seen: &mut Vec<usize>) -> Option<Target> {
        if seen.contains(&module) {
            return None;
        }
        seen.push(module);
        let found = &self.modules[module];
        match found.exports.get(name) {
            Some(Export::Local(heading)) => Some(Target::Item {
                module,
                name: heading.clone()?,
            }),
            Some(Export::From(import)) => self.imported(module, import, seen),
            None if name == "default" => None,
            None => found.stars.iter().find_map(|specifier| {
                let path = &self.sources[module].path;
                let star = files::resolve(path, specifier, &self.ids)?;
                self.export(star, name, seen)
            }),
        }
    }

    /// The page of the module at `module`, if it has one.
    fn page(&self, module: usize) -> Option<Target> {
        self.layout.module_page[module].map(Target::Page)
    }

    /// What only one module of the package shows under `name`, if just
    /// one does.
    fn anywhere(&self, name: &str) -> Option<Target> {
        let mut found = (self.modules.iter().enumerate()).filter(|(_, module)| {
            module
                .object
                .members
                .iter()
                .any(|member| member.name == name)
        });
        let (module, _) = found.next()?;
        let name = name.to_string();
        found
            .next()
            .is_none()
            .then_some(Target::Item { module, name })
    }

    /// Where `rest`, the names after a link's first, like `.load` in
    /// `Config.load`, lead within `target`: to what a module exports, or to
    /// what it shows, under the name of what it's in, then a `.`.
    fn within(&self, target: Target, rest: &str) -> Option<Target> {
        let Some(rest) = rest.get(1..) else {
            return Some(target);
        };
        match target {
            Target::Item { module, name } => Some(Target::Item {
                module,
                name: format!("{name}.{}", rest.replace(['#', '~'], ".")),
            }),
            Target::Page(page) => {
                let module = self.layout.pages[page].module?;
                let (first, rest) = split(rest);
                let target = match self.export(module, first, &mut Vec::new()) {
                    Some(target) => target,
                    None => Target::Item {
                        module,
                        name: self.modules[module].locals.get(first)?.clone(),
                    },
                };
                self.within(target, rest)
            }
        }
    }

    /// Where `path`, after `module:`, leads: to the module or folder that
    /// the longest name it starts with names, then what that defines.
    fn module_path(&self, path: &str) -> Option<Target> {
        let path = path.trim_matches(['"', '\'']);
        let ends = path.match_indices(['.', '#', '~']).map(|(end, _)| end);
        let mut ends: Vec<usize> = ends.chain([path.len()]).collect();
        ends.reverse();
        let (end, target) = ends
            .into_iter()
            .find_map(|end| Some((end, self.named(&path[..end])?)))?;
        self.within(target, &path[end..])
    }

    /// The page of the module or folder called `name`: the module whose
    /// `@module` tag names it so, or else whose path from the package's
    /// folder it is, without its extension, with the package's name before
    /// it or not, like `my-lib/util/strings`, or that of the folder at that
    /// path, or the package's own.
    fn named(&self, name: &str) -> Option<Target> {
        let named = self
            .modules
            .iter()
            .position(|module| module.name.as_deref() == Some(name));
        if let Some(module) = named {
            return self.page(module);
        }
        let id = match name.strip_prefix(self.package) {
            Some("") => return Some(Target::Page(0)),
            Some(id) if id.starts_with('/') => &id[1..],
            _ => name,
        };
        let module = self
            .ids
            .get(id)
            .or_else(|| self.ids.get(&join(id, "index")));
        if let Some(&module) = module {
            return self.page(module);
        }
        let folder = format!("{id}/README.md");
        let page = self
            .layout
            .pages
            .iter()
            .position(|page| page.path == folder);
        page.map(Target::Page)
    }
}

/// `path`'s first name, and the rest of it, from the `.`, `#`, or `~`
/// after that.
fn split(path: &str) -> (&str, &str) {
    path.split_at(path.find(['.', '#', '~']).unwrap_or(path.len()))
}

/// `text`, in Markdown, with each `{@link}`, `{@linkcode}`, and
/// `{@linkplain}` that isn't in code as a link to where `resolve` says its
/// target leads, or, if it leads nowhere, as its text alone, and with no
/// `{@inheritDoc}`. A link shows the text after its target, or after a
/// `|`, or before it in brackets, like `[the config]{@link Config}`, or
/// else its target, in code, but for `{@linkplain}`'s.
pub fn render(text: &str, resolve: &dyn Fn(&str) -> Option<String>) -> String {
    if !text.contains("{@") {
        return text.to_string();
    }
    let code = code(text);
    let mut rendered = String::new();
    // Where the text not yet in `rendered` starts.
    let mut done = 0;
    let mut search = 0;
    while let Some(found) = text[search..].find("{@") {
        let start = search + found;
        search = start + 2;
        if code.iter().any(|range| range.contains(&start)) {
            continue;
        }
        let Some(length) = text[start..].find('}') else {
            break;
        };
        let end = start + length + 1;
        let inner = &text[start + 2..end - 1];
        let (tag, content) = inner.split_once(char::is_whitespace).unwrap_or((inner, ""));
        let (from, written) = match tag {
            "inheritDoc" | "inheritdoc" => (start, String::new()),
            "link" | "linkcode" | "linkplain" => {
                let (target, mut shown) = match content.split_once('|') {
                    Some((target, shown)) => (target.trim(), shown.trim()),
                    None => {
                        let content = content.trim();
                        let (target, shown) = content
                            .split_once(char::is_whitespace)
                            .unwrap_or((content, ""));
                        (target, shown.trim())
                    }
                };
                if target.is_empty() {
                    continue;
                }
                let mut from = start;
                let before = &text[done..start];
                if shown.is_empty()
                    && let Some(bracketed) = before.strip_suffix(']')
                    && let Some(open) = bracketed.rfind('[')
                    && !bracketed[open..].contains('\n')
                {
                    from = done + open;
                    shown = &bracketed[open + 1..];
                }
                let shown = match (shown.is_empty(), tag) {
                    (true, "linkplain") => target.to_string(),
                    (true, _) => format!("`{target}`"),
                    (false, "linkcode") => format!("`{shown}`"),
                    (false, _) => shown.to_string(),
                };
                let url = match target.contains("://") || target.starts_with("mailto:") {
                    true => Some(target.to_string()),
                    false => resolve(target),
                };
                let written = match url {
                    Some(url) => format!("[{shown}]({url})"),
                    None => shown,
                };
                (from, written)
            }
            _ => continue,
        };
        rendered.push_str(&text[done..from]);
        rendered.push_str(&written);
        done = end;
        search = end;
    }
    rendered.push_str(&text[done..]);
    rendered
}

/// Where in `text`, in Markdown, there's code: each code span, and each
/// code block.
fn code(text: &str) -> Vec<Range<usize>> {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let parser = Parser::new_ext(text, options).into_offset_iter();
    let ranges = parser.filter_map(|(event, range)| match event {
        Event::Code(_) | Event::Start(Tag::CodeBlock(_)) => Some(range),
        _ => None,
    });
    ranges.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(text: &str) -> String {
        let resolve = |path: &str| match path {
            "Config" => Some("config.md#config".to_string()),
            "load" => Some("#load".to_string()),
            _ => None,
        };
        render(text, &resolve)
    }

    #[test]
    fn links_lead_to_their_targets_and_show_them_in_code() {
        assert_eq!(
            rendered("See {@link Config}."),
            "See [`Config`](config.md#config)."
        );
        assert_eq!(rendered("See {@linkcode load}."), "See [`load`](#load).");
        assert_eq!(rendered("See {@linkplain load}."), "See [load](#load).");
    }

    #[test]
    fn links_show_the_text_after_their_targets_or_in_brackets_before_them() {
        assert_eq!(
            rendered("{@link Config|the config}"),
            "[the config](config.md#config)"
        );
        assert_eq!(
            rendered("{@link Config | the config}"),
            "[the config](config.md#config)"
        );
        assert_eq!(
            rendered("{@link Config the config}"),
            "[the config](config.md#config)"
        );
        assert_eq!(rendered("{@linkcode load loading}"), "[`loading`](#load)");
        assert_eq!(
            rendered("Use [the config]{@link Config}."),
            "Use [the config](config.md#config)."
        );
    }

    #[test]
    fn links_that_lead_nowhere_are_their_text() {
        assert_eq!(rendered("See {@link Missing}."), "See `Missing`.");
        assert_eq!(rendered("See {@link Missing | it}."), "See it.");
        assert_eq!(rendered("See {@linkplain Missing}."), "See Missing.");
    }

    #[test]
    fn links_to_urls_lead_to_them() {
        assert_eq!(
            rendered("{@link https://jsdoc.app JSDoc}"),
            "[JSDoc](https://jsdoc.app)"
        );
        assert_eq!(
            rendered("{@link https://jsdoc.app}"),
            "[`https://jsdoc.app`](https://jsdoc.app)"
        );
    }

    #[test]
    fn links_in_code_and_other_tags_are_left_as_they_are() {
        let text = "Write `{@link Config}`, or\n\n```js\n/** {@link load} */\n```\n";
        assert_eq!(rendered(text), text);
        assert_eq!(
            rendered("{@tutorial intro} and {@link"),
            "{@tutorial intro} and {@link"
        );
        assert_eq!(
            rendered("{@link } {@link Config}"),
            "{@link } [`Config`](config.md#config)"
        );
    }

    #[test]
    fn inherit_doc_is_left_out() {
        assert_eq!(rendered("{@inheritDoc} More."), " More.");
        assert_eq!(rendered("Text"), "Text");
    }
}
