//! What a Go file declares, as far as its docs go: its package and its doc
//! comment, what it imports, and what it exports, each with its doc
//! comment and its code, as Go's go/doc reads them.

use std::ops::Deref;

use gosyn::ast::{self, Declaration, Expression};
use gosyn::token::{Keyword, LitKind, Operator, Token};
use little_api_reference::model::Kind;
use unic_ucd_category::GeneralCategory;

use crate::source::{Group, Source};

/// A Go file, as far as its docs go.
#[derive(Debug, Default)]
pub struct File {
    /// The name of its package.
    pub package: String,
    /// The package's doc comment in it, as Go gives its text, or nothing.
    pub doc: String,
    /// The paths it imports, each with the name it imports it as, if any.
    pub imports: Vec<(Option<String>, String)>,
    pub types: Vec<Type>,
    /// Its functions and methods, of any type.
    pub funcs: Vec<Func>,
    pub values: Vec<Value>,
}

/// A type a file exports.
#[derive(Debug)]
pub struct Type {
    pub name: String,
    pub doc: String,
    /// Its declaration, with only the fields or methods it exports.
    pub signature: String,
    /// Its declaration as it's written, if that's other than its signature.
    pub source: Option<String>,
    /// The fields a struct exports.
    pub fields: Vec<Field>,
    /// The methods an interface exports.
    pub methods: Vec<Field>,
}

/// A field a struct exports, or a method an interface does.
#[derive(Debug, PartialEq)]
pub struct Field {
    pub name: String,
    /// A field's type, or a method's signature.
    pub ty: String,
    pub doc: String,
    /// Whether it's an embedded field, named for its type.
    pub embedded: bool,
}

/// A function or method a file exports.
#[derive(Debug)]
pub struct Func {
    pub name: String,
    /// The type a method's receiver is, without a `*` or type parameters.
    pub receiver: Option<String>,
    pub doc: String,
    /// From `func` to before its body.
    pub signature: String,
    /// All of it.
    pub source: String,
}

/// The constants or variables a file exports in one spec, like `A, B = 1, 2`.
#[derive(Debug)]
pub struct Value {
    /// The names it exports, of those it declares.
    pub names: Vec<String>,
    /// `Constant` or `Variable`.
    pub kind: Kind,
    pub doc: String,
    pub signature: String,
}

/// The types Go declares itself, which an interface can embed.
const PREDECLARED: &[&str] = &[
    "any",
    "bool",
    "byte",
    "comparable",
    "complex64",
    "complex128",
    "error",
    "float32",
    "float64",
    "int",
    "int8",
    "int16",
    "int32",
    "int64",
    "rune",
    "string",
    "uint",
    "uint8",
    "uint16",
    "uint32",
    "uint64",
    "uintptr",
];

/// Whether a file is left out of every build, as `//go:build ignore`, or
/// `// +build ignore`, in the comments before its package clause, leave it.
pub fn ignored(text: &str) -> bool {
    for line in text.lines().map(str::trim) {
        if line == "//go:build ignore" || line == "// +build ignore" {
            return true;
        }
        if !line.is_empty() && !line.starts_with("//") {
            break;
        }
    }
    false
}

/// Reads a Go file's `text`, or says why it can't, like
/// `unexpected '}' on line 3`.
pub fn read(text: &str) -> Result<File, String> {
    let file = gosyn(text, gosyn::parse_source)?;
    let tokens = gosyn(text, gosyn::tokenize_source)?;
    let mut reader = Reader {
        source: Source::new(text, tokens),
        file: File::default(),
    };
    match reader.read(&file) {
        Some(()) => Ok(reader.file),
        None => Err("its tokens don't match its declarations".to_string()),
    }
}

