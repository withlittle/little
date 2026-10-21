//! A module of the package, read from its code with oxc's parser: what its
//! page shows, and what it imports and exports, which its links lead
//! through.

use std::collections::{HashMap, HashSet};

use little_api_reference::model::{Kind, Object, Section};
use little_api_reference::pages::join;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    self, Argument, ArrowFunctionExpression, AssignmentOperator, AssignmentTarget, BindingPattern,
    Class, ExportDefaultDeclaration, ExportDefaultDeclarationKind, Expression, Function,
    ImportDeclaration, ImportDeclarationSpecifier, ObjectExpression, ObjectPropertyKind, Statement,
    TSEnumDeclaration, TSInterfaceDeclaration, TSTypeAliasDeclaration, VariableDeclaration,
    VariableDeclarator,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};

use crate::comment::{self, Comment, Context, braced};
use crate::files::Source;
use crate::print::written;

/// A module of the package, as its page shows it.
#[derive(Debug)]
pub struct Module {
    /// What its page shows: its doc comment's sections, and what it
    /// exports that's documented.
    pub object: Object,
    /// The name its `@module` tag gives it, if it gives one, like
    /// `my-lib/strings`, which `{@link module:my-lib/strings}` leads to.
    pub name: Option<String>,
    /// The language of its code, as a page's code blocks name it, like
    /// `ts`.
    pub language: &'static str,
    /// The heading each of its declarations that its page shows is under,
    /// by its name in the module.
    pub locals: HashMap<String, String>,
    /// What each name it exports is.
    pub exports: HashMap<String, Export>,
    /// The specifier of each module it exports all of, with
    /// `export * from`.
    pub stars: Vec<String>,
    /// What each name it imports is, by its name in the module.
    pub imports: HashMap<String, Import>,
    /// What it exports from other modules, in order, which its page lists.
    pub re_exports: Vec<ReExport>,
}

impl Module {
    /// A module of code in `language`, with nothing in it.
    fn new(language: &'static str) -> Module {
        Module {
            object: Object::new("", Kind::Module),
            name: None,
            language,
            locals: HashMap::new(),
            exports: HashMap::new(),
            stars: Vec::new(),
            imports: HashMap::new(),
            re_exports: Vec::new(),
        }
    }

    /// Whether its page has something to show: its docs, what it exports
    /// that's documented, or what it exports from other modules.
    pub fn is_documented(&self) -> bool {
        self.object.is_documented()
            || !self.object.members.is_empty()
            || !self.re_exports.is_empty()
    }
}

/// What a module exports under a name.
#[derive(Clone, Debug, PartialEq)]
pub enum Export {
    /// What it declares, with the heading its page shows it under, if it
    /// shows it.
    Local(Option<String>),
    /// What it exports from another module.
    From(Import),
}

/// What a module imports from another, or exports from it.
#[derive(Clone, Debug, PartialEq)]
pub struct Import {
    /// The other module's specifier, as the code writes it, like
    /// `./config`.
    pub specifier: String,
    /// The name the other module exports it under, like `default`, or
    /// none for the whole module.
    pub name: Option<String>,
}

/// What a module exports from another, as its page lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct ReExport {
    /// What the page lists it as: the name it's exported under, or, for all
    /// of a module, `* from './config'`.
    pub shown: String,
    pub import: Import,
}

/// What reads a module's declarations: its code, and its doc comments.
pub struct Reader<'a> {
    /// The module's code.
    pub text: &'a str,
    /// The language of its code, like `ts`.
    pub language: &'static str,
    /// Each doc comment on code, by where what it's on starts.
    comments: HashMap<u32, Comment>,
}

impl Reader<'_> {
    /// The doc comment on what starts at the first of `at` that one is on.
    pub fn comment(&self, at: &[u32]) -> Option<&Comment> {
        at.iter().find_map(|start| self.comments.get(start))
    }
}

