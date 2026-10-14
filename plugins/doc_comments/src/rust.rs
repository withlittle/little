//! What a crate documents, read from its modules' items as rustdoc shows
//! them: an object for each module, with its `pub` items that have docs,
//! their signatures and code, the fields and variants they list, the traits
//! they implement, and the methods of their `impl`s.

use std::collections::HashMap;

use little_api_reference::model::{self, Items, Kind, Object, Section};
use proc_macro2::TokenTree;
use syn::ext::IdentExt;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Attribute, Block, Expr, Field, Fields, FieldsNamed, FnModifiers, ImplItem, ImplItemConst, Item,
    ItemConst, ItemEnum, ItemFn, ItemImpl, ItemMacro, ItemStruct, ItemTrait, ItemUnion, Path,
    Signature, Token, TraitItem, Type, TypePath, UnOp, Visibility,
};

use crate::docs;
use crate::modules::{self, Module, Scopes, Target, Use, is_hidden, is_test};

/// Where the paths in an object's docs are read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    /// Where the module they're in is in the crate's list.
    pub module: usize,
    /// What `Self` is there, if anything.
    pub this: Option<Target>,
}

/// What a crate documents.
pub struct Documented {
    /// The object of each of its modules, in the order of the modules.
    pub objects: Vec<Object>,
    /// Where each object's docs are read from, by where the module whose
    /// page it's on is in the list, and the name its heading shows, like
    /// `Config::load`, or `""` for the module's own.
    pub contexts: HashMap<(usize, String), Context>,
    /// What each module re-exports, with `pub use`.
    pub re_exports: Vec<Vec<Use>>,
}

/// What `modules`, whose names `scopes` knows, document. A `#[doc]` that
/// can't be read is warned of.
pub fn document(modules: &[Module], scopes: &Scopes, warnings: &mut Vec<String>) -> Documented {
    let objects = (modules.iter().enumerate())
        .map(|(index, module)| {
            let kind = if index == 0 {
                Kind::Crate
            } else {
                Kind::Module
            };
            Object::new(&module.name, kind)
        })
        .collect();
    let mut reader = Reader {
        modules,
        scopes,
        warnings,
        objects,
        contexts: HashMap::new(),
        re_exports: modules.iter().map(|_| Vec::new()).collect(),
        types: HashMap::new(),
        implements: HashMap::new(),
    };
    for index in 0..modules.len() {
        reader.module(index);
    }
    // Every type is known before the `impl`s are read, wherever they are.
    for (index, module) in modules.iter().enumerate() {
        for item in &module.items {
            if let Item::Impl(item) = item {
                reader.implementation(index, item);
            }
        }
    }
    reader.finish()
}

/// What's been documented so far.
struct Reader<'a> {
    modules: &'a [Module],
    scopes: &'a Scopes,
    warnings: &'a mut Vec<String>,
    objects: Vec<Object>,
    contexts: HashMap<(usize, String), Context>,
    re_exports: Vec<Vec<Use>>,
    /// Where each struct, enum, and union is in its module's object's
    /// members, by where its module is in the list, and its name.
    types: HashMap<(usize, String), usize>,
    /// The traits each type implements, by where its module is in the
    /// list, and its name, as its docs list them.
    implements: HashMap<(usize, String), Vec<String>>,
}