/// What gosyn's `read` makes of `text`, or what it found wrong there, and
/// on what line.
fn gosyn<'a, T, E>(text: &'a str, read: fn(&'a str) -> Result<T, E>) -> Result<T, String>
where
    E: Deref<Target = dyn std::error::Error + Send + Sync>,
{
    let error = match read(text) {
        Ok(value) => return Ok(value),
        Err(error) => error,
    };
    let reason = match error.downcast_ref::<gosyn::Error>() {
        Some(gosyn::Error::UnexpectedToken {
            actual: Some(token),
            ..
        }) => {
            format!("unexpected {token:?}")
        }
        Some(gosyn::Error::UnexpectedToken { actual: None, .. }) => {
            "unexpected end of file".to_string()
        }
        Some(gosyn::Error::Else { reason, .. }) => reason.clone(),
        _ => return Err(error.to_string()),
    };
    // gosyn's lines are wrong once it's backed up over one and read it
    // again, but it finds the same error, in the same words, in each start
    // of `text` that reaches it, and the shortest ends on its line.
    let error = error.to_string();
    let ends: Vec<usize> = text
        .char_indices()
        .map(|(at, c)| at + c.len_utf8())
        .collect();
    let shortest = ends.partition_point(|&end| {
        read(&text[..end])
            .err()
            .is_none_or(|other| other.to_string() != error)
    });
    let start = &text[..ends.get(shortest).copied().unwrap_or(text.len())];
    let line = start
        .strip_suffix('\n')
        .unwrap_or(start)
        .matches('\n')
        .count()
        + 1;
    Err(format!("{reason} on line {line}"))
}

/// Reads a file's declarations from their elements in its tokens, each
/// from its first token to its last; each read returns `None` where its
/// tokens don't match its declaration.
struct Reader<'a> {
    source: Source<'a>,
    file: File,
}

