//! A crate's modules, read from its files as rustc finds them, and what the
//! names in each lead to, for the links in docs and the types of `impl`s.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use little_api_reference::pages::join;
use syn::ext::IdentExt;
use syn::{Attribute, Expr, ExprLit, Ident, Item, ItemMacro, ItemMod, ItemUse, Lit, Meta, UseTree};

/// A module of a crate, or the crate itself.
pub struct Module {
    /// Its name, or, for the crate, the crate's.
    pub name: String,
    /// The names of the modules it's in, after the crate, and its own, like
    /// `["site", "versions"]`; empty for the crate.
    pub path: Vec<String>,
    /// The file its code is in, as the plugin sees it, like `src/site.rs`.
    pub file: String,
    /// What that file holds, whose bytes its items' spans count.
    pub source: Rc<str>,
    /// Its attributes: those on its `mod`, then its own, like its `//!`s.
    pub attrs: Vec<Attribute>,
    /// Its items, but the modules it declares.
    pub items: Vec<Item>,
    /// Where the module it's in is in the list, or none for the crate.
    pub parent: Option<usize>,
    /// Where the modules it declares are in the list, in the order it
    /// declares them.
    pub children: Vec<usize>,
}

/// The modules of the crate called `krate`, whose root is the `lib.rs` or
/// `main.rs` in `folder`: the crate first, then each module before the ones
/// it declares. A module that can't be read is left out, and warned of.
pub fn read(folder: &str, krate: &str, warnings: &mut Vec<String>) -> Vec<Module> {
    let mut reader = Reader {
        modules: Vec::new(),
        warnings,
    };
    let roots = ["lib.rs", "main.rs"].map(|name| join(folder, name));
    let Some(root) = roots.into_iter().find(|file| is_file(file)) else {
        reader.warnings.push(format!(
            "`{folder}` holds no `lib.rs` or `main.rs`, so there's no crate in it to document"
        ));
        return Vec::new();
    };
    if let Some((source, parsed)) = reader.parse(&root) {
        let (items, mods) = split(parsed.items);
        let module = Module {
            name: krate.to_string(),
            path: Vec::new(),
            file: root.clone(),
            source,
            attrs: parsed.attrs,
            items,
            parent: None,
            children: Vec::new(),
        };
        let place = Place {
            file: root,
            children: folder.to_string(),
            inline: false,
        };
        reader.add(module, mods, &place);
    }
    reader.modules
}

/// Where a module's code is, and where the files of the modules it declares
/// are.
struct Place {
    /// The file its code is in.
    file: String,
    /// The folder the files of the modules it declares are in, as rustc
    /// finds them.
    children: String,
    /// Whether its code is a `mod m { ... }` in that file.
    inline: bool,
}

/// The modules read so far, and what's warned of.
struct Reader<'a> {
    modules: Vec<Module>,
    warnings: &'a mut Vec<String>,
}