/// What a declaration that a module may show declares.
#[derive(Clone)]
pub enum Declared<'a> {
    Function(&'a Function<'a>),
    Arrow(&'a ArrowFunctionExpression<'a>),
    Class(&'a Class<'a>),
    /// A variable, which may be a function or a class.
    Variable(&'a VariableDeclaration<'a>, &'a VariableDeclarator<'a>),
    Interface(&'a TSInterfaceDeclaration<'a>),
    TypeAlias(&'a TSTypeAliasDeclaration<'a>),
    Enum(&'a TSEnumDeclaration<'a>),
    /// Another value a module exports, with what the code writes before
    /// it, like `exports.version =`.
    Value(&'a Expression<'a>, String),
}

/// A declaration that a module may show: what it declares, and where.
#[derive(Clone)]
pub struct Declaration<'a> {
    pub declared: Declared<'a>,
    /// The overloads of a function before it, which have no bodies, in
    /// order.
    pub overloads: Vec<&'a Function<'a>>,
    /// Where it starts, which tells it from the others.
    id: u32,
    /// Where what a doc comment on it can be on starts: it, then the
    /// statement it's in, and where it's exported.
    pub at: Vec<u32>,
    /// The statement it's in, whose code is its source.
    pub span: Span,
}

impl<'a> Declaration<'a> {
    /// A declaration of what `declared` is, with `at`, of which the first
    /// is where it starts, in the statement at `span`.
    fn new(declared: Declared<'a>, at: Vec<u32>, span: Span) -> Declaration<'a> {
        Declaration {
            declared,
            overloads: Vec::new(),
            id: at[0],
            at,
            span,
        }
    }
}

/// The module in the file `source`, in the folder `root`; or one with
/// nothing in it, warning why, if the file can't be read or parsed.
pub fn read(root: &str, source: &Source, warnings: &mut Vec<String>) -> Module {
    let path = join(root, &source.path);
    let language = language(&source.path);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            warnings.push(format!("can't read `{path}`: {error}"));
            return Module::new(language);
        }
    };
    let source_type = SourceType::from_path(&source.path).unwrap_or_default();
    let source_type = match source_type.is_typescript() {
        true => source_type,
        false => source_type.with_jsx(true),
    };
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &text, source_type).parse();
    if let Some(error) = parsed.diagnostics.errors().next() {
        let offset = error
            .labels
            .first()
            .map_or(0, |label| label.offset() as usize);
        let line = text.get(..offset).unwrap_or_default().matches('\n').count() + 1;
        warnings.push(format!(
            "can't parse `{path}`: {} on line {line}",
            error.message
        ));
        return Module::new(language);
    }
    let mut docs = None;
    let mut typedefs = Vec::new();
    let mut comments = HashMap::new();
    for found in parsed
        .program
        .comments
        .iter()
        .filter(|found| found.is_jsdoc())
    {
        let comment = Comment::parse(found.content_span().source_text(&text));
        if comment.is_module() {
            docs.get_or_insert(comment);
        } else if (comment.has("typedef") || comment.has("callback")) && !comment.is_hidden() {
            typedefs.push(comment);
        } else if !comment.is_standalone() {
            comments.insert(found.attached_to, comment);
        }
    }
    let mut module = Module::new(language);
    if let Some(docs) = docs {
        (module.name, module.object.docs) = documented(docs, language);
    }
    let mut builder = Builder {
        reader: Reader {
            text: &text,
            language,
            comments,
        },
        module,
        esm: parsed.module_record.has_module_syntax,
        common: false,
        aliases: aliases(&parsed.program.body),
        declared: HashMap::new(),
        order: Vec::new(),
        queue: Vec::new(),
        emitted: Vec::new(),
    };
    for statement in &parsed.program.body {
        builder.statement(statement);
    }
    builder.finish(typedefs)
}

/// The language of the code in the file at `path`, as a page's code blocks
/// name it, like `ts`.
fn language(path: &str) -> &'static str {
    let typescript = [".ts", ".mts", ".cts"]
        .iter()
        .any(|extension| path.ends_with(extension));
    if path.ends_with(".tsx") {
        "tsx"
    } else if path.ends_with(".jsx") {
        "jsx"
    } else if typescript {
        "ts"
    } else {
        "js"
    }
}