impl Reader<'_> {
    fn read(&mut self, file: &ast::File) -> Option<()> {
        let elements = self.source.elements();
        let (&(package, _), rest) = elements.split_first()?;
        self.keyword(package, Keyword::Package)?;
        let declarations: Vec<_> = (rest.iter())
            .skip_while(|&&(first, _)| self.keyword(first, Keyword::Import).is_some())
            .collect();
        if declarations.len() != file.decl.len() {
            return None;
        }
        self.file.package = file.pkg_name.name.clone();
        self.file.doc = self.text([self.source.doc(package)]);
        self.file.imports = (file.imports.iter())
            .map(|import| {
                let name = import.name.as_ref().map(|name| name.name.clone());
                (name, unquote(&import.path.value))
            })
            .collect();
        for (&&element, declaration) in declarations.iter().zip(&file.decl) {
            match declaration {
                Declaration::Function(func) => self.func(element, func)?,
                Declaration::Type(decl) => self.types(element, decl)?,
                Declaration::Const(decl) => {
                    let specs = decl.specs.iter().map(|spec| Spec {
                        names: &spec.name,
                        typed: spec.typ.is_some(),
                        valued: !spec.values.is_empty(),
                        conversion: match &spec.values[..] {
                            [value] => conversion(value),
                            _ => None,
                        },
                    });
                    self.values(element, Kind::Constant, decl.pos1, specs.collect())?;
                }
                Declaration::Variable(decl) => {
                    let specs = decl.specs.iter().map(|spec| Spec {
                        names: &spec.name,
                        typed: spec.typ.is_some(),
                        valued: !spec.values.is_empty(),
                        conversion: None,
                    });
                    self.values(element, Kind::Variable, decl.pos1, specs.collect())?;
                }
            }
        }
        Some(())
    }

    /// Reads a function or method, from `func` to the end of its body.
    fn func(&mut self, (first, last): (usize, usize), func: &ast::FuncDecl) -> Option<()> {
        self.keyword(first, Keyword::Func)?;
        if !exported(&func.name.name) {
            return Some(());
        }
        let receiver = match &func.recv {
            None => None,
            Some(receiver) => match receiver.list.first().map(|field| base_type(&field.typ)) {
                Some(Some((name, false))) => Some(name.to_string()),
                // A receiver go/doc can't read, which it leaves out.
                _ => return Some(()),
            },
        };
        let signature = match &func.body {
            Some(body) => {
                let open = self.bracket(body.pos.0, Operator::BraceLeft)?;
                self.before(first, open)?
            }
            None => last,
        };
        self.file.funcs.push(Func {
            name: func.name.name.clone(),
            receiver,
            doc: self.text([self.source.doc(first)]),
            signature: self.source.code(first, signature),
            source: self.source.code(first, last),
        });
        Some(())
    }

    /// Reads the types a `type` declaration declares, alone or in a group.
    fn types(
        &mut self,
        (first, last): (usize, usize),
        decl: &ast::Decl<ast::TypeSpec>,
    ) -> Option<()> {
        self.keyword(first, Keyword::Type)?;
        let doc = self.source.doc(first);
        let Some((open, close)) = decl.pos1 else {
            let [spec] = &decl.specs[..] else {
                return None;
            };
            let name = *self.source.significant(first, last).get(1)?;
            return self.type_spec((first, name, last), spec, false, [None, doc]);
        };
        let open = self.bracket(open, Operator::ParenLeft)?;
        let close = self.bracket(close, Operator::ParenRight)?;
        let elements = self.source.elements_in(open, close);
        if elements.len() != decl.specs.len() {
            return None;
        }
        for (&(first, last), spec) in elements.iter().zip(&decl.specs) {
            let own = self.source.doc(first);
            self.type_spec((first, first, last), spec, true, [own, doc])?;
        }
        Some(())
    }

    /// Reads a type from `start`, `type` or its name in a group, its name
    /// at `name`, and its last token, `last`, with the first doc comment of
    /// `docs` that has text.
    fn type_spec(
        &mut self,
        (start, name, last): (usize, usize, usize),
        spec: &ast::TypeSpec,
        grouped: bool,
        docs: [Option<Group>; 2],
    ) -> Option<()> {
        if self.ident(name)? != spec.name.name {
            return None;
        }
        if !exported(&spec.name.name) {
            return Some(());
        }
        let filtered = match &spec.typ {
            Expression::TypeStruct(typ) => self.filter(start, typ.pos, &typ.fields, false)?,
            Expression::TypeInterface(typ) => {
                self.filter(start, typ.methods.pos?, &typ.methods.list, true)?
            }
            _ => None,
        };
        let (signature, members) =
            filtered.unwrap_or_else(|| (self.source.code(start, last), Vec::new()));
        let prefix = if grouped { "type " } else { "" };
        let signature = format!("{prefix}{signature}");
        let code = format!("{prefix}{}", self.source.code(start, last));
        let (fields, methods) = match &spec.typ {
            Expression::TypeInterface(_) => (Vec::new(), members),
            _ => (members, Vec::new()),
        };
        self.file.types.push(Type {
            name: spec.name.name.clone(),
            doc: self.text(docs),
            source: (code != signature).then_some(code),
            signature,
            fields,
            methods,
        });
        Some(())
    }

    /// A struct's or interface's declaration, from `start` to the closing
    /// brace of `braces`, with only the fields or methods it exports, on a
    /// line each, as go/doc filters them, and a comment where it leaves any
    /// out; with those it exports. `None` within, for one whose fields
    /// don't match their tokens, which is shown as it's written.
    #[allow(clippy::type_complexity)]
    fn filter(
        &self,
        start: usize,
        (open, close): (usize, usize),
        fields: &[ast::Field],
        interface: bool,
    ) -> Option<Option<(String, Vec<Field>)>> {
        let source = &self.source;
        let open = self.bracket(open, Operator::BraceLeft)?;
        let close = self.bracket(close, Operator::BraceRight)?;
        let elements = source.elements_in(open, close);
        if elements.len() != fields.len() {
            return Some(None);
        }
        let margin = source.indent(start).unwrap_or("");
        let relative = |i: usize| {
            source
                .indent(i)
                .map(|indent| indent.strip_prefix(margin).unwrap_or(indent))
        };
        let header = source.code(start, open);
        let mut lines = vec![format!("{} {{", header.trim_end_matches('{').trim_end())];
        let (mut members, mut removed) = (Vec::new(), false);
        for (&(first, last), field) in elements.iter().zip(fields) {
            let indent = relative(first).unwrap_or("\t");
            let doc = self.text([source.doc(first), source.line_comment(last)]);
            let names: Vec<&str> = (field.name.iter())
                .map(|name| name.name.as_str())
                .filter(|name| exported(name))
                .collect();
            if field.name.is_empty() {
                let base = base_type(&field.typ);
                let kept = match base {
                    Some((name, _)) => exported(name) || interface && PREDECLARED.contains(&name),
                    None => interface,
                };
                if !kept {
                    removed = true;
                    continue;
                }
                if let (Some((name, _)), false) = (base, interface) {
                    members.push(Field {
                        name: name.to_string(),
                        ty: source.code(first, self.typed((first, last), field)?),
                        doc,
                        embedded: true,
                    });
                }
                lines.push(format!(
                    "{indent}{}",
                    source.code_within(first, last, margin)
                ));
                continue;
            }
            if names.is_empty() {
                removed = true;
                continue;
            }
            let ty = *source
                .significant(first, last)
                .get(2 * field.name.len() - 1)?;
            if names.len() < field.name.len() {
                removed = true;
                let code = source.code_within(ty, last, margin);
                lines.push(format!("{indent}{} {code}", names.join(", ")));
            } else {
                lines.push(format!(
                    "{indent}{}",
                    source.code_within(first, last, margin)
                ));
            }
            let ty = match interface {
                true => source.code(first, last),
                false => source.code(ty, self.typed((first, last), field)?),
            };
            for name in names {
                members.push(Field {
                    name: name.to_string(),
                    ty: ty.clone(),
                    doc: doc.clone(),
                    embedded: false,
                });
            }
        }
        let line = |i: usize| source.line(source.tokens[i].start);
        if !removed && line(open) == line(close) {
            return Some(Some((source.code(start, close), members)));
        }
        if removed {
            let what = if interface { "methods" } else { "fields" };
            lines.push(format!("\t// contains filtered or unexported {what}"));
        }
        lines.push(format!("{}}}", relative(close).unwrap_or("")));
        Some(Some((lines.join("\n"), members)))
    }

    /// The last token of the field from `first` to `last`'s type, before
    /// its tag, if it has one.
    fn typed(&self, (first, last): (usize, usize), field: &ast::Field) -> Option<usize> {
        match &field.tag {
            Some(tag) => self.before(first, self.source.index(tag.pos)),
            None => Some(last),
        }
    }

    /// Reads the constants or variables a declaration declares, alone or
    /// in a group, each spec with its own doc comment, else its line
    /// comment, else the group's doc comment; a constant that repeats the
    /// expressions before it, as Go lets it, with their type.
    fn values(
        &mut self,
        (first, last): (usize, usize),
        kind: Kind,
        parens: Option<(usize, usize)>,
        specs: Vec<Spec>,
    ) -> Option<()> {
        let (keyword, word) = match kind {
            Kind::Constant => (Keyword::Const, "const"),
            _ => (Keyword::Var, "var"),
        };
        self.keyword(first, keyword)?;
        let source = &self.source;
        let Some((open, close)) = parens else {
            let [spec] = &specs[..] else {
                return None;
            };
            let value = Value {
                names: spec.exported(),
                kind,
                doc: self.text([source.doc(first), source.line_comment(last)]),
                signature: source.code(first, last),
            };
            self.file
                .values
                .extend((!value.names.is_empty()).then_some(value));
            return Some(());
        };
        let open = self.bracket(open, Operator::ParenLeft)?;
        let close = self.bracket(close, Operator::ParenRight)?;
        let elements = source.elements_in(open, close);
        if elements.len() != specs.len() {
            return None;
        }
        let doc = source.doc(first);
        let mut ty = None;
        let mut values = Vec::new();
        for (&(first, last), spec) in elements.iter().zip(&specs) {
            if self.ident(first)? != spec.names.first()?.name {
                return None;
            }
            let (mut signature, typed) = self.spec(word, first, last, spec)?;
            if spec.valued {
                ty = typed.or_else(|| spec.conversion.clone());
            } else if let (None, Kind::Constant, Some(ty)) = (typed, kind, &ty) {
                signature = format!("{signature} {ty}");
            }
            let names = spec.exported();
            if !names.is_empty() {
                values.push(Value {
                    names,
                    kind,
                    doc: self.text([source.doc(first), source.line_comment(last), doc]),
                    signature,
                });
            }
        }
        self.file.values.extend(values);
        Some(())
    }

    /// The signature of a spec in a group, from its tokens `first` to
    /// `last`: `word`, then its names, type, and values, each as it's
    /// written, but with one space between them, whatever lines them up
    /// with the specs around it; and its type, if it has one.
    fn spec(
        &self,
        word: &str,
        first: usize,
        last: usize,
        spec: &Spec,
    ) -> Option<(String, Option<String>)> {
        let significant = self.source.significant(first, last);
        let margin = self.source.indent(first).unwrap_or("");
        let code = |from: usize, to: usize| {
            let (from, to) = (*significant.get(from)?, *significant.get(to)?);
            (from <= to).then(|| self.source.code_within(from, to, margin))
        };
        let names = 2 * spec.names.len() - 1;
        let assign = (significant.iter())
            .position(|&i| self.source.tokens[i].token == Token::Operator(Operator::Assign));
        let mut signature = format!("{word} {}", code(0, names - 1)?);
        let typed = match spec.typed {
            true => Some(code(names, assign.unwrap_or(significant.len()) - 1)?),
            false => None,
        };
        if let Some(ty) = &typed {
            signature = format!("{signature} {ty}");
        }
        if let Some(assign) = assign {
            signature = format!("{signature} = {}", code(assign + 1, significant.len() - 1)?);
        }
        Some((signature, typed))
    }

    /// The text of the first of `groups` of comments that has any.
    fn text<const N: usize>(&self, groups: [Option<Group>; N]) -> String {
        (groups.into_iter().flatten())
            .map(|group| self.source.text(group))
            .find(|text| !text.is_empty())
            .unwrap_or_default()
    }

    fn keyword(&self, i: usize, keyword: Keyword) -> Option<()> {
        (self.source.tokens[i].token == Token::Keyword(keyword)).then_some(())
    }

    fn ident(&self, i: usize) -> Option<&str> {
        match &self.source.tokens[i].token {
            Token::Literal(LitKind::Ident, name) => Some(name),
            _ => None,
        }
    }

    /// The index of the token at `position`, if it's `bracket`.
    fn bracket(&self, position: usize, bracket: Operator) -> Option<usize> {
        let i = self.source.index(position);
        let token = self.source.tokens.get(i)?;
        (token.start == position && token.token == Token::Operator(bracket)).then_some(i)
    }

    /// The last token from `first` to before `end` that isn't a comment.
    fn before(&self, first: usize, end: usize) -> Option<usize> {
        (first..end).rev().find(|&i| !self.source.is_comment(i))
    }
}