impl Reader<'_> {
    /// Document the module at `index`, and the items in it.
    fn module(&mut self, index: usize) {
        let modules = self.modules;
        let module = &modules[index];
        let text = docs::text(&module.attrs, &module.file, self.warnings);
        self.objects[index].docs = untitled(text);
        let context = Context {
            module: index,
            this: None,
        };
        self.contexts.insert((index, String::new()), context);
        for item in &module.items {
            match item {
                Item::Use(item) if is_pub(&item.vis) && !is_skipped(&item.attrs) => {
                    self.re_exports[index].extend(modules::uses(item));
                }
                Item::Macro(item) if modules::is_exported(item) => self.exported(index, item),
                item => self.item(index, item),
            }
        }
    }

    /// Document `item`, in the module at `index`, if it's `pub`, has docs,
    /// and is the first with its name there.
    fn item(&mut self, index: usize, item: &Item) {
        let modules = self.modules;
        let module = &modules[index];
        let (attrs, vis) = match item {
            Item::Const(item) => (&item.attrs, &item.vis),
            Item::Enum(item) => (&item.attrs, &item.vis),
            Item::Fn(item) => (&item.attrs, &item.vis),
            Item::Static(item) => (&item.attrs, &item.vis),
            Item::Struct(item) => (&item.attrs, &item.vis),
            Item::Trait(item) => (&item.attrs, &item.vis),
            Item::Type(item) => (&item.attrs, &item.vis),
            Item::Union(item) => (&item.attrs, &item.vis),
            _ => return,
        };
        let Some(name) = modules::name(item) else {
            return;
        };
        let taken = (self.objects[index].members.iter()).any(|member| member.name == name);
        if !is_pub(vis) || is_skipped(attrs) || taken {
            return;
        }
        let text = docs::text(attrs, &module.file, self.warnings);
        if text.is_empty() {
            return;
        }
        let (kind, signature) = match item {
            Item::Const(item) => (Kind::Constant, constant(item.clone())),
            Item::Enum(item) => (Kind::Enum, enumeration(item)),
            Item::Fn(item) => (Kind::Function, function(&item.vis, &item.sig)),
            Item::Static(item) => {
                let mut item = item.clone();
                item.attrs.clear();
                *item.expr = value(&item.expr);
                (Kind::Static, print(Item::Static(item)))
            }
            Item::Struct(item) => (Kind::Struct, structure(item)),
            Item::Trait(item) => (Kind::Trait, signature_of_trait(item)),
            Item::Type(item) => {
                let mut item = item.clone();
                item.attrs.clear();
                (Kind::TypeAlias, print(Item::Type(item)))
            }
            Item::Union(item) => (Kind::Union, union(item)),
            _ => return,
        };
        let mut object = Object::new(&name, kind);
        object.signature = Some(signature);
        object.docs = untitled(text);
        let file = &module.file;
        let listed = match item {
            Item::Struct(item) => self.fields(item.fields.iter(), file),
            Item::Union(item) => self.fields(item.fields.named.iter(), file),
            Item::Enum(item) => self.variants(item, file),
            _ => None,
        };
        object.docs.extend(listed);
        object.source = source(&module.source, item, attrs_of);
        let this = Target::Item {
            module: index,
            name: name.clone(),
            member: None,
        };
        if let Item::Trait(item) = item {
            object.members = self.trait_members(index, &name, item, &this);
        }
        if matches!(item, Item::Struct(_) | Item::Enum(_) | Item::Union(_)) {
            let at = self.objects[index].members.len();
            self.types.insert((index, name.clone()), at);
            let derives = derives(attrs);
            if !derives.is_empty() {
                self.implements.insert((index, name.clone()), derives);
            }
        }
        let context = Context {
            module: index,
            this: Some(this),
        };
        self.contexts.insert((index, name), context);
        self.objects[index].members.push(object);
    }

    /// Document `item`, a `#[macro_export]`ed `macro_rules!` in the module
    /// at `index`, on the crate's page, where it's exported, if it has docs
    /// and is the first there with its name.
    fn exported(&mut self, index: usize, item: &ItemMacro) {
        let modules = self.modules;
        let module = &modules[index];
        let name = (item.ident.as_ref())
            .expect("it's exported")
            .unraw()
            .to_string();
        let taken = (self.objects[0].members.iter()).any(|member| member.name == name);
        if is_skipped(&item.attrs) || taken {
            return;
        }
        let text = docs::text(&item.attrs, &module.file, self.warnings);
        if text.is_empty() {
            return;
        }
        let mut object = Object::new(&name, Kind::Macro);
        object.signature = Some(rules(&name, item, &module.source));
        object.docs = untitled(text);
        object.source = source(&module.source, item, macro_attrs);
        if module.file != modules[0].file {
            object.file = Some(module.file.clone());
        }
        let context = Context {
            module: index,
            this: None,
        };
        self.contexts.insert((0, name), context);
        self.objects[0].members.push(object);
    }

    /// The members of `item`, the trait called `name` in the module at
    /// `index`, which is `this`, that have docs.
    fn trait_members(
        &mut self,
        index: usize,
        name: &str,
        item: &ItemTrait,
        this: &Target,
    ) -> Vec<Object> {
        let modules = self.modules;
        let module = &modules[index];
        let mut members: Vec<Object> = Vec::new();
        for member in &item.items {
            let (attrs, ident) = match member {
                TraitItem::Fn(member) => (&member.attrs, &member.sig.ident),
                TraitItem::Const(member) => (&member.attrs, &member.ident),
                TraitItem::Type(member) => (&member.attrs, &member.ident),
                _ => continue,
            };
            let member_name = ident.unraw().to_string();
            let taken = members.iter().any(|member| member.name == member_name);
            if is_skipped(attrs) || taken {
                continue;
            }
            let text = docs::text(attrs, &module.file, self.warnings);
            if text.is_empty() {
                continue;
            }
            let (kind, signature) = match member {
                TraitItem::Fn(member) => {
                    let kind = match member.sig.receiver() {
                        Some(_) => Kind::Method,
                        None => Kind::Function,
                    };
                    (kind, function(&Visibility::Inherited, &member.sig))
                }
                TraitItem::Const(_) => (Kind::AssociatedConstant, in_trait(member)),
                _ => (Kind::AssociatedType, in_trait(member)),
            };
            let mut object = Object::new(&member_name, kind);
            object.signature = Some(signature);
            object.docs = untitled(text);
            object.source = source(&module.source, member, trait_item_attrs);
            let context = Context {
                module: index,
                this: Some(this.clone()),
            };
            (self.contexts).insert((index, format!("{name}::{member_name}")), context);
            members.push(object);
        }
        members
    }

    /// The section listing `fields`, in `file`, that are `pub`, shown, and
    /// have docs, if any are; a tuple's are named by their places, like
    /// `0`.
    fn fields<'f>(
        &mut self,
        fields: impl Iterator<Item = &'f Field>,
        file: &str,
    ) -> Option<Section> {
        let mut items = Vec::new();
        for (place, field) in fields.enumerate() {
            if !is_shown(field) {
                continue;
            }
            let description = docs::text(&field.attrs, file, self.warnings);
            if description.is_empty() {
                continue;
            }
            let name = match &field.ident {
                Some(ident) => ident.unraw().to_string(),
                None => place.to_string(),
            };
            items.push(model::Item {
                name: Some(name),
                ty: Some(type_text(&field.ty)),
                description,
                ..model::Item::default()
            });
        }
        (!items.is_empty()).then_some(Section::Items {
            kind: Items::Fields,
            items,
        })
    }

    /// The section listing the variants of `item`, in `file`, that are shown
    /// and have docs, if any are.
    fn variants(&mut self, item: &ItemEnum, file: &str) -> Option<Section> {
        let mut items = Vec::new();
        for variant in item.variants.iter() {
            if is_skipped(&variant.attrs) {
                continue;
            }
            let description = docs::text(&variant.attrs, file, self.warnings);
            if description.is_empty() {
                continue;
            }
            items.push(model::Item {
                name: Some(variant.ident.unraw().to_string()),
                description,
                ..model::Item::default()
            });
        }
        (!items.is_empty()).then_some(Section::Items {
            kind: Items::Variants,
            items,
        })
    }

    /// Document what `item`, an `impl` in the module at `index`, gives a
    /// documented type: the trait it implements, or its `pub` functions and
    /// constants that have docs.
    fn implementation(&mut self, index: usize, item: &ItemImpl) {
        let Type::Path(TypePath {
            qself: None, path, ..
        }) = &*item.self_ty
        else {
            return;
        };
        if item.modifiers.polarity.is_some() || is_skipped(&item.attrs) {
            return;
        }
        let Some(Target::Item {
            module,
            name,
            member: None,
        }) = self.scopes.resolve(index, None, &names(path))
        else {
            return;
        };
        let Some(&at) = self.types.get(&(module, name.clone())) else {
            return;
        };
        let Some((path, _)) = &item.trait_ else {
            let this = Target::Item {
                module,
                name,
                member: None,
            };
            for member in &item.items {
                self.member(index, &this, at, member);
            }
            return;
        };
        let written = code(&type_text(&path_type(&last(path))));
        let implemented = match self.scopes.resolve(index, None, &names(path)) {
            Some(Target::Item {
                module,
                name,
                member: None,
            }) => format!("[{written}]({})", self.absolute(module, &name)),
            _ => written,
        };
        let implements = self.implements.entry((module, name)).or_default();
        if !implements.contains(&implemented) {
            implements.push(implemented);
        }
    }

    /// Document `member`, of an `impl` in the module at `index` for `this`,
    /// the type at `at` in its module's object's members, if it's `pub`,
    /// has docs, and is the type's first with its name.
    fn member(&mut self, index: usize, this: &Target, at: usize, member: &ImplItem) {
        let Target::Item {
            module: page,
            name: ty,
            ..
        } = this
        else {
            return;
        };
        let modules = self.modules;
        let module = &modules[index];
        let (attrs, vis, ident) = match member {
            ImplItem::Fn(member) => (&member.attrs, &member.vis, &member.sig.ident),
            ImplItem::Const(member) => (&member.attrs, &member.vis, &member.ident),
            _ => return,
        };
        let name = ident.unraw().to_string();
        let members = &self.objects[*page].members[at].members;
        let taken = members.iter().any(|member| member.name == name);
        if !is_pub(vis) || is_skipped(attrs) || taken {
            return;
        }
        let text = docs::text(attrs, &module.file, self.warnings);
        if text.is_empty() {
            return;
        }
        let (kind, signature) = match member {
            ImplItem::Fn(member) => {
                let kind = match member.sig.receiver() {
                    Some(_) => Kind::Method,
                    None => Kind::Function,
                };
                (kind, function(&member.vis, &member.sig))
            }
            ImplItem::Const(member) => (Kind::AssociatedConstant, constant(item_const(member))),
            _ => return,
        };
        let mut object = Object::new(&name, kind);
        object.signature = Some(signature);
        object.docs = untitled(text);
        object.source = source(&module.source, member, impl_item_attrs);
        if module.file != modules[*page].file {
            object.file = Some(module.file.clone());
        }
        let context = Context {
            module: index,
            this: Some(this.clone()),
        };
        self.contexts
            .insert((*page, format!("{ty}::{name}")), context);
        self.objects[*page].members[at].members.push(object);
    }

    /// The path from the crate's root to the item called `name` in the
    /// module at `module`, like `crate::config::Config`.
    fn absolute(&self, module: usize, name: &str) -> String {
        let path = &self.modules[module].path;
        let names = std::iter::once("crate")
            .chain(path.iter().map(String::as_str))
            .chain([name]);
        names.collect::<Vec<_>>().join("::")
    }

    /// What's been documented, with the traits each type implements listed
    /// last in its docs.
    fn finish(mut self) -> Documented {
        for ((module, name), implements) in self.implements {
            let at = self.types[&(module, name)];
            self.objects[module].members[at].docs.push(Section::Text {
                title: Some("Implements".to_string()),
                text: implements.join(", "),
            });
        }
        Documented {
            objects: self.objects,
            contexts: self.contexts,
            re_exports: self.re_exports,
        }
    }
}