/// The name that `comment`, a module's doc comment, gives the module with
/// `@module`, if it gives one, and the sections of its page it makes, with
/// what its `@file` says after its description.
fn documented(comment: Comment, language: &'static str) -> (Option<String>, Vec<Section>) {
    let name = comment.texts(&["module"]).next().and_then(|text| {
        let name = braced(text).1.split_whitespace().next()?;
        Some(name.trim_matches(['"', '\'', '`']).to_string()).filter(|name| !name.is_empty())
    });
    let description: Vec<&str> = std::iter::once(comment.description.as_str())
        .chain(comment.texts(&["file", "fileoverview", "overview"]))
        .filter(|text| !text.is_empty())
        .collect();
    let description = description.join("\n\n");
    let comment = Comment {
        description,
        tags: comment.tags,
    };
    let context = Context {
        language,
        ..Context::default()
    };
    (name, comment::sections(&comment, &context))
}

/// What reads a module's statements, in order: what they declare, and
/// what the module exports, before it's known what each name it exports
/// is.
struct Builder<'a> {
    reader: Reader<'a>,
    module: Module,
    /// Whether the module is an ES module, with `import` or `export`.
    esm: bool,
    /// Whether it exports as CommonJS does, with `module.exports` or
    /// `exports`.
    common: bool,
    /// The names that hold what it exports, as CommonJS does, so that
    /// `name.x = …` exports `x`.
    aliases: HashSet<&'a str>,
    /// Each declaration, by its name.
    declared: HashMap<&'a str, Declaration<'a>>,
    /// The names declared, in the order they're first declared.
    order: Vec<&'a str>,
    /// What the module exports, in order.
    queue: Vec<Queued<'a>>,
    /// The heading and the id of each of the module's members, in order.
    emitted: Vec<(String, u32)>,
}

/// What a module exports, before it's known what each name it exports is.
enum Queued<'a> {
    /// What it calls `local`, exported as `exported`, with `at`, where else
    /// a doc comment on it can be.
    Named {
        exported: String,
        local: &'a str,
        at: Vec<u32>,
    },
    /// What it declares as it exports it, shown under `heading`.
    Declared {
        exported: String,
        heading: String,
        declaration: Declaration<'a>,
        labels: Vec<&'static str>,
    },
    /// What it exports from another module, under `exported`, or, if none,
    /// all it exports.
    From {
        exported: Option<String>,
        shown: String,
        import: Import,
    },
}

impl<'a> Builder<'a> {
    /// Read `statement`.
    fn statement(&mut self, statement: &'a Statement<'a>) {
        let start = statement.span().start;
        match statement {
            Statement::ImportDeclaration(import) => self.import(import),
            Statement::ExportDeclaration(export) => {
                for name in self.declare(&export.declaration, &[start], export.span) {
                    self.export(name, name, Vec::new());
                }
            }
            Statement::ExportDefaultDeclaration(export) => self.default(export),
            Statement::ExportNamedDeclaration(export) => {
                for specifier in &export.specifiers {
                    let exported = specifier.exported.name().as_str();
                    self.export(exported, specifier.local.name().as_str(), Vec::new());
                }
            }
            Statement::ExportFromDeclaration(export) => {
                for specifier in &export.specifiers {
                    let exported = specifier.exported.name().to_string();
                    let import = Import {
                        specifier: export.source.value.to_string(),
                        name: Some(specifier.local.name().to_string()),
                    };
                    let shown = exported.clone();
                    let exported = Some(exported);
                    self.queue.push(Queued::From {
                        exported,
                        shown,
                        import,
                    });
                }
            }
            Statement::ExportAllDeclaration(export) => {
                let import = Import {
                    specifier: export.source.value.to_string(),
                    name: None,
                };
                let (exported, shown) = match &export.exported {
                    Some(name) => (Some(name.name().to_string()), name.name().to_string()),
                    None => (
                        None,
                        format!("* from {}", written(self.reader.text, export.source.span)),
                    ),
                };
                self.queue.push(Queued::From {
                    exported,
                    shown,
                    import,
                });
            }
            Statement::TSExportAssignment(export) => {
                self.assigned(None, &export.expression, "export =", export.span);
            }
            Statement::ExpressionStatement(statement) if !self.esm => {
                let (targets, value) = chain(&statement.expression);
                for target in targets {
                    if let Some(name) = exports(target, &self.aliases) {
                        let head = format!("{} =", written(self.reader.text, target.span()));
                        self.assigned(name, value, &head, statement.span);
                    }
                }
            }
            statement => {
                if let Some(declaration) = statement.as_declaration() {
                    self.declare(declaration, &[], statement.span());
                }
            }
        }
    }

