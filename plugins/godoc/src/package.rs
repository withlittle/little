//! A package of a Go module's: what its files declare, put together as
//! go/doc puts a package together, and what its page shows.

use std::collections::{BTreeMap, HashMap, HashSet};

use little_api_reference::model::{Item, Items, Kind, Object, Section};
use little_api_reference::pages::join;

use crate::comment::{self, Context};
use crate::files::Folder;
use crate::reader::{self, File};

/// A package of the module's, as far as its docs go.
#[derive(Debug)]
pub struct Package {
    /// Its import path, like `example.com/m/config`.
    pub import_path: String,
    /// Its name, as its files' package clauses give it, like `config`.
    pub name: String,
    /// Its folder, from the module's, or empty for the module's own.
    pub folder: String,
    /// Its files, each by its path, as the plugin sees it, in order of
    /// their names.
    files: Vec<(String, File)>,
    /// The path of each package its files import, by the name they import
    /// it as, but a name they import more than one package as.
    imports: HashMap<String, String>,
    /// What it declares that a link can name, like `Config`, or
    /// `Config.Load`, for a method or field.
    syms: HashSet<String>,
}

/// The package in `folder`, of the module at `root`, whose path is
/// `module`, from its files, but those every build leaves out, warning of
/// each it can't read; of a folder with files of more than one package,
/// the one with the most, or the first by name of those, warning of the
/// others. None, if no file is left.
pub fn read(
    root: &str,
    module: &str,
    folder: &Folder,
    warnings: &mut Vec<String>,
) -> Option<Package> {
    let dir = match folder.path.as_str() {
        "" => root.to_string(),
        path => join(root, path),
    };
    let mut files = Vec::new();
    for name in &folder.files {
        let path = join(&dir, name);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                warnings.push(format!("can't read `{path}`: {error}"));
                continue;
            }
        };
        if reader::ignored(&text) {
            continue;
        }
        match reader::read(&text) {
            Ok(file) => files.push((path, file)),
            Err(reason) => warnings.push(format!("can't parse `{path}`: {reason}")),
        }
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, file) in &files {
        *counts.entry(&file.package).or_default() += 1;
    }
    let most = *counts.values().max()?;
    let (name, _) = counts.iter().find(|&(_, &count)| count == most)?;
    let name = name.to_string();
    if counts.len() > 1 {
        let names: Vec<String> = counts.keys().map(|name| format!("`{name}`")).collect();
        warnings.push(format!(
            "`{dir}` holds files of more than one package, {}; only `{name}` is documented",
            and(&names)
        ));
    }
    files.retain(|(_, file)| file.package == name);
    let import_path = match folder.path.as_str() {
        "" => module.to_string(),
        path => format!("{module}/{path}"),
    };
    Some(Package::new(import_path, folder.path.clone(), files))
}