/// A spec of a `const` or `var` declaration.
struct Spec<'a> {
    names: &'a [ast::Ident],
    /// Whether it has a type.
    typed: bool,
    /// Whether it has values.
    valued: bool,
    /// The type its one value converts to, if it's a constant's, like
    /// `Kind` in `A = Kind(iota)`, for the constants that repeat it.
    conversion: Option<String>,
}

impl Spec<'_> {
    fn exported(&self) -> Vec<String> {
        (self.names.iter())
            .filter(|name| exported(&name.name))
            .map(|name| name.name.clone())
            .collect()
    }
}

/// Whether Go exports `name`: whether it starts with an upper-case letter.
pub fn exported(name: &str) -> bool {
    (name.chars().next())
        .is_some_and(|c| GeneralCategory::of(c) == GeneralCategory::UppercaseLetter)
}

/// The name of the type `typ` is, through a `*`, brackets, and type
/// arguments, and whether it's imported, as go/doc's `baseTypeName` finds
/// it, for an embedded field or a method's receiver.
fn base_type(typ: &Expression) -> Option<(&str, bool)> {
    match typ {
        Expression::Ident(ident) => Some((&ident.name, false)),
        Expression::Selector(selector) if matches!(*selector.x, Expression::Ident(_)) => {
            Some((&selector.sel.name, true))
        }
        Expression::Index(index) => base_type(&index.left),
        Expression::IndexList(list) => base_type(&list.left),
        Expression::Paren(paren) => base_type(&paren.expr),
        Expression::Star(star) => base_type(&star.right),
        Expression::TypePointer(pointer) => base_type(&pointer.typ),
        _ => None,
    }
}