    /// Note what `import` imports, by the name it gives each.
    fn import(&mut self, import: &ImportDeclaration) {
        let specifier = import.source.value.to_string();
        for imported in import.specifiers.iter().flatten() {
            let name = match imported {
                ImportDeclarationSpecifier::ImportSpecifier(imported) => {
                    Some(imported.imported.name().to_string())
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => Some("default".into()),
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => None,
            };
            let import = Import {
                specifier: specifier.clone(),
                name,
            };
            self.module
                .imports
                .insert(imported.name().to_string(), import);
        }
    }

    /// Read `export`, an `export default`.
    fn default(&mut self, export: &'a ExportDefaultDeclaration<'a>) {
        let extra = [export.span.start];
        let (declaration, name) = match &export.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(declared) => (
                function(declared, &extra, export.span),
                declared.id.as_ref().map(|id| id.name.as_str()),
            ),
            ExportDefaultDeclarationKind::ClassDeclaration(declared) => (
                class(declared, &extra, export.span),
                declared.id.as_ref().map(|id| id.name.as_str()),
            ),
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(declared) => {
                let at = at(declared.span.start, &extra);
                let declaration = Declaration::new(Declared::Interface(declared), at, export.span);
                (declaration, Some(declared.id.name.as_str()))
            }
            kind => {
                let Some(expression) = kind.as_expression() else {
                    return;
                };
                if let Expression::Identifier(identifier) = expression.without_parentheses() {
                    self.export("default", identifier.name.as_str(), extra.to_vec());
                    return;
                }
                let at = at(expression.span().start, &extra);
                let declared = valued(expression, "export default");
                (Declaration::new(declared, at, export.span), None)
            }
        };
        match name {
            Some(name) => {
                self.insert(name, declaration);
                self.export("default", name, Vec::new());
            }
            None => self.queue.push(Queued::Declared {
                exported: "default".to_string(),
                heading: "default".to_string(),
                declaration,
                labels: Vec::new(),
            }),
        }
    }