impl Reader<'_> {
    /// Add `module`, whose code is at `place`, and the modules it declares,
    /// `mods`, and give back where it is in the list.
    fn add(&mut self, module: Module, mods: Vec<ItemMod>, place: &Place) -> usize {
        let index = self.modules.len();
        let source = module.source.clone();
        self.modules.push(module);
        for item in mods {
            if let Some(child) = self.child(index, item, place, &source) {
                self.modules[index].children.push(child);
            }
        }
        index
    }

    /// Add the module `item` declares in the module at `parent`, whose code
    /// is at `place`, in `source`, and give back where it is in the list;
    /// or none if it's for tests, hidden, declared already, as it is for
    /// each platform, or can't be read, which is warned of.
    fn child(
        &mut self,
        parent: usize,
        item: ItemMod,
        place: &Place,
        source: &Rc<str>,
    ) -> Option<usize> {
        let name = item.ident.unraw().to_string();
        let children = &self.modules[parent].children;
        let declared = (children.iter()).any(|&child| self.modules[child].name == name);
        if declared || is_test(&item.attrs) || is_hidden(&item.attrs) {
            return None;
        }
        let (attrs, items, place, source) = match item.content {
            Some((_, items)) => {
                let place = Place {
                    file: place.file.clone(),
                    children: join(&place.children, &name),
                    inline: true,
                };
                (item.attrs, items, place, source.clone())
            }
            None => {
                let file = self.find(&name, &item.attrs, place)?;
                let (source, parsed) = self.parse(&file)?;
                // rustc finds the modules a `#[path]` file declares beside it,
                // as it does those a `mod.rs` declares.
                let children = match path_attr(&item.attrs) {
                    Some(_) => dir(&file),
                    None => join(&place.children, &name),
                };
                let mut attrs = item.attrs;
                attrs.extend(parsed.attrs);
                if is_test(&attrs) || is_hidden(&attrs) {
                    return None;
                }
                let place = Place {
                    file,
                    children,
                    inline: false,
                };
                (attrs, parsed.items, place, source)
            }
        };
        let mut path = self.modules[parent].path.clone();
        path.push(name.clone());
        let (items, mods) = split(items);
        let module = Module {
            name,
            path,
            file: place.file.clone(),
            source,
            attrs,
            items,
            parent: Some(parent),
            children: Vec::new(),
        };
        Some(self.add(module, mods, &place))
    }

    /// The file of the module called `name`, declared with `attrs` in the
    /// module at `place`, or none, warning why, if it has none.
    fn find(&mut self, name: &str, attrs: &[Attribute], place: &Place) -> Option<String> {
        if let Some(path) = path_attr(attrs) {
            let folder = match place.inline {
                true => place.children.clone(),
                false => dir(&place.file),
            };
            return Some(join(&folder, &path));
        }
        let flat = join(&place.children, &format!("{name}.rs"));
        let nested = join(&place.children, &format!("{name}/mod.rs"));
        if is_file(&flat) {
            return Some(flat);
        }
        if is_file(&nested) {
            return Some(nested);
        }
        self.warnings.push(format!(
            "can't find module `{name}`, which `{}` declares: there's no `{flat}` or `{nested}`",
            place.file
        ));
        None
    }

    /// The code in `file`, and what it holds, or none, warning why, if it
    /// can't be read or parsed.
    fn parse(&mut self, file: &str) -> Option<(Rc<str>, syn::File)> {
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(error) => {
                self.warnings.push(format!("can't read `{file}`: {error}"));
                return None;
            }
        };
        // syn skips a byte order mark and a `#!` line, and its spans count
        // bytes from after them.
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        match syn::parse_file(text) {
            Ok(parsed) => {
                let skipped = parsed.shebang.as_ref().map_or(0, String::len);
                Some((Rc::from(&text[skipped..]), parsed))
            }
            Err(error) => {
                let line = error.span().start().line;
                (self.warnings).push(format!("can't parse `{file}`: {error} on line {line}"));
                None
            }
        }
    }
}

/// `items`, and, apart from them, the modules they declare.
fn split(items: Vec<Item>) -> (Vec<Item>, Vec<ItemMod>) {
    let mut own = Vec::new();
    let mut mods = Vec::new();
    for item in items {
        match item {
            Item::Mod(item) => mods.push(item),
            item => own.push(item),
        }
    }
    (own, mods)
}

/// Whether `attrs` has `#[cfg(test)]`, so only tests build what it's on.
pub fn is_test(attrs: &[Attribute]) -> bool {
    has(attrs, "cfg", "test")
}

/// Whether `attrs` has `#[doc(hidden)]`.
pub fn is_hidden(attrs: &[Attribute]) -> bool {
    has(attrs, "doc", "hidden")
}

/// Whether `attrs` has `#[name(arg)]`.
fn has(attrs: &[Attribute], name: &str, arg: &str) -> bool {
    (attrs.iter()).any(|attr| {
        attr.path().is_ident(name) && attr.parse_args::<Ident>().is_ok_and(|ident| ident == arg)
    })
}