/// The type `value` converts its one argument to, like `Kind` in
/// `Kind(iota)`, or `time.Duration` in `time.Duration(1)`, if it's a
/// conversion, not a call to a built-in function a constant can call.
fn conversion(value: &Expression) -> Option<String> {
    const BUILTINS: [&str; 7] = ["cap", "complex", "imag", "len", "max", "min", "real"];
    let Expression::Call(call) = value else {
        return None;
    };
    if call.args.len() != 1 {
        return None;
    }
    match &*call.func {
        Expression::Ident(ty) if !BUILTINS.contains(&ty.name.as_str()) => Some(ty.name.clone()),
        Expression::Selector(selector) => match &*selector.x {
            Expression::Ident(package) if package.name != "unsafe" => {
                Some(format!("{}.{}", package.name, selector.sel.name))
            }
            _ => None,
        },
        _ => None,
    }
}

/// A string literal's value, without its quotes, for an import's path,
/// which has no escapes.
fn unquote(literal: &str) -> String {
    let mut chars = literal.chars();
    chars.next();
    chars.next_back();
    chars.as_str().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str, ty: &str, doc: &str, embedded: bool) -> Field {
        Field {
            name: name.into(),
            ty: ty.into(),
            doc: doc.into(),
            embedded,
        }
    }

    #[test]
    fn a_file_has_its_package_doc_and_imports() {
        let file = read(
            "//go:build linux\n\n// Package p does things.\npackage p\n\nimport (\n\t\"fmt\"\n\tstr \"strings\"\n)\n\nimport \"io\"\n",
        )
        .unwrap();
        assert_eq!(file.package, "p");
        assert_eq!(file.doc, "Package p does things.\n");
        assert_eq!(
            file.imports,
            [
                (None, "fmt".into()),
                (Some("str".into()), "strings".into()),
                (None, "io".into())
            ]
        );
    }

    #[test]
    fn only_what_a_file_exports_is_read() {
        let file = read(
            "package p\n\nfunc f() {}\n\n// F is f.\nfunc F(a int) (int, error) {\n\treturn a, nil\n}\n\n// G is in assembly.\nfunc G() int\n\ntype t int\n\nvar v, V = 1, 2\n",
        )
        .unwrap();
        let funcs: Vec<_> = (file.funcs.iter())
            .map(|func| {
                (
                    func.name.as_str(),
                    func.doc.as_str(),
                    func.signature.as_str(),
                )
            })
            .collect();
        assert_eq!(
            funcs,
            [
                ("F", "F is f.\n", "func F(a int) (int, error)"),
                ("G", "G is in assembly.\n", "func G() int")
            ]
        );
        assert_eq!(
            file.funcs[0].source,
            "func F(a int) (int, error) {\n\treturn a, nil\n}"
        );
        assert!(file.types.is_empty());
        assert_eq!(file.values.len(), 1);
        assert_eq!(file.values[0].names, ["V"]);
        assert_eq!(file.values[0].signature, "var v, V = 1, 2");
    }

    #[test]
    fn a_method_is_read_with_its_receivers_type() {
        let file = read(
            "package p\n\ntype T[K any] struct{}\n\n// M is m.\nfunc (t *T[K]) M() {}\n\nfunc (x.T) N() {}\n\nfunc (t T[K]) m() {}\n",
        )
        .unwrap();
        assert_eq!(file.funcs.len(), 1);
        assert_eq!(file.funcs[0].receiver.as_deref(), Some("T"));
        assert_eq!(file.funcs[0].signature, "func (t *T[K]) M()");
    }

    #[test]
    fn a_struct_shows_only_the_fields_it_exports() {
        let text = "package p\n\n// T is t.\ntype T struct {\n\t// A is a.\n\tA int `json:\"a\"`\n\tb, C string // c\n\td bool\n\t*Embedded\n\tio.Reader\n\tlower\n}\n";
        let file = read(text).unwrap();
        let [t] = &file.types[..] else {
            panic!("{:?}", file.types)
        };
        assert_eq!(t.doc, "T is t.\n");
        assert_eq!(
            t.signature,
            "type T struct {\n\tA int `json:\"a\"`\n\tC string\n\t*Embedded\n\tio.Reader\n\t// contains filtered or unexported fields\n}"
        );
        assert_eq!(t.source.as_deref(), Some(&text[22..text.len() - 1]));
        assert_eq!(
            t.fields,
            [
                field("A", "int", "A is a.\n", false),
                field("C", "string", "c\n", false),
                field("Embedded", "*Embedded", "", true),
                field("Reader", "io.Reader", "", true),
            ]
        );
    }

    #[test]
    fn a_struct_on_one_line_is_as_its_written() {
        let file = read("package p\n\ntype T struct{ A int }\n\ntype U struct{ a int }\n").unwrap();
        assert_eq!(file.types[0].signature, "type T struct{ A int }");
        assert_eq!(file.types[0].source, None);
        assert_eq!(
            file.types[1].signature,
            "type U struct {\n\t// contains filtered or unexported fields\n}"
        );
    }

    #[test]
    fn an_interface_shows_only_the_methods_it_exports() {
        let file = read(
            "package p\n\ntype I interface {\n\tio.Reader\n\terror\n\t~int | ~string\n\t// M is m.\n\tM(a int) error\n\tn()\n\tN() // n\n}\n",
        )
        .unwrap();
        let [i] = &file.types[..] else {
            panic!("{:?}", file.types)
        };
        assert_eq!(
            i.signature,
            "type I interface {\n\tio.Reader\n\terror\n\t~int | ~string\n\tM(a int) error\n\tN()\n\t// contains filtered or unexported methods\n}"
        );
        assert_eq!(
            i.methods,
            [
                field("M", "M(a int) error", "M is m.\n", false),
                field("N", "N()", "n\n", false)
            ]
        );
        assert!(i.fields.is_empty());
    }

    #[test]
    fn a_type_in_a_group_has_its_own_doc_else_the_groups() {
        let file = read(
            "package p\n\n// Types.\ntype (\n\t// A is a.\n\tA int\n\tB struct {\n\t\tC int\n\t\td int\n\t}\n\tD = int\n)\n",
        )
        .unwrap();
        let [a, b, d] = &file.types[..] else {
            panic!("{:?}", file.types)
        };
        assert_eq!(
            (a.doc.as_str(), a.signature.as_str()),
            ("A is a.\n", "type A int")
        );
        assert_eq!(a.source, None);
        assert_eq!(b.doc, "Types.\n");
        assert_eq!(
            b.signature,
            "type B struct {\n\tC int\n\t// contains filtered or unexported fields\n}"
        );
        assert_eq!(
            b.source.as_deref(),
            Some("type B struct {\n\tC int\n\td int\n}")
        );
        assert_eq!(
            (d.signature.as_str(), d.source.as_deref()),
            ("type D = int", None)
        );
    }

    #[test]
    fn a_constant_repeats_the_type_before_it() {
        let file = read(
            "package p\n\n// Days.\nconst (\n\tSunday Weekday = iota // Sunday is first.\n\t// Monday is second.\n\tMonday\n\tTuesday\n\tx = 1\n\tY\n)\n",
        )
        .unwrap();
        let values: Vec<_> = (file.values.iter())
            .map(|value| {
                (
                    value.names.join(", "),
                    value.doc.as_str(),
                    value.signature.as_str(),
                )
            })
            .collect();
        assert_eq!(
            values,
            [
                (
                    "Sunday".into(),
                    "Sunday is first.\n",
                    "const Sunday Weekday = iota"
                ),
                (
                    "Monday".into(),
                    "Monday is second.\n",
                    "const Monday Weekday"
                ),
                ("Tuesday".into(), "Days.\n", "const Tuesday Weekday"),
                ("Y".into(), "Days.\n", "const Y"),
            ]
        );
    }

    #[test]
    fn a_constant_repeats_the_type_its_value_converts_to() {
        let file = read(
            "package p\n\nconst (\n\tUnknown = Kind(iota)\n\tBee\n\tWasp    = other.Insect(2)\n\tAnt\n\tSize    = len(\"abc\")\n\tLength\n\tPointer = unsafe.Sizeof(0)\n\tWidth\n)\n",
        )
        .unwrap();
        let signatures: Vec<_> = (file.values.iter())
            .map(|value| value.signature.as_str())
            .collect();
        assert_eq!(
            signatures,
            [
                "const Unknown = Kind(iota)",
                "const Bee Kind",
                "const Wasp = other.Insect(2)",
                "const Ant other.Insect",
                "const Size = len(\"abc\")",
                "const Length",
                "const Pointer = unsafe.Sizeof(0)",
                "const Width",
            ]
        );
    }

    #[test]
    fn a_value_in_a_group_isnt_lined_up_with_the_others() {
        let file = read(
            "package p\n\nvar (\n\tA, Bee int    = 1, 2\n\tLonger string // Longer.\n\tC             = f(\n\t\t1,\n\t)\n)\n",
        )
        .unwrap();
        let signatures: Vec<_> = (file.values.iter())
            .map(|value| value.signature.as_str())
            .collect();
        assert_eq!(
            signatures,
            [
                "var A, Bee int = 1, 2",
                "var Longer string",
                "var C = f(\n\t1,\n)",
            ]
        );
    }

    #[test]
    fn a_value_alone_has_its_doc_else_its_line_comment() {
        let file = read("package p\n\n// A is a.\nvar A = 1\n\nconst B = 2 // B is b.\n").unwrap();
        let values: Vec<_> = (file.values.iter())
            .map(|value| (value.kind, value.doc.as_str(), value.signature.as_str()))
            .collect();
        assert_eq!(
            values,
            [
                (Kind::Variable, "A is a.\n", "var A = 1"),
                (Kind::Constant, "B is b.\n", "const B = 2")
            ]
        );
    }

    #[test]
    fn a_file_left_out_of_builds_is_ignored() {
        assert!(ignored(
            "// Copyright\n\n//go:build ignore\n\npackage main\n"
        ));
        assert!(ignored("// +build ignore\r\n\r\npackage main\r\n"));
        assert!(!ignored("package main\n\n//go:build ignore\n"));
        assert!(!ignored("//go:build linux\n\npackage p\n"));
    }

    #[test]
    fn a_file_that_cant_be_parsed_says_where() {
        let errors = [
            (
                "package p\n\nfunc F( {\n}\n",
                "expect a type representation on line 3",
            ),
            ("package p\n\n\nfunc F() {\n", "expect statement on line 4"),
            (
                "package p\n\nvar x = \"\n",
                "unexpected character on line 3",
            ),
            ("package p; func )\n", "unexpected ')' on line 1"),
            ("package p\nfunc )\n", "unexpected ')' on line 2"),
            (
                "package p\n\ntype T struct {\n\tA int\n\n\tB int\n}\n\nfunc (T) M[P any]() {}\n",
                "unexpected '[' on line 9",
            ),
        ];
        for (text, error) in errors {
            assert_eq!(read(text).unwrap_err(), error, "{text:?}");
        }
    }

    #[test]
    fn a_name_is_exported_if_it_starts_in_upper_case() {
        assert!(exported("Name"));
        assert!(exported("Été"));
        assert!(!exported("name"));
        assert!(!exported("_Name"));
        assert!(!exported("ǅ"));
    }
}