    /// Declare what `declaration`, in the statement at `span`, declares,
    /// with `extra`, where else a doc comment on it can be, and give the
    /// names it declares.
    fn declare(
        &mut self,
        declaration: &'a ast::Declaration<'a>,
        extra: &[u32],
        span: Span,
    ) -> Vec<&'a str> {
        let (name, declaration) = match declaration {
            ast::Declaration::VariableDeclaration(variable) => {
                return self.variables(variable, extra, span);
            }
            ast::Declaration::FunctionDeclaration(declared) => match &declared.id {
                Some(id) => (id.name.as_str(), function(declared, extra, span)),
                None => return Vec::new(),
            },
            ast::Declaration::ClassDeclaration(declared) => match &declared.id {
                Some(id) => (id.name.as_str(), class(declared, extra, span)),
                None => return Vec::new(),
            },
            ast::Declaration::TSInterfaceDeclaration(declared) => {
                let at = at(declared.span.start, extra);
                let declaration = Declaration::new(Declared::Interface(declared), at, span);
                (declared.id.name.as_str(), declaration)
            }
            ast::Declaration::TSTypeAliasDeclaration(declared) => {
                let at = at(declared.span.start, extra);
                let declaration = Declaration::new(Declared::TypeAlias(declared), at, span);
                (declared.id.name.as_str(), declaration)
            }
            ast::Declaration::TSEnumDeclaration(declared) => {
                let at = at(declared.span.start, extra);
                let declaration = Declaration::new(Declared::Enum(declared), at, span);
                (declared.id.name.as_str(), declaration)
            }
            _ => return Vec::new(),
        };
        self.insert(name, declaration);
        vec![name]
    }

    /// Declare the variables that `variable`, in the statement at `span`,
    /// declares, but those it destructures, noting what each `require()`
    /// imports, and give their names.
    fn variables(
        &mut self,
        variable: &'a VariableDeclaration<'a>,
        extra: &[u32],
        span: Span,
    ) -> Vec<&'a str> {
        let mut names = Vec::new();
        for (index, declarator) in variable.declarations.iter().enumerate() {
            if let Some(import) = declarator.init.as_ref().and_then(required) {
                self.bind(&declarator.id, import);
                continue;
            }
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                continue;
            };
            let mut at = vec![declarator.span.start];
            // A doc comment before `const` is on its first variable.
            if index == 0 {
                at.push(variable.span.start);
                at.extend(extra);
            }
            let name = identifier.name.as_str();
            let declared = Declared::Variable(variable, declarator);
            self.insert(name, Declaration::new(declared, at, span));
            names.push(name);
            // `var app = module.exports = {}` exports `app`, and what's in it.
            if let Some(init) = declarator.init.as_ref().filter(|_| !self.esm) {
                let (targets, value) = chain(init);
                if targets.into_iter().any(module_exports) {
                    self.common = true;
                    self.export("default", name, Vec::new());
                    if let Expression::ObjectExpression(object) = value {
                        self.properties(object);
                    }
                }
            }
        }
        names
    }

    /// Note what `pattern`, bound to what a `require()` imports, `import`,
    /// imports.
    fn bind(&mut self, pattern: &BindingPattern, import: Import) {
        match pattern {
            BindingPattern::BindingIdentifier(identifier) => {
                self.module
                    .imports
                    .insert(identifier.name.to_string(), import);
            }
            BindingPattern::ObjectPattern(object) if import.name.is_none() => {
                for property in object
                    .properties
                    .iter()
                    .filter(|property| !property.computed)
                {
                    let name = property.key.static_name();
                    let (Some(name), Some(local)) = (name, property.value.get_identifier_name())
                    else {
                        continue;
                    };
                    let import = Import {
                        specifier: import.specifier.clone(),
                        name: Some(name.into_owned()),
                    };
                    self.module.imports.insert(local.to_string(), import);
                }
            }
            _ => {}
        }
    }

    /// Declare `declaration` as `name`, after the overloads of the function
    /// declared as `name` before it, if it has no body.
    fn insert(&mut self, name: &'a str, mut declaration: Declaration<'a>) {
        match self.declared.get(name) {
            Some(previous) => {
                if let (Declared::Function(_), Declared::Function(overload)) =
                    (&declaration.declared, &previous.declared)
                    && overload.body.is_none()
                {
                    let mut at = previous.at.clone();
                    at.extend(&declaration.at);
                    declaration.at = at;
                    declaration.id = previous.id;
                    declaration.overloads = previous.overloads.clone();
                    declaration.overloads.push(overload);
                }
            }
            None => self.order.push(name),
        }
        self.declared.insert(name, declaration);
    }

    /// Export what the module calls `local` as `exported`, with `at`, where
    /// else a doc comment on it can be.
    fn export(&mut self, exported: &str, local: &'a str, at: Vec<u32>) {
        let exported = exported.to_string();
        self.queue.push(Queued::Named {
            exported,
            local,
            at,
        });
    }

    /// Read `module.exports = value`, if `name` is none, or else
    /// `exports.name = value`, in the statement at `span`, which writes
    /// `head` before the value, like `exports.name =`.
    fn assigned(
        &mut self,
        name: Option<&'a str>,
        value: &'a Expression<'a>,
        head: &str,
        span: Span,
    ) {
        self.common = true;
        let value = value.without_parentheses();
        if let Some(import) = required(value) {
            let default = import.name.as_ref().map(|_| "default".to_string());
            let exported = name.map(String::from).or(default);
            let shown =
                (exported.clone()).unwrap_or_else(|| written(self.reader.text, value.span()));
            self.queue.push(Queued::From {
                exported,
                shown,
                import,
            });
            return;
        }
        let at = vec![value.span().start, span.start];
        match (name, value) {
            (_, Expression::Identifier(identifier)) => {
                let exported = name.unwrap_or("default");
                self.export(exported, identifier.name.as_str(), vec![span.start]);
            }
            (None, Expression::ObjectExpression(object)) => self.properties(object),
            (Some(name), value) => self.queue.push(Queued::Declared {
                exported: name.to_string(),
                heading: name.to_string(),
                declaration: Declaration::new(valued(value, head), at, span),
                labels: Vec::new(),
            }),
            (None, value) => {
                let own = match value {
                    Expression::FunctionExpression(function) => function.id.as_ref(),
                    Expression::ClassExpression(class) => class.id.as_ref(),
                    _ => None,
                };
                self.queue.push(Queued::Declared {
                    exported: "default".to_string(),
                    heading: own.map_or("default", |id| id.name.as_str()).to_string(),
                    declaration: Declaration::new(valued(value, head), at, span),
                    labels: own.map(|_| "default").into_iter().collect(),
                });
            }
        }
    }

    /// Read the properties of `object`, in `module.exports = { ... }`.
    fn properties(&mut self, object: &'a ObjectExpression<'a>) {
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            if let Some(key) = property.key.static_name().filter(|_| !property.computed) {
                self.property(&key, &property.value, property.span);
            }
        }
    }

    /// Read `key: value`, at `span`, in `module.exports = { ... }`.
    fn property(&mut self, key: &str, value: &'a Expression<'a>, span: Span) {
        let value = value.without_parentheses();
        if let Some(import) = required(value) {
            let (exported, shown) = (Some(key.to_string()), key.to_string());
            self.queue.push(Queued::From {
                exported,
                shown,
                import,
            });
        } else if let Expression::Identifier(identifier) = value {
            self.export(key, identifier.name.as_str(), vec![span.start]);
        } else {
            let at = vec![span.start, value.span().start];
            let declared = valued(value, &format!("{key}:"));
            self.queue.push(Queued::Declared {
                exported: key.to_string(),
                heading: key.to_string(),
                declaration: Declaration::new(declared, at, span),
                labels: Vec::new(),
            });
        }
    }

    /// The module, with what it exports, or, if it's a script, what it
    /// declares, that's documented, and the types its `@typedef`s and
    /// `@callback`s, `typedefs`, define.
    fn finish(mut self, typedefs: Vec<Comment>) -> Module {
        if !self.esm && !self.common {
            for name in std::mem::take(&mut self.order) {
                let declaration = self.declared[name].clone();
                self.emit(name, name, &declaration, &[]);
            }
        }
        for queued in std::mem::take(&mut self.queue) {
            match queued {
                Queued::Named {
                    exported,
                    local,
                    at,
                } => self.named(exported, local, at),
                Queued::Declared {
                    exported,
                    heading,
                    declaration,
                    labels,
                } => {
                    let heading = self.emit(&heading, &heading, &declaration, &labels);
                    self.module.exports.insert(exported, Export::Local(heading));
                }
                Queued::From {
                    exported,
                    shown,
                    import,
                } => {
                    match exported {
                        Some(exported) => {
                            let export = Export::From(import.clone());
                            self.module.exports.insert(exported, export);
                        }
                        None => self.module.stars.push(import.specifier.clone()),
                    }
                    self.module.re_exports.push(ReExport { shown, import });
                }
            }
        }
        for comment in typedefs {
            let Some(object) = self.reader.typedef(&comment) else {
                continue;
            };
            let name = object.name.clone();
            self.module.object.members.push(object);
            let locals = self.module.locals.entry(name.clone());
            locals.or_insert_with(|| name.clone());
            let exports = self.module.exports.entry(name.clone());
            exports.or_insert(Export::Local(Some(name)));
        }
        self.module
    }

    /// Export what the module calls `local` as `exported`, with `at`,
    /// where else a doc comment on it can be: what it imports, or what it
    /// declares, shown under the name it's exported as, or, if it's the
    /// default, its own.
    fn named(&mut self, exported: String, local: &str, at: Vec<u32>) {
        let export = if let Some(import) = self.module.imports.get(local) {
            let import = import.clone();
            let shown = exported.clone();
            let re_export = ReExport {
                shown,
                import: import.clone(),
            };
            self.module.re_exports.push(re_export);
            Export::From(import)
        } else if let Some(declaration) = self.declared.get(local) {
            let mut declaration = declaration.clone();
            declaration.at.extend(at);
            let default = exported == "default";
            let heading = if default { local } else { exported.as_str() };
            let labels: &[&str] = if default { &["default"] } else { &[] };
            Export::Local(self.emit(local, heading, &declaration, labels))
        } else {
            Export::Local(None)
        };
        self.module.exports.insert(exported, export);
    }

    /// Show `declaration`, which the module calls `local`, under `heading`,
    /// with `labels`, if it's documented, and give the heading it's under:
    /// the one it was first shown under, if it's shown already.
    fn emit(
        &mut self,
        local: &str,
        heading: &str,
        declaration: &Declaration<'a>,
        labels: &[&str],
    ) -> Option<String> {
        let shown = self
            .emitted
            .iter()
            .position(|(_, id)| *id == declaration.id);
        if let Some(index) = shown {
            let object = &mut self.module.object.members[index];
            let all = object.labels.iter().map(String::as_str);
            object.labels = comment::ordered(all.chain(labels.iter().copied()));
            return Some(self.emitted[index].0.clone());
        }
        let mut object = self.reader.object(heading, declaration)?;
        let all = object.labels.iter().map(String::as_str);
        object.labels = comment::ordered(all.chain(labels.iter().copied()));
        self.module.object.members.push(object);
        self.emitted.push((heading.to_string(), declaration.id));
        let locals = self.module.locals.entry(local.to_string());
        locals.or_insert_with(|| heading.to_string());
        Some(heading.to_string())
    }
}