/// The string `attrs` sets with `#[name = "..."]`, if it does.
pub fn string(attrs: &[Attribute], name: &str) -> Option<String> {
    (attrs.iter())
        .filter(|attr| attr.path().is_ident(name))
        .find_map(|attr| match &attr.meta {
            Meta::NameValue(meta) => match &meta.value {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(string),
                    ..
                }) => Some(string.value()),
                _ => None,
            },
            _ => None,
        })
}

/// The path `attrs` gives a module's file with `#[path = "..."]`, if they
/// do.
fn path_attr(attrs: &[Attribute]) -> Option<String> {
    string(attrs, "path")
}

/// Whether `item` is a `macro_rules!` with `#[macro_export]`, so it's in the
/// crate's own module, whichever module defines it.
pub fn is_exported(item: &ItemMacro) -> bool {
    let exported = (item.attrs.iter()).any(|attr| attr.path().is_ident("macro_export"));
    exported && item.mac.path.is_ident("macro_rules") && item.ident.is_some()
}

/// The name of what `item` defines, if it's a name a doc can link to, but a
/// module's or a macro's.
pub fn name(item: &Item) -> Option<String> {
    let ident = match item {
        Item::Const(item) => &item.ident,
        Item::Enum(item) => &item.ident,
        Item::Fn(item) => &item.sig.ident,
        Item::Static(item) => &item.ident,
        Item::Struct(item) => &item.ident,
        Item::Trait(item) => &item.ident,
        Item::Type(item) => &item.ident,
        Item::Union(item) => &item.ident,
        _ => return None,
    };
    Some(ident.unraw().to_string()).filter(|name| name != "_")
}

/// A name a `use` brings into its module, and the path it leads to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Use {
    /// The name, or none for a `*`, which brings in each name in its path.
    pub name: Option<String>,
    /// The path, like `["crate", "config", "Config"]`.
    pub path: Vec<String>,
}

/// The names `item` brings in, but `_`, and those from `::`, which is
/// another crate.
pub fn uses(item: &ItemUse) -> Vec<Use> {
    let mut uses = Vec::new();
    if item.leading_colon.is_none() {
        flatten(&item.tree, &mut Vec::new(), &mut uses);
    }
    uses
}

/// Add the names `tree`, after `prefix`, brings in to `uses`.
fn flatten(tree: &UseTree, prefix: &mut Vec<String>, uses: &mut Vec<Use>) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.unraw().to_string());
            flatten(&path.tree, prefix, uses);
            prefix.pop();
        }
        UseTree::Name(name) => uses.extend(named(prefix, &name.ident, &name.ident)),
        UseTree::Rename(rename) => uses.extend(named(prefix, &rename.ident, &rename.rename)),
        UseTree::Glob(_) => uses.push(Use {
            name: None,
            path: prefix.clone(),
        }),
        UseTree::Group(group) => {
            for tree in &group.items {
                flatten(tree, prefix, uses);
            }
        }
    }
}

/// What `ident`, after `prefix`, leads to, brought in as `name`, but for
/// `_`: `self` is the last name in `prefix`.
fn named(prefix: &[String], ident: &Ident, name: &Ident) -> Option<Use> {
    let mut path = prefix.to_vec();
    if ident != "self" {
        path.push(ident.unraw().to_string());
    }
    let name = match name == "self" {
        true => prefix.last()?.clone(),
        false => name.unraw().to_string(),
    };
    (name != "_").then_some(Use {
        name: Some(name),
        path,
    })
}

/// The folder `file` is in.
fn dir(file: &str) -> String {
    (file.rsplit_once('/')).map_or_else(String::new, |(folder, _)| folder.to_string())
}

fn is_file(path: &str) -> bool {
    Path::new(path).is_file()
}

/// What a path leads to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// The module at this place in the list.
    Module(usize),
    /// The item called `name` in the module at `module`, or its `member`,
    /// like a method or a variant.
    Item {
        module: usize,
        name: String,
        member: Option<String>,
    },
}

/// The names a module can use.
#[derive(Default)]
struct Scope {
    /// Where the modules it declares are in the list, by name.
    modules: HashMap<String, usize>,
    /// The names of its items.
    items: HashSet<String>,
    /// What each name it `use`s leads to.
    uses: HashMap<String, Vec<String>>,
    /// The paths it `use`s each name in, with `*`.
    globs: Vec<Vec<String>>,
}