/// `items`, with `,` between them, and `and` before the last.
fn and(items: &[String]) -> String {
    match items {
        [one, two] => format!("{one} and {two}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
        [] => String::new(),
    }
}

/// The last element of the import path `path`, or, if it's a major
/// version, like `v2`, the one before it, as the go command names a
/// command it builds.
pub fn short_name(path: &str) -> &str {
    let mut elements = path.rsplit('/');
    let last = elements.next().unwrap_or(path);
    let version = (last.strip_prefix('v')).is_some_and(|number| {
        !number.is_empty()
            && number.bytes().all(|byte| byte.is_ascii_digit())
            && !number.starts_with('0')
            && number != "1"
    });
    match (version, elements.next()) {
        (true, Some(before)) => before,
        _ => last,
    }
}

impl Package {
    /// The package whose path is `import_path`, in `folder`, from `files`,
    /// each of the same package, with what links in its docs can name.
    fn new(import_path: String, folder: String, files: Vec<(String, File)>) -> Package {
        let name = (files.first()).map_or(String::new(), |(_, file)| file.package.clone());
        // Each name a package is imported as, as go/doc's reader takes it.
        let mut imports: HashMap<String, String> = HashMap::new();
        for (name, path) in files.iter().flat_map(|(_, file)| &file.imports) {
            let name = match name.as_deref() {
                Some(".") => continue,
                Some(name) => name.to_string(),
                None => comment::assumed_name(path),
            };
            match imports.get(&name) {
                None => _ = imports.insert(name, path.clone()),
                Some(old) if old != path && !old.is_empty() => {
                    _ = imports.insert(name, String::new())
                }
                Some(_) => {}
            }
        }
        imports.retain(|_, path| !path.is_empty());
        let types: HashSet<&str> = (files.iter())
            .flat_map(|(_, file)| &file.types)
            .map(|ty| ty.name.as_str())
            .collect();
        let mut syms = HashSet::new();
        for (_, file) in &files {
            syms.extend(
                file.values
                    .iter()
                    .flat_map(|value| value.names.iter().cloned()),
            );
            for ty in &file.types {
                syms.insert(ty.name.clone());
                let fields = ty.fields.iter().filter(|field| !field.embedded);
                for member in fields.chain(&ty.methods) {
                    syms.insert(format!("{}.{}", ty.name, member.name));
                }
            }
            for func in &file.funcs {
                match &func.receiver {
                    None => _ = syms.insert(func.name.clone()),
                    Some(receiver) if types.contains(receiver.as_str()) => {
                        syms.insert(format!("{receiver}.{}", func.name));
                    }
                    Some(_) => {}
                }
            }
        }
        Package {
            import_path,
            name,
            folder,
            files,
            imports,
            syms,
        }
    }

    /// What its page shows: its doc comment, and what it exports that's
    /// documented, each type with its methods, and its fields; or, for a
    /// command, its doc comment alone. Of what's declared in more than one
    /// file, for different builds, the first, by the files' names, with a
    /// doc comment. Its links to Go's code are left for
    /// [`comment::render`] to lead.
    pub fn object(&self) -> Object {
        let context = Context {
            import_path: &self.import_path,
            name: &self.name,
            imports: &self.imports,
            syms: &self.syms,
        };
        let docs: Vec<&str> = (self.files.iter())
            .map(|(_, file)| file.doc.as_str())
            .filter(|doc| !doc.is_empty())
            .collect();
        let doc = docs.join("\n");
        if self.name == "main" {
            let mut command =
                documented(short_name(&self.import_path), Kind::Package, &doc, &context);
            command.labels.insert(0, "command".to_string());
            return command;
        }
        let mut package = documented(&self.name, Kind::Package, &doc, &context);
        let mut seen = HashSet::new();
        let mut funcs = Vec::new();
        let mut types: Vec<(&str, &reader::Type)> = Vec::new();
        let mut type_index: HashMap<&str, usize> = HashMap::new();
        for (path, file) in &self.files {
            for ty in &file.types {
                match type_index.get(ty.name.as_str()) {
                    Some(&index) if types[index].1.doc.is_empty() && !ty.doc.is_empty() => {
                        types[index] = (path.as_str(), ty);
                    }
                    Some(_) => {}
                    None => {
                        type_index.insert(&ty.name, types.len());
                        types.push((path.as_str(), ty));
                    }
                }
            }
            for value in file.values.iter().filter(|value| !value.doc.is_empty()) {
                for name in value.names.iter().filter(|&name| seen.insert(name)) {
                    let mut object = documented(name, value.kind, &value.doc, &context);
                    object.signature = Some(value.signature.clone());
                    object.file = Some(path.clone());
                    package.members.push(object);
                }
            }
            let documented_funcs = file.funcs.iter().filter(|func| !func.doc.is_empty());
            for func in documented_funcs.filter(|func| func.receiver.is_none()) {
                if seen.insert(&func.name) {
                    funcs.push(function(path, func, Kind::Function, &context));
                }
            }
        }
        let mut objects: Vec<Object> = (types.iter())
            .map(|&(path, ty)| self.ty(path, ty, &context))
            .collect();
        // Each type's methods are added once every type is known.
        for (path, file) in &self.files {
            for func in file.funcs.iter().filter(|func| !func.doc.is_empty()) {
                let receiver = func.receiver.as_deref();
                let Some(&index) = receiver.and_then(|ty| type_index.get(ty)) else {
                    continue;
                };
                let methods = &mut objects[index].members;
                if !methods.iter().any(|method| method.name == func.name) {
                    methods.push(function(path, func, Kind::Method, &context));
                }
            }
        }
        package.members.extend(funcs);
        let shown = (objects.into_iter()).filter(|ty| ty.is_documented() || !ty.members.is_empty());
        package.members.extend(shown);
        package
    }

    /// The type `ty`, in the file at `path`, with its documented fields, if
    /// it's a struct, and methods, if it's an interface. Go's types are
    /// alike, all `type`, and listed together, as `go doc` lists them.
    fn ty(&self, path: &str, ty: &reader::Type, context: &Context) -> Object {
        let mut object = documented(&ty.name, Kind::Type, &ty.doc, context);
        object.signature = Some(ty.signature.clone());
        object.source = ty.source.clone();
        object.file = Some(path.to_string());
        let fields: Vec<Item> = (ty.fields.iter())
            .filter(|field| !field.doc.is_empty())
            .map(|field| Item {
                name: Some(field.name.clone()),
                ty: Some(field.ty.clone()),
                description: text(comment::sections(&field.doc, context)),
                ..Item::default()
            })
            .collect();
        if !fields.is_empty() {
            object.docs.push(Section::Items {
                kind: Items::Fields,
                items: fields,
            });
        }
        for method in ty.methods.iter().filter(|method| !method.doc.is_empty()) {
            let mut object_method = documented(&method.name, Kind::Method, &method.doc, context);
            object_method.signature = Some(method.ty.clone());
            object.members.push(object_method);
        }
        object
    }
}

/// An object called `name`, of `kind`, with the doc comment `doc`,
/// labelled `deprecated` if it says it is.
fn documented(name: &str, kind: Kind, doc: &str, context: &Context) -> Object {
    let mut object = Object::new(name, kind);
    object.docs = comment::sections(doc, context);
    if comment::deprecated(doc) {
        object.labels.push("deprecated".to_string());
    }
    object
}

/// The function or method `func`, in the file at `path`, as `kind`.
fn function(path: &str, func: &reader::Func, kind: Kind, context: &Context) -> Object {
    let mut object = documented(&func.name, kind, &func.doc, context);
    object.signature = Some(func.signature.clone());
    object.source = (func.source != func.signature).then(|| func.source.clone());
    object.file = Some(path.to_string());
    object
}

/// The text of `sections`, as one, for a cell of a table.
fn text(sections: Vec<Section>) -> String {
    let texts = sections.into_iter().filter_map(|section| match section {
        Section::Text { text, .. } => Some(text),
        Section::Items { .. } => None,
    });
    texts.collect::<Vec<_>>().join("\n\n")
}

#[cfg(test)]
mod tests {
    use little_api_reference::testing::folder_with;

    use super::*;

    /// The package at `import_path`, from `files`, each its name and text.
    fn package(import_path: &str, files: &[(&str, &str)]) -> Package {
        let files = (files.iter())
            .map(|(name, text)| (name.to_string(), reader::read(text).unwrap()))
            .collect();
        Package::new(import_path.to_string(), String::new(), files)
    }

    /// The names of `object`'s members, each with those of its own.
    fn names(object: &Object) -> Vec<String> {
        let mut names = Vec::new();
        for member in &object.members {
            names.push(member.name.clone());
            for inner in &member.members {
                names.push(format!("{}.{}", member.name, inner.name));
            }
        }
        names
    }

    fn texts(object: &Object) -> Vec<&str> {
        let texts = object.docs.iter().filter_map(|section| match section {
            Section::Text { text, .. } => Some(text.as_str()),
            Section::Items { .. } => None,
        });
        texts.collect()
    }

    #[test]
    fn the_last_element_of_a_path_names_a_command_but_a_major_version() {
        assert_eq!(short_name("example.com/m/cmd/tool"), "tool");
        assert_eq!(short_name("example.com/tool/v2"), "tool");
        assert_eq!(short_name("example.com/tool/v10"), "tool");
        assert_eq!(short_name("example.com/tool/v1"), "v1");
        assert_eq!(short_name("example.com/tool/v0"), "v0");
        assert_eq!(short_name("example.com/tool/v02"), "v02");
        assert_eq!(short_name("example.com/tool/vx"), "vx");
        assert_eq!(short_name("gopkg.in/yaml.v3"), "yaml.v3");
        assert_eq!(short_name("v2"), "v2");
        assert_eq!(short_name("tool"), "tool");
    }

    #[test]
    fn items_are_joined_with_commas_and_and() {
        let items = |items: &[&str]| {
            items
                .iter()
                .map(|item| item.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(and(&items(&["a", "b"])), "a and b");
        assert_eq!(and(&items(&["a", "b", "c"])), "a, b, and c");
    }

    #[test]
    fn imports_are_named_as_go_doc_names_them_and_ambiguous_ones_left_out() {
        let package = package(
            "example.com/m",
            &[
                (
                    "a.go",
                    "package m\n\nimport (\n\t\"fmt\"\n\tyaml \"gopkg.in/yaml.v3\"\n\t. \"strings\"\n\t\"example.com/go-thing/v2\"\n\tr \"math/rand\"\n)\n",
                ),
                (
                    "b.go",
                    "package m\n\nimport (\n\t\"fmt\"\n\tr \"crypto/rand\"\n)\n",
                ),
            ],
        );
        let mut imports: Vec<(&str, &str)> = (package.imports.iter())
            .map(|(name, path)| (name.as_str(), path.as_str()))
            .collect();
        imports.sort();
        assert_eq!(
            imports,
            [
                ("fmt", "fmt"),
                ("thing", "example.com/go-thing/v2"),
                ("yaml", "gopkg.in/yaml.v3"),
            ]
        );
    }

    #[test]
    fn links_can_name_what_a_package_declares_and_exports() {
        let package = package(
            "example.com/m",
            &[(
                "a.go",
                "package m

const A, b = 1, 2

type T struct {
\tF int
\tEmbedded
\tg int
}

type Embedded struct{}

type I interface {
\tM()
}

func (t T) Method() {}

func (u *unknown) Other() {}

func New() T { return T{} }
",
            )],
        );
        let mut syms: Vec<&str> = package.syms.iter().map(String::as_str).collect();
        syms.sort();
        assert_eq!(
            syms,
            ["A", "Embedded", "I", "I.M", "New", "T", "T.F", "T.Method"]
        );
    }

    #[test]
    fn a_page_shows_whats_documented_each_type_with_its_methods() {
        let object = package(
            "example.com/m/shapes",
            &[
                (
                    "a.go",
                    "// Package shapes draws [Square] shapes.
package shapes

// Sides is how many sides a [Square] has.
const Sides = 4

const Undocumented = 5

// A Square is a shape.
type Square struct {
\t// Size is its size.
\tSize int
\tcolour int
}

// Area is its area.
func (s Square) Area() int { return s.Size * s.Size }

func (s Square) Undocumented() {}

// New makes a square.
func New(size int) Square { return Square{size, 0} }

// Draw draws it.
//
// Deprecated: Use [Square.Area].
func Draw(s Square)

type hidden int

// Size is its size.
func (h hidden) Size() int { return int(h) }

type Plain int

// Shape is anything drawn.
type Shape interface {
\t// Area is its area.
\tArea() int
\tPerimeter() int
}
",
                ),
                (
                    "b.go",
                    "// More of its docs.
package shapes

// New makes a square, again.
func New(size int) Square { return Square{size, 1} }

// Square is a square, again.
type Square int
",
                ),
            ],
        )
        .object();
        assert_eq!(object.name, "shapes");
        assert_eq!(object.kind, Kind::Package);
        let (start, end) = ('\u{E000}', '\u{E002}');
        let expected = format!(
            "Package shapes draws {start}example.com/m/shapes#Square\u{E001}`Square`{end} \
             shapes.\n\nMore of its docs."
        );
        assert_eq!(texts(&object), [expected.as_str()]);
        assert_eq!(
            names(&object),
            [
                "Sides",
                "New",
                "Draw",
                "Square",
                "Square.Area",
                "Shape",
                "Shape.Area"
            ]
        );
        let new = &object.members[1];
        assert_eq!(new.signature.as_deref(), Some("func New(size int) Square"));
        assert_eq!(
            new.source.as_deref(),
            Some("func New(size int) Square { return Square{size, 0} }")
        );
        assert_eq!(new.file.as_deref(), Some("a.go"));
        let draw = &object.members[2];
        assert_eq!(draw.labels, ["deprecated"]);
        assert_eq!(draw.source, None);
        let square = &object.members[3];
        assert_eq!(square.kind, Kind::Type);
        assert_eq!(
            square.signature.as_deref(),
            Some(
                "type Square struct {\n\tSize int\n\t// contains filtered or unexported fields\n}"
            )
        );
        let fields = Section::Items {
            kind: Items::Fields,
            items: vec![Item {
                name: Some("Size".to_string()),
                ty: Some("int".to_string()),
                description: "Size is its size.".to_string(),
                ..Item::default()
            }],
        };
        assert_eq!(square.docs[1], fields);
        let shape = &object.members[4];
        assert_eq!(shape.kind, Kind::Type);
        assert_eq!(shape.members[0].kind, Kind::Method);
        assert_eq!(shape.members[0].signature.as_deref(), Some("Area() int"));
        assert_eq!(
            object.members[0].signature.as_deref(),
            Some("const Sides = 4")
        );
    }

    #[test]
    fn of_what_builds_declare_alike_the_first_documented_is_shown() {
        let object = package(
            "example.com/m",
            &[
                ("a_linux.go", "package m\n\nfunc Open() {}\n\ntype File int\n"),
                (
                    "a_windows.go",
                    "package m\n\n// Open opens.\nfunc Open() {}\n\n// A File is a file.\ntype File int\n\n// Name is its name.\nfunc (File) Name() {}\n",
                ),
                (
                    "b_darwin.go",
                    "package m\n\n// Open opens, on macOS.\nfunc Open() {}\n\n// Name is its name, on macOS.\nfunc (File) Name() {}\n",
                ),
            ],
        )
        .object();
        assert_eq!(names(&object), ["Open", "File", "File.Name"]);
        assert_eq!(texts(&object.members[0]), ["Open opens."]);
        assert_eq!(texts(&object.members[1]), ["A File is a file."]);
        assert_eq!(texts(&object.members[1].members[0]), ["Name is its name."]);
    }

    #[test]
    fn a_command_shows_only_its_doc_comment() {
        let source = "// Tool does things.\npackage main\n\n// Run runs.\nfunc Run() {}\n";
        let object = package("example.com/tool/v2", &[("main.go", source)]).object();
        assert_eq!(object.name, "tool");
        assert_eq!(object.labels, ["command"]);
        assert_eq!(texts(&object), ["Tool does things."]);
        assert!(object.members.is_empty());
        let object = package("example.com/tool", &[("main.go", "package main\n")]).object();
        assert!(!object.is_documented());
    }

    #[test]
    fn a_folder_of_two_packages_shows_the_one_with_more_files() {
        let files = [
            ("a.go", "package a\n"),
            ("b.go", "package a\n"),
            ("broken.go", "package a\n\nfunc {\n"),
            ("ignored.go", "//go:build ignore\n\npackage main\n"),
            ("tools.go", "//go:build tools\n\npackage tools\n"),
        ];
        let root = folder_with("little-godoc-package", "two", &files);
        let folder = Folder {
            path: String::new(),
            files: files.iter().map(|(name, _)| name.to_string()).collect(),
        };
        let mut warnings = Vec::new();
        let package = read(&root, "example.com/a", &folder, &mut warnings).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(package.name, "a");
        assert_eq!(package.import_path, "example.com/a");
        let paths: Vec<&str> = package
            .files
            .iter()
            .map(|(path, _)| path.as_str())
            .collect();
        assert_eq!(paths, [format!("{root}/a.go"), format!("{root}/b.go")]);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].starts_with(&format!("can't parse `{root}/broken.go`: ")));
        assert_eq!(
            warnings[1],
            format!(
                "`{root}` holds files of more than one package, `a` and `tools`; only `a` is \
                 documented"
            )
        );
    }
}