/// Whether `vis` is `pub`, so what it's on is in the crate's API.
fn is_pub(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

/// Whether what has `attrs` is left out: built only for tests, or hidden.
fn is_skipped(attrs: &[Attribute]) -> bool {
    is_test(attrs) || is_hidden(attrs)
}

/// Whether `field` is shown: `pub`, and not left out.
fn is_shown(field: &Field) -> bool {
    is_pub(&field.vis) && !is_skipped(&field.attrs)
}

/// The sections of docs whose text is `text`: none if it's empty.
fn untitled(text: String) -> Vec<Section> {
    match text.is_empty() {
        true => Vec::new(),
        false => vec![Section::Text { title: None, text }],
    }
}

/// The names in `path`, without their generics; none if it starts with
/// `::`, in another crate.
fn names(path: &Path) -> Vec<String> {
    if path.leading_colon.is_some() {
        return Vec::new();
    }
    let segments = path.segments.iter();
    segments
        .map(|segment| segment.ident.unraw().to_string())
        .collect()
}

/// `text` in code.
fn code(text: &str) -> String {
    format!("`{text}`")
}

/// `item` as prettyplease writes it.
fn print(item: Item) -> String {
    let mut file = syn::parse_file("").expect("nothing parses");
    file.items.push(item);
    prettyplease::unparse(&file).trim_end().to_string()
}

/// A block with nothing in it, `{}`.
fn empty() -> Block {
    Block {
        brace_token: Default::default(),
        stmts: Vec::new(),
    }
}

/// `_`, as a type.
fn infer() -> Type {
    syn::parse_str("_").expect("`_` parses")
}

/// The signature of the function `sig` is of, which `vis` is on, like
/// `pub fn load() -> Config`.
fn function(vis: &Visibility, sig: &Signature) -> String {
    let item = ItemFn {
        attrs: Vec::new(),
        vis: vis.clone(),
        modifiers: FnModifiers::default(),
        sig: sig.clone(),
        block: Box::new(empty()),
    };
    let printed = print(Item::Fn(item));
    printed
        .strip_suffix("{}")
        .unwrap_or(&printed)
        .trim_end()
        .to_string()
}

/// The signature of `item`, a constant, with the value rustdoc shows.
fn constant(mut item: ItemConst) -> String {
    item.attrs.clear();
    *item.expr = value(&item.expr);
    print(Item::Const(item))
}

/// `member`, a constant of an `impl`, as a constant on its own.
fn item_const(member: &ImplItemConst) -> ItemConst {
    let member = member.clone();
    ItemConst {
        attrs: member.attrs,
        vis: member.vis,
        modifiers: member.modifiers,
        const_token: member.const_token,
        ident: member.ident,
        generics: member.generics,
        colon_token: member.colon_token,
        ty: Box::new(member.ty),
        eq_token: member.eq_token,
        expr: Box::new(member.expr),
        semi_token: member.semi_token,
    }
}

/// `expr`, a constant's value, as rustdoc shows it: as it's written if it's
/// a literal, perhaps negative, or a path, else `_`.
fn value(expr: &Expr) -> Expr {
    let shown = match expr {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Unary(unary) => {
            matches!(unary.op, UnOp::Neg(_)) && matches!(*unary.expr, Expr::Lit(_))
        }
        _ => false,
    };
    match shown {
        true => expr.clone(),
        false => syn::parse_str("_").expect("`_` parses"),
    }
}

/// The signature of `item`, a struct, with only its fields that are shown,
/// and `_` for each other in a tuple.
fn structure(item: &ItemStruct) -> String {
    let mut item = item.clone();
    item.attrs.clear();
    let left_out = match &mut item.fields {
        Fields::Named(fields) => keep(fields),
        Fields::Unnamed(fields) => {
            for field in fields.unnamed.iter_mut() {
                if !is_shown(field) {
                    field.vis = Visibility::Inherited;
                    field.ty = infer();
                }
                field.attrs.clear();
            }
            false
        }
        Fields::Unit => false,
    };
    let printed = print(Item::Struct(item));
    match left_out {
        true => private(&printed),
        false => printed,
    }
}

/// The signature of `item`, a union, with only its fields that are shown.
fn union(item: &ItemUnion) -> String {
    let mut item = item.clone();
    item.attrs.clear();
    let left_out = keep(&mut item.fields);
    let printed = print(Item::Union(item));
    match left_out {
        true => private(&printed),
        false => printed,
    }
}

/// Keep only the `fields` that are shown, without their attributes, and
/// give back whether any are left out.
fn keep(fields: &mut FieldsNamed) -> bool {
    let all = fields.named.len();
    fields.named = (std::mem::take(&mut fields.named).into_iter())
        .filter(is_shown)
        .map(|mut field| {
            field.attrs.clear();
            field
        })
        .collect();
    fields.named.len() < all
}

/// `code`, the signature of a struct or union, with a comment where the
/// fields it leaves out are, as rustdoc writes it.
fn private(code: &str) -> String {
    if let Some(start) = code.strip_suffix("{}") {
        return format!("{start}{{ /* private fields */ }}");
    }
    match code.rfind("\n}") {
        Some(end) => format!("{}\n    /* private fields */{}", &code[..end], &code[end..]),
        None => code.to_string(),
    }
}

/// The signature of `item`, an enum, with only its variants that are shown.
fn enumeration(item: &ItemEnum) -> String {
    let mut item = item.clone();
    item.attrs.clear();
    item.variants = (std::mem::take(&mut item.variants).into_iter())
        .filter(|variant| !is_skipped(&variant.attrs))
        .map(|mut variant| {
            variant.attrs.clear();
            for field in variant.fields.iter_mut() {
                field.attrs.clear();
            }
            variant
        })
        .collect();
    print(Item::Enum(item))
}

/// The signature of `item`, a trait, with only its members that are shown,
/// and `{ ... }` for each default body, as rustdoc writes it.
fn signature_of_trait(item: &ItemTrait) -> String {
    let mut item = item.clone();
    item.attrs.clear();
    item.items = item.items.iter().filter_map(bare).collect();
    let printed = print(Item::Trait(item));
    (printed.lines())
        .map(|line| match line.strip_suffix("{}") {
            Some(start) if line.starts_with(' ') => format!("{start}{{ ... }}"),
            _ => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `member`, of a trait, as its signature shows it, without attributes,
/// with an empty default body, and a default value as rustdoc shows it; or
/// none if it's left out, or isn't a function, constant, or type.
fn bare(member: &TraitItem) -> Option<TraitItem> {
    match member.clone() {
        TraitItem::Fn(mut member) if !is_skipped(&member.attrs) => {
            member.attrs.clear();
            if member.default.is_some() {
                member.default = Some(empty());
                member.semi_token = None;
            }
            Some(TraitItem::Fn(member))
        }
        TraitItem::Const(mut member) if !is_skipped(&member.attrs) => {
            member.attrs.clear();
            if let Some((_, expr)) = &mut member.default {
                *expr = value(expr);
            }
            Some(TraitItem::Const(member))
        }
        TraitItem::Type(mut member) if !is_skipped(&member.attrs) => {
            member.attrs.clear();
            Some(TraitItem::Type(member))
        }
        _ => None,
    }
}

/// The signature of `member`, a constant or type of a trait, as the trait's
/// shows it.
fn in_trait(member: &TraitItem) -> String {
    let mut item: ItemTrait = syn::parse_str("trait T {}").expect("a trait parses");
    item.items
        .push(bare(member).unwrap_or_else(|| member.clone()));
    let printed = print(Item::Trait(item));
    let lines: Vec<&str> = printed.lines().collect();
    let inside = match lines.as_slice() {
        [_, inside @ .., _] if !inside.is_empty() => inside,
        _ => return printed.clone(),
    };
    (inside.iter())
        .map(|line| line.strip_prefix("    ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The signature of `item`, the `macro_rules!` called `name`, in `source`:
/// each rule's matcher, as it's written, on one line, and `{ ... }` for what
/// it expands to, as rustdoc writes it.
fn rules(name: &str, item: &ItemMacro, source: &str) -> String {
    let mut out = format!("macro_rules! {name} {{\n");
    let mut matcher: Option<String> = None;
    for tree in item.mac.tokens.clone() {
        match tree {
            TokenTree::Punct(punct) if punct.as_char() == ';' => {
                if let Some(matcher) = matcher.take() {
                    out += &format!("    {matcher} => {{ ... }};\n");
                }
            }
            TokenTree::Group(group) if matcher.is_none() => {
                let written = (source.get(group.span().byte_range()))
                    .map_or_else(|| group.to_string(), String::from);
                matcher = Some(written.split_whitespace().collect::<Vec<_>>().join(" "));
            }
            _ => {}
        }
    }
    if let Some(matcher) = matcher {
        out += &format!("    {matcher} => {{ ... }};\n");
    }
    out + "}"
}

/// The traits `attrs` derive, each in code, by its own name, like
/// `` `Serialize` `` for `serde::Serialize`.
fn derives(attrs: &[Attribute]) -> Vec<String> {
    let mut derives = Vec::new();
    for attr in (attrs.iter()).filter(|attr| attr.path().is_ident("derive")) {
        let paths = attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated);
        for path in paths.iter().flatten() {
            derives.push(code(&type_text(&path_type(&last(path)))));
        }
    }
    derives
}

/// The last segment of `path`, with its generics, as rustdoc names a
/// trait, like `From<String>` for `std::convert::From<String>`.
fn last(path: &Path) -> Path {
    match path.segments.last() {
        Some(segment) => Path::from(segment.clone()),
        None => path.clone(),
    }
}

/// `path` as a type.
fn path_type(path: &Path) -> Type {
    Type::Path(TypePath {
        attrs: Vec::new(),
        qself: None,
        path: path.clone(),
    })
}

/// `ty` as prettyplease writes it, on one line.
fn type_text(ty: &Type) -> String {
    let Ok(Item::Type(mut alias)) = syn::parse_str::<Item>("type T = _;") else {
        unreachable!("a type alias parses");
    };
    *alias.ty = ty.clone();
    let printed = print(Item::Type(alias));
    let written = (printed.strip_prefix("type T = "))
        .and_then(|written| written.strip_suffix(';'))
        .unwrap_or(&printed);
    let joined = written.lines().map(str::trim).collect::<Vec<_>>().join(" ");
    (joined.replace("< ", "<"))
        .replace("( ", "(")
        .replace("[ ", "[")
        .replace(", >", ">")
        .replace(", )", ")")
        .replace(", ]", "]")
}

/// The code `node` is written as in `source`, without its docs: from the
/// start of its first line, without that line's indentation. `attrs` gives
/// the attributes of a node, if it has any.
fn source<T: Clone + Spanned>(
    source: &str,
    node: &T,
    attrs: fn(&mut T) -> Option<&mut Vec<Attribute>>,
) -> Option<String> {
    let mut node = node.clone();
    if let Some(attrs) = attrs(&mut node) {
        attrs.retain(|attr| !attr.path().is_ident("doc"));
    }
    let range = node.span().byte_range();
    let start = (source.get(..range.start)?.rfind('\n')).map_or(0, |newline| newline + 1);
    let code = source.get(start..range.end)?;
    let whitespace = |line: &str| line.len() - line.trim_start_matches([' ', '\t']).len();
    let margin = code.lines().next().map_or(0, whitespace);
    let lines: Vec<&str> = (code.lines())
        .map(|line| &line[margin.min(whitespace(line))..])
        .collect();
    Some(lines.join("\n"))
}

fn attrs_of(item: &mut Item) -> Option<&mut Vec<Attribute>> {
    match item {
        Item::Const(item) => Some(&mut item.attrs),
        Item::Enum(item) => Some(&mut item.attrs),
        Item::Fn(item) => Some(&mut item.attrs),
        Item::Static(item) => Some(&mut item.attrs),
        Item::Struct(item) => Some(&mut item.attrs),
        Item::Trait(item) => Some(&mut item.attrs),
        Item::Type(item) => Some(&mut item.attrs),
        Item::Union(item) => Some(&mut item.attrs),
        _ => None,
    }
}

fn trait_item_attrs(member: &mut TraitItem) -> Option<&mut Vec<Attribute>> {
    match member {
        TraitItem::Fn(member) => Some(&mut member.attrs),
        TraitItem::Const(member) => Some(&mut member.attrs),
        TraitItem::Type(member) => Some(&mut member.attrs),
        _ => None,
    }
}

fn impl_item_attrs(member: &mut ImplItem) -> Option<&mut Vec<Attribute>> {
    match member {
        ImplItem::Fn(member) => Some(&mut member.attrs),
        ImplItem::Const(member) => Some(&mut member.attrs),
        _ => None,
    }
}

fn macro_attrs(item: &mut ItemMacro) -> Option<&mut Vec<Attribute>> {
    Some(&mut item.attrs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse<T: syn::parse::Parse>(code: &str) -> T {
        syn::parse_str(code).unwrap()
    }

    #[test]
    fn a_struct_shows_only_its_pub_fields() {
        let item: ItemStruct = parse(
            "/// A config.\n#[derive(Clone)]\npub struct Config {\n    /// Its source.\n    \
             pub source: PathBuf,\n    secret: u8,\n    #[doc(hidden)]\n    pub hidden: u8,\n}",
        );
        assert_eq!(
            structure(&item),
            "pub struct Config {\n    pub source: PathBuf,\n    /* private fields */\n}"
        );
        let item: ItemStruct =
            parse("pub struct Secret<T>\nwhere\n    T: Copy,\n{\n    key: T,\n}");
        assert_eq!(
            structure(&item),
            "pub struct Secret<T>\nwhere\n    T: Copy,\n{ /* private fields */ }"
        );
        let item: ItemStruct = parse("pub struct Px(pub u32, u8);");
        assert_eq!(structure(&item), "pub struct Px(pub u32, _);");
        let item: ItemUnion = parse("pub union Bits {\n    pub int: u32,\n    float: f32,\n}");
        assert_eq!(
            union(&item),
            "pub union Bits {\n    pub int: u32,\n    /* private fields */\n}"
        );
    }

    #[test]
    fn an_enum_shows_its_variants_but_hidden_ones() {
        let item: ItemEnum = parse(
            "pub enum Kind {\n    /// A site.\n    Site,\n    #[doc(hidden)]\n    Hidden,\n    \
             Number(#[allow(unused)] u8),\n}",
        );
        assert_eq!(
            enumeration(&item),
            "pub enum Kind {\n    Site,\n    Number(u8),\n}"
        );
    }

    #[test]
    fn a_trait_shows_its_members_without_their_bodies() {
        let item: ItemTrait = parse(
            "pub trait Plugin: Sized {\n    /// Its name.\n    const NAME: &'static str = \
             \"plugin\";\n    const SIZE: usize = 1 + 1;\n    /// What it gives.\n    type Output;\n    \
             fn render(&self) -> Self::Output;\n    /// Set it up.\n    fn setup(&mut self) {\n        \
             println!(\"set up\");\n    }\n    #[doc(hidden)]\n    fn secret(&self) {}\n}",
        );
        assert_eq!(
            signature_of_trait(&item),
            "pub trait Plugin: Sized {\n    const NAME: &'static str = \"plugin\";\n    const SIZE: \
             usize = _;\n    type Output;\n    fn render(&self) -> Self::Output;\n    fn setup(&mut \
             self) { ... }\n}"
        );
        assert_eq!(
            in_trait(&item.items[0]),
            "const NAME: &'static str = \"plugin\";"
        );
        assert_eq!(in_trait(&item.items[2]), "type Output;");
    }

    #[test]
    fn a_function_is_shown_without_its_body() {
        let item: ItemFn = parse(
            "/// Load it.\npub async fn load<T>(path: &Path) -> Result<T>\nwhere\n    T: Read,\n{\n    \
             todo!()\n}",
        );
        assert_eq!(
            function(&item.vis, &item.sig),
            "pub async fn load<T>(path: &Path) -> Result<T>\nwhere\n    T: Read,"
        );
        let item: ItemFn = parse("pub const unsafe fn f(&self) {}");
        assert_eq!(
            function(&item.vis, &item.sig),
            "pub const unsafe fn f(&self)"
        );
    }

    #[test]
    fn a_constant_shows_its_value_if_its_simple() {
        let shown = |code: &str| constant(parse(code));
        assert_eq!(
            shown("pub const MAX: usize = 1 << 10;"),
            "pub const MAX: usize = _;"
        );
        assert_eq!(
            shown("pub const MIN: i32 = -1;"),
            "pub const MIN: i32 = -1;"
        );
        assert_eq!(
            shown("pub const NAME: &str = \"x\";"),
            "pub const NAME: &str = \"x\";"
        );
        assert_eq!(
            shown("pub const ONE: u8 = u8::MIN;"),
            "pub const ONE: u8 = u8::MIN;"
        );
    }

    #[test]
    fn a_macro_shows_its_matchers() {
        let code = "#[macro_export]\nmacro_rules! hello {\n    () => { println!(\"hello\") };\n    \
                    ($name:expr) => {\n        println!(\"hello {}\", $name)\n    };\n    \
                    ($name:expr,\n     $($rest:tt)*) => {}\n}";
        let item: ItemMacro = parse(code);
        assert_eq!(
            rules("hello", &item, code),
            "macro_rules! hello {\n    () => { ... };\n    ($name:expr) => { ... };\n    \
             ($name:expr, $($rest:tt)*) => { ... };\n}"
        );
    }

    #[test]
    fn derives_and_types_are_written_as_code() {
        let item: ItemStruct = parse("#[derive(Clone, serde::Serialize)]\n#[serde(x)]\nstruct S;");
        assert_eq!(derives(&item.attrs), ["`Clone`", "`Serialize`"]);
        let ty: Type = parse("Vec < HashMap<String,u8> >");
        assert_eq!(type_text(&ty), "Vec<HashMap<String, u8>>");
    }

    #[test]
    fn source_starts_at_its_line_without_its_docs() {
        let code = "// Ünïcödé — before it.\n/// Its docs.\n#[derive(Clone)]\npub struct Café {\n    \
                    pub naïve: bool,\n}\n";
        let item: Item = parse(code);
        assert_eq!(
            source(code, &item, attrs_of).as_deref(),
            Some("#[derive(Clone)]\npub struct Café {\n    pub naïve: bool,\n}")
        );
        let code = "impl Café {\n    /// Docs.\n    pub fn f(&self) {\n        g();\n    }\n}";
        let item: ItemImpl = parse(code);
        assert_eq!(
            source(code, &item.items[0], impl_item_attrs).as_deref(),
            Some("pub fn f(&self) {\n    g();\n}")
        );
    }
}