/// What the names in a crate's modules lead to.
pub struct Scopes {
    krate: String,
    parents: Vec<Option<usize>>,
    scopes: Vec<Scope>,
}

/// What each name has led to in a module, so far, as a path is followed;
/// none for one being looked up, so a `use` that leads back to itself
/// leads nowhere.
type Memo = HashMap<(usize, String), Option<Target>>;

impl Scopes {
    /// The names in `modules`, of the crate called `krate`.
    pub fn new(krate: &str, modules: &[Module]) -> Scopes {
        let mut scopes: Vec<Scope> = modules.iter().map(|_| Scope::default()).collect();
        for (index, module) in modules.iter().enumerate() {
            for &child in &module.children {
                let name = modules[child].name.clone();
                scopes[index].modules.insert(name, child);
            }
            for item in &module.items {
                if let Some(name) = name(item) {
                    scopes[index].items.insert(name);
                }
                match item {
                    Item::Use(item) => {
                        for Use { name, path } in uses(item) {
                            match name {
                                Some(name) => {
                                    scopes[index].uses.entry(name).or_insert(path);
                                }
                                None => scopes[index].globs.push(path),
                            }
                        }
                    }
                    Item::Macro(item) if is_exported(item) => {
                        let name = item.ident.as_ref().expect("it's exported").to_string();
                        scopes[0].items.insert(name.clone());
                        let path = vec!["crate".to_string(), name.clone()];
                        scopes[index].uses.entry(name).or_insert(path);
                    }
                    _ => {}
                }
            }
        }
        Scopes {
            krate: krate.to_string(),
            parents: modules.iter().map(|module| module.parent).collect(),
            scopes,
        }
    }

    /// What `path` leads to in the module at `module`, where `Self` is
    /// `this`, if it leads anywhere in the crate.
    pub fn resolve(&self, module: usize, this: Option<&Target>, path: &[String]) -> Option<Target> {
        self.walk(module, this, path, &mut Memo::new())
    }

    fn walk(
        &self,
        module: usize,
        this: Option<&Target>,
        path: &[String],
        memo: &mut Memo,
    ) -> Option<Target> {
        let (first, rest) = path.split_first()?;
        let mut target = match first.as_str() {
            "crate" => Target::Module(0),
            "self" => Target::Module(module),
            "super" => Target::Module(self.parents[module]?),
            "Self" => this?.clone(),
            name => match self.lookup(module, name, memo) {
                Some(target) => target,
                None if *name == self.krate => Target::Module(0),
                None => return None,
            },
        };
        for segment in rest {
            target = match target {
                Target::Module(module) if segment == "super" => {
                    Target::Module(self.parents[module]?)
                }
                Target::Module(module) => self.lookup(module, segment, memo)?,
                Target::Item {
                    module,
                    name,
                    member: None,
                } => Target::Item {
                    module,
                    name,
                    member: Some(segment.clone()),
                },
                Target::Item { .. } => return None,
            };
        }
        Some(target)
    }

    /// What `name` leads to in the module at `module`: a module it declares,
    /// an item it defines, or a name it `use`s.
    fn lookup(&self, module: usize, name: &str, memo: &mut Memo) -> Option<Target> {
        let key = (module, name.to_string());
        if let Some(target) = memo.get(&key) {
            return target.clone();
        }
        memo.insert(key.clone(), None);
        let scope = &self.scopes[module];
        let target = if let Some(&child) = scope.modules.get(name) {
            Some(Target::Module(child))
        } else if scope.items.contains(name) {
            Some(Target::Item {
                module,
                name: name.to_string(),
                member: None,
            })
        } else if let Some(path) = scope.uses.get(name) {
            self.walk(module, None, path, memo)
        } else {
            (scope.globs.iter()).find_map(|glob| match self.walk(module, None, glob, memo)? {
                Target::Module(glob) => self.lookup(glob, name, memo),
                Target::Item { .. } => None,
            })
        };
        memo.insert(key, target.clone());
        target
    }
}