/// `start`, where a declaration starts, then `extra`, where else a doc
/// comment on it can be.
fn at(start: u32, extra: &[u32]) -> Vec<u32> {
    std::iter::once(start)
        .chain(extra.iter().copied())
        .collect()
}

/// The declaration of the function `declared`, in the statement at `span`,
/// with `extra`, where else a doc comment on it can be.
fn function<'a>(declared: &'a Function<'a>, extra: &[u32], span: Span) -> Declaration<'a> {
    Declaration::new(
        Declared::Function(declared),
        at(declared.span.start, extra),
        span,
    )
}

/// The declaration of the class `declared`, in the statement at `span`,
/// with `extra`, where else a doc comment on it can be, as well as before
/// its decorators.
fn class<'a>(declared: &'a Class<'a>, extra: &[u32], span: Span) -> Declaration<'a> {
    let mut at = at(declared.span.start, extra);
    at.extend(
        declared
            .decorators
            .first()
            .map(|decorator| decorator.span.start),
    );
    Declaration::new(Declared::Class(declared), at, span)
}

/// What `expression`, a value exported after `head`, like
/// `export default`, declares: a function or a class, if it's one, or else
/// a value.
fn valued<'a>(expression: &'a Expression<'a>, head: &str) -> Declared<'a> {
    match expression.without_parentheses() {
        Expression::FunctionExpression(function) => Declared::Function(function),
        Expression::ArrowFunctionExpression(arrow) => Declared::Arrow(arrow),
        Expression::ClassExpression(class) => Declared::Class(class),
        expression => Declared::Value(expression, head.to_string()),
    }
}

/// What assigning to `target` exports, as CommonJS does: the name it
/// exports for `exports.name`, `module.exports.name`, or `alias.name`, if
/// `aliases` holds `alias`; none for `module.exports`, its default; or
/// nothing for anything else.
fn exports<'a>(
    target: &'a AssignmentTarget<'a>,
    aliases: &HashSet<&str>,
) -> Option<Option<&'a str>> {
    if module_exports(target) {
        return Some(None);
    }
    let member = target.as_member_expression()?;
    let name = member.static_property_name()?;
    let object = member.object();
    let exports = object.is_specific_id("exports")
        || object.is_specific_member_access("module", "exports")
        || matches!(object, Expression::Identifier(alias) if aliases.contains(alias.name.as_str()));
    exports.then_some(Some(name))
}

/// Whether `target` is `module.exports`.
fn module_exports(target: &AssignmentTarget) -> bool {
    target.as_member_expression().is_some_and(|member| {
        member.object().is_specific_id("module") && member.static_property_name() == Some("exports")
    })
}

/// The targets of the assignments `expression` chains, like `a` and `b` in
/// `a = b = value`, and the value they're all set to.
fn chain<'a>(
    expression: &'a Expression<'a>,
) -> (Vec<&'a AssignmentTarget<'a>>, &'a Expression<'a>) {
    let mut targets = Vec::new();
    let mut value = expression.without_parentheses();
    while let Expression::AssignmentExpression(assignment) = value
        && assignment.operator == AssignmentOperator::Assign
    {
        targets.push(&assignment.left);
        value = assignment.right.without_parentheses();
    }
    (targets, value)
}

/// The names that hold what a module, with the statements `body`, exports,
/// as CommonJS does: those set to `exports` or `module.exports`, as in
/// `var app = exports = module.exports = {}`, and those `module.exports` is
/// set to, as in `module.exports = app`.
fn aliases<'a>(body: &'a [Statement<'a>]) -> HashSet<&'a str> {
    let mut aliases = HashSet::new();
    for statement in body {
        match statement {
            Statement::VariableDeclaration(variable) => {
                for declarator in &variable.declarations {
                    let (BindingPattern::BindingIdentifier(id), Some(init)) =
                        (&declarator.id, &declarator.init)
                    else {
                        continue;
                    };
                    let (targets, value) = chain(init);
                    let exports = value.is_specific_id("exports")
                        || value.is_specific_member_access("module", "exports");
                    if exports || targets.into_iter().any(module_exports) {
                        aliases.insert(id.name.as_str());
                    }
                }
            }
            Statement::ExpressionStatement(statement) => {
                let (targets, value) = chain(&statement.expression);
                if !targets.iter().any(|target| module_exports(target)) {
                    continue;
                }
                for target in targets {
                    if let AssignmentTarget::AssignmentTargetIdentifier(id) = target {
                        aliases.insert(id.name.as_str());
                    }
                }
                if let Expression::Identifier(id) = value {
                    aliases.insert(id.name.as_str());
                }
            }
            _ => {}
        }
    }
    aliases
}

/// What `expression` imports, if it's `require('./config')`, or a name
/// that it exports, like `require('./config').load`.
fn required(expression: &Expression) -> Option<Import> {
    match expression.without_parentheses() {
        Expression::CallExpression(call) if call.is_require_call() => match &call.arguments[0] {
            Argument::StringLiteral(specifier) => Some(Import {
                specifier: specifier.value.to_string(),
                name: None,
            }),
            _ => None,
        },
        expression => {
            let member = expression.as_member_expression()?;
            let import = required(member.object()).filter(|import| import.name.is_none())?;
            let name = Some(member.static_property_name()?.to_string());
            Some(Import { name, ..import })
        }
    }
}
