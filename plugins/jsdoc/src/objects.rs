//! The objects a module's page shows, read from its declarations: what
//! each is, its labels, its signature, as code, and the sections of its
//! doc comment.

use little_api_reference::model::{Item, Kind, Object, Parameter};
use oxc_ast::ast::{
    ArrowFunctionExpression, BindingPattern, Class, ClassElement, Decorator, Expression,
    FormalParameters, Function, MethodDefinition, MethodDefinitionKind, MethodDefinitionType,
    PropertyDefinitionType, TSAccessibility, TSEnumDeclaration, TSInterfaceDeclaration,
    TSMethodSignature, TSMethodSignatureKind, TSSignature, TSThisParameter, TSType,
    TSTypeAliasDeclaration, TSTypeAnnotation, TSTypeParameterDeclaration, VariableDeclaration,
    VariableDeclarator,
};
use oxc_span::{GetSpan, Span};

use crate::comment::{self, Comment, Context, Param, braced, param, template};
use crate::print::{
    self, WIDTH, call, indent, inline, key, literal, printed, source, type_arguments,
    type_parameters, written,
};
use crate::reader::{Declaration, Declared, Reader};

/// What can be called: a function, an arrow function, a method, or a
/// method's signature in an interface.
struct Callable<'a> {
    type_parameters: Option<&'a TSTypeParameterDeclaration<'a>>,
    this: Option<&'a TSThisParameter<'a>>,
    params: &'a FormalParameters<'a>,
    returns: Option<&'a TSTypeAnnotation<'a>>,
    r#async: bool,
    generator: bool,
    /// Whether it has a body, which an overload doesn't.
    body: bool,
}

impl<'a> Callable<'a> {
    fn function(function: &'a Function<'a>) -> Callable<'a> {
        Callable {
            type_parameters: function.type_parameters.as_deref(),
            this: function.this_param.as_deref(),
            params: &function.params,
            returns: function.return_type.as_deref(),
            r#async: function.r#async,
            generator: function.generator,
            body: function.body.is_some(),
        }
    }

    fn arrow(arrow: &'a ArrowFunctionExpression<'a>) -> Callable<'a> {
        Callable {
            type_parameters: arrow.type_parameters.as_deref(),
            this: None,
            params: &arrow.params,
            returns: arrow.return_type.as_deref(),
            r#async: arrow.r#async,
            generator: false,
            body: true,
        }
    }

    fn method(method: &'a TSMethodSignature<'a>) -> Callable<'a> {
        Callable {
            type_parameters: method.type_parameters.as_deref(),
            this: method.this_param.as_deref(),
            params: &method.params,
            returns: method.return_type.as_deref(),
            r#async: false,
            generator: false,
            body: false,
        }
    }

    /// How it's called, as code in `text`: `head`, like `function load`,
    /// then its type parameters, its parameters, and its return type.
    fn signature(&self, text: &str, head: &str) -> String {
        let head = format!("{head}{}", type_parameters(self.type_parameters));
        let params = print::parameters(text, self.this, self.params);
        let tail = match self.returns {
            Some(returns) => format!(": {}", inline(&returns.type_annotation)),
            None => String::new(),
        };
        call(&head, &params, &tail, WIDTH)
    }

    /// Its parameters, for the table of those its doc comment's `@param`s,
    /// `docs`, list: each named as the code names it, or, if it
    /// destructures, as the `@param` in its place does, with its type and
    /// default from the code, or else its default from the `@param`.
    fn parameters(&self, text: &str, docs: &[Param]) -> Vec<Parameter> {
        let named = |index: usize, pattern: &BindingPattern| match pattern {
            BindingPattern::BindingIdentifier(identifier) => identifier.name.to_string(),
            pattern => (docs.get(index)).map_or_else(|| inline(pattern), |doc| doc.name.clone()),
        };
        let mut parameters = Vec::new();
        for (index, param) in self.params.items.iter().enumerate() {
            let name = named(index, &param.pattern);
            let doc = docs.iter().find(|doc| doc.name == name);
            let default = match &param.initializer {
                Some(initializer) => Some(written(text, initializer.span())),
                None => (doc.and_then(|doc| doc.default.clone())).or_else(|| {
                    let optional = param.optional || doc.is_some_and(|doc| doc.optional);
                    optional.then(|| "undefined".to_string())
                }),
            };
            parameters.push(Parameter {
                name,
                annotation: annotation(param.type_annotation.as_deref()),
                default,
            });
        }
        if let Some(rest) = &self.params.rest {
            parameters.push(Parameter {
                name: named(self.params.items.len(), &rest.rest.argument),
                annotation: annotation(rest.type_annotation.as_deref()),
                default: Some("[]".to_string()),
            });
        }
        parameters
    }
}

/// The type `annotation` gives, as code, if there's one.
fn annotation(annotation: Option<&TSTypeAnnotation>) -> Option<String> {
    annotation.map(|annotation| inline(&annotation.type_annotation))
}

/// The type parameters `declaration` declares, for a doc comment's
/// `@template`s: each with its constraint and default.
fn type_items(declaration: Option<&TSTypeParameterDeclaration>) -> Vec<Item> {
    let params = declaration
        .into_iter()
        .flat_map(|declaration| &declaration.params);
    let items = params.map(|param| Item {
        name: Some(param.name.name.to_string()),
        ty: param.constraint.as_ref().map(inline),
        default: param.default.as_ref().map(inline),
        description: String::new(),
    });
    items.collect()
}

/// Each of `modifiers` that's on, then the labels that `comment` gives,
/// in the order a page shows them.
fn labels(modifiers: &[(&'static str, bool)], comment: &Comment) -> Vec<String> {
    let modifiers = (modifiers.iter())
        .filter(|(_, on)| *on)
        .map(|(label, _)| *label);
    comment::ordered(modifiers.chain(comment.labels()))
}

/// Whether `accessibility` is TypeScript's `protected`.
fn protected(accessibility: Option<TSAccessibility>) -> bool {
    accessibility == Some(TSAccessibility::Protected)
}

/// Whether `accessibility` is TypeScript's `private`, which hides what
/// it's on.
fn private(accessibility: Option<TSAccessibility>) -> bool {
    accessibility == Some(TSAccessibility::Private)
}

/// The overloads in `group`, the methods without a body, or, if it has
/// none, all of them.
fn overloads<'b, 'a>(group: &[&'b MethodDefinition<'a>]) -> Vec<&'b MethodDefinition<'a>> {
    let mut shown = group.to_vec();
    if shown.iter().any(|method| method.value.body.is_none()) {
        shown.retain(|method| method.value.body.is_none());
    }
    shown
}

impl Reader<'_> {
    /// What `declaration` is, shown under `heading`, if the module's page
    /// shows it: if it has a doc comment that doesn't hide it, or what it
    /// defines has.
    pub fn object(&self, heading: &str, declaration: &Declaration) -> Option<Object> {
        let comment = self.comment(&declaration.at);
        if comment.is_some_and(Comment::is_hidden) {
            return None;
        }
        let empty = Comment::default();
        let docs = comment.unwrap_or(&empty);
        let span = declaration.span;
        let object = match &declaration.declared {
            Declared::Function(function) => {
                self.function(heading, function, &declaration.overloads, docs, span)
            }
            Declared::Arrow(arrow) => self.held(heading, &Callable::arrow(arrow), None, docs, span),
            Declared::Class(class) => self.class(heading, class, docs),
            Declared::Variable(variable, declarator) => {
                self.variable(heading, variable, declarator, docs, span)
            }
            Declared::Interface(interface) => self.interface(heading, interface, docs),
            Declared::TypeAlias(alias) => self.alias(heading, alias, docs),
            Declared::Enum(declared) => self.enumeration(heading, declared, docs),
            Declared::Value(value, head) => {
                let mut object = Object::new(heading, Kind::Variable);
                object.labels = labels(&[], docs);
                let literal = literal(self.text, value);
                object.signature = literal.map(|literal| format!("{head} {literal}"));
                object.docs = comment::sections(docs, &self.context(Vec::new()));
                object
            }
        };
        let shown = comment.is_some() || object.is_documented() || !object.members.is_empty();
        shown.then_some(object)
    }

    /// The context of a doc comment on what has `type_parameters`, in the
    /// module's language.
    fn context(&self, type_parameters: Vec<Item>) -> Context {
        Context {
            language: self.language,
            type_parameters,
            ..Context::default()
        }
    }

    /// What `callable`, of `kind`, shown under `name`, is, as `comment`
    /// documents it, with `modifiers`: its labels, parameters, return
    /// type, and docs.
    fn callable(
        &self,
        name: &str,
        kind: Kind,
        callable: &Callable,
        modifiers: &[(&'static str, bool)],
        comment: &Comment,
    ) -> Object {
        let mut object = Object::new(name, kind);
        let modifiers = [modifiers, &[("async", callable.r#async)]].concat();
        object.labels = labels(&modifiers, comment);
        object.parameters = callable.parameters(self.text, &comment.params());
        object.returns = annotation(callable.returns);
        let context = self.context(type_items(callable.type_parameters));
        object.docs = comment::sections(comment, &context);
        object
    }

    /// The function `function`, after its `overloads`, shown under `name`,
    /// as `comment` documents it, in the statement at `span`. Its signature
    /// is its overloads', if it has a body, or else theirs and its own.
    fn function(
        &self,
        name: &str,
        function: &Function,
        overloads: &[&Function],
        comment: &Comment,
        span: Span,
    ) -> Object {
        let callable = Callable::function(function);
        let star = if function.generator { "*" } else { "" };
        let head = format!("function{star} {name}");
        let mut shown = overloads.to_vec();
        if overloads.is_empty() || !callable.body {
            shown.push(function);
        }
        let signatures: Vec<String> = (shown.iter())
            .map(|function| Callable::function(function).signature(self.text, &head))
            .collect();
        let mut object = self.callable(name, Kind::Function, &callable, &[], comment);
        object.signature = Some(signatures.join("\n"));
        object.source = callable.body.then(|| source(self.text, span));
        object
    }

    /// The function that a variable shown under `name` holds, `callable`,
    /// in the statement at `span`, as `comment` documents it: declared as
    /// `declared`, like `const load: Loader`, if the variable's type is
    /// given, or else as a function.
    fn held(
        &self,
        name: &str,
        callable: &Callable,
        declared: Option<String>,
        comment: &Comment,
        span: Span,
    ) -> Object {
        let mut object = self.callable(name, Kind::Function, callable, &[], comment);
        let star = if callable.generator { "*" } else { "" };
        let head = format!("function{star} {name}");
        object.signature = Some(declared.unwrap_or_else(|| callable.signature(self.text, &head)));
        object.source = Some(source(self.text, span));
        object
    }

    /// The variable `declarator`, in `variable`, shown under `name`, as
    /// `comment` documents it, in the statement at `span`: a function or a
    /// class, if it holds one.
    fn variable(
        &self,
        name: &str,
        variable: &VariableDeclaration,
        declarator: &VariableDeclarator,
        comment: &Comment,
        span: Span,
    ) -> Object {
        let ty = annotation(declarator.type_annotation.as_deref());
        let mut declared = format!("{} {name}", variable.kind.as_str());
        if let Some(ty) = &ty {
            declared += &format!(": {ty}");
        }
        let init = declarator
            .init
            .as_ref()
            .map(Expression::without_parentheses);
        let typed = ty.as_ref().map(|_| declared.clone());
        match init {
            Some(Expression::ClassExpression(class)) => return self.class(name, class, comment),
            Some(Expression::ArrowFunctionExpression(arrow)) => {
                return self.held(name, &Callable::arrow(arrow), typed, comment, span);
            }
            Some(Expression::FunctionExpression(function)) => {
                return self.held(name, &Callable::function(function), typed, comment, span);
            }
            _ => {}
        }
        if let Some(literal) = init.and_then(|init| literal(self.text, init)) {
            declared += &format!(" = {literal}");
        }
        let mut object = Object::new(name, Kind::Variable);
        object.labels = labels(&[], comment);
        object.signature = Some(declared);
        let context = Context {
            typed: ty.is_some(),
            ..self.context(Vec::new())
        };
        object.docs = comment::sections(comment, &context);
        object.returns = ty;
        object
    }

    /// The class `class`, shown under `name`, as `comment` documents it,
    /// then its constructor's doc comment. Its signature shows its
    /// constructor, or its overloads, if it has a body, but not a private
    /// one, and its source is its constructor's, as pydoc's is.
    fn class(&self, name: &str, class: &Class, comment: &Comment) -> Object {
        let type_parameters_ = class.type_parameters.as_deref();
        let mut head = format!("class {name}{}", type_parameters(type_parameters_));
        if let Some(heritage) = &class.heritage {
            let base = written(self.text, heritage.expression.span());
            let arguments = type_arguments(heritage.type_arguments.as_deref());
            head += &format!(" extends {base}{arguments}");
        }
        if !class.implements.is_empty() {
            let implements: Vec<String> = (class.implements.iter())
                .map(|implements| written(self.text, implements.span))
                .collect();
            head += &format!(" implements {}", implements.join(", "));
        }
        let constructors: Vec<&MethodDefinition> = (class.body.body.iter())
            .filter_map(|element| match element {
                ClassElement::MethodDefinition(method)
                    if method.kind == MethodDefinitionKind::Constructor =>
                {
                    Some(&**method)
                }
                _ => None,
            })
            .collect();
        let at: Vec<u32> = (constructors.iter())
            .flat_map(|constructor| starts(constructor.span, &constructor.decorators))
            .collect();
        let documented = self.comment(&at);
        let hidden = documented.is_some_and(Comment::is_hidden)
            || (constructors.iter()).any(|constructor| private(constructor.accessibility));
        let mut object = Object::new(name, Kind::Class);
        let mut merged = comment.clone();
        object.signature = Some(head.clone());
        if let Some(implementation) = constructors.last().filter(|_| !hidden) {
            if let Some(documented) = documented {
                let descriptions = [merged.description.as_str(), &documented.description];
                let descriptions: Vec<&str> = descriptions
                    .into_iter()
                    .filter(|text| !text.is_empty())
                    .collect();
                merged.description = descriptions.join("\n\n");
                merged.tags.extend(documented.tags.iter().cloned());
            }
            let lines: Vec<String> = (overloads(&constructors).iter())
                .map(|constructor| {
                    let head = match protected(constructor.accessibility) {
                        true => "protected constructor",
                        false => "constructor",
                    };
                    let callable = Callable::function(&constructor.value);
                    format!("{};", indent(&callable.signature(self.text, head), 2))
                })
                .collect();
            object.signature = Some(format!("{head} {{\n{}\n}}", lines.join("\n")));
            let callable = Callable::function(&implementation.value);
            object.parameters = callable.parameters(self.text, &merged.params());
            object.source = callable
                .body
                .then(|| source(self.text, implementation.span));
        }
        object.labels = labels(&[("abstract", class.r#abstract)], comment);
        let context = self.context(type_items(type_parameters_));
        object.docs = comment::sections(&merged, &context);
        object.members = self.members(&class.body.body);
        object
    }

    /// The doc comment on what starts at the first of `at` that one is on,
    /// if it doesn't hide it.
    fn documented(&self, at: &[u32]) -> Option<&Comment> {
        self.comment(at).filter(|comment| !comment.is_hidden())
    }

    /// What a class with `elements` defines that its page shows: each
    /// method, field, and accessor with a doc comment that doesn't hide
    /// it, but those that are private, which TypeScript's `private`, or a
    /// name like `#size`, makes them.
    fn members(&self, elements: &[ClassElement]) -> Vec<Object> {
        // A method's name, and whether it's static, and an accessor, which
        // the methods that share them are documented under together.
        type Group = (String, bool, bool);
        let mut methods: Vec<(Group, Vec<&MethodDefinition>)> = Vec::new();
        let mut members = Vec::new();
        for element in elements {
            match element {
                ClassElement::MethodDefinition(method) => {
                    let constructor = method.kind == MethodDefinitionKind::Constructor;
                    let name = key(self.text, &method.key, method.computed);
                    let Some(name) =
                        name.filter(|_| !constructor && !private(method.accessibility))
                    else {
                        continue;
                    };
                    let accessor = matches!(
                        method.kind,
                        MethodDefinitionKind::Get | MethodDefinitionKind::Set
                    );
                    let group = (name, method.r#static, accessor);
                    match methods.iter_mut().find(|(key, _)| *key == group) {
                        Some((_, group)) => group.push(method),
                        None => methods.push((group, vec![method])),
                    }
                }
                ClassElement::PropertyDefinition(property) => {
                    let name = key(self.text, &property.key, property.computed);
                    let Some(name) = name.filter(|_| !private(property.accessibility)) else {
                        continue;
                    };
                    let at = starts(property.span, &property.decorators);
                    let Some(comment) = self.documented(&at) else {
                        continue;
                    };
                    let abstract_ =
                        property.r#type == PropertyDefinitionType::TSAbstractPropertyDefinition;
                    let modifiers = [
                        ("static", property.r#static),
                        ("abstract", abstract_),
                        ("protected", protected(property.accessibility)),
                        ("readonly", property.readonly),
                        ("optional", property.optional),
                        ("override", property.r#override),
                    ];
                    let value = property.value.as_ref();
                    let callable = match value.map(Expression::without_parentheses) {
                        Some(Expression::ArrowFunctionExpression(arrow)) => {
                            Some(Callable::arrow(arrow))
                        }
                        Some(Expression::FunctionExpression(function)) => {
                            Some(Callable::function(function))
                        }
                        _ => None,
                    };
                    let annotated = property.type_annotation.as_deref();
                    let object = match callable {
                        Some(callable) => {
                            let kind = Kind::Method;
                            let mut object =
                                self.callable(&name, kind, &callable, &modifiers, comment);
                            object.signature = Some(match annotation(annotated) {
                                Some(ty) => format!("{name}: {ty}"),
                                None => callable.signature(self.text, &name),
                            });
                            object.source = Some(source(self.text, property.span));
                            object
                        }
                        None => {
                            let optional = property.optional;
                            self.field(&name, annotated, value, optional, &modifiers, comment)
                        }
                    };
                    members.push(object);
                }
                ClassElement::AccessorProperty(property) => {
                    let name = key(self.text, &property.key, property.computed);
                    let Some(name) = name.filter(|_| !private(property.accessibility)) else {
                        continue;
                    };
                    let at = starts(property.span, &property.decorators);
                    let Some(comment) = self.documented(&at) else {
                        continue;
                    };
                    let modifiers = [
                        ("static", property.r#static),
                        ("protected", protected(property.accessibility)),
                        ("override", property.r#override),
                        ("accessor", true),
                    ];
                    let annotated = property.type_annotation.as_deref();
                    let value = property.value.as_ref();
                    members.push(self.field(&name, annotated, value, false, &modifiers, comment));
                }
                _ => {}
            }
        }
        for ((name, _, accessor), group) in methods {
            let at: Vec<u32> = (group.iter())
                .flat_map(|method| starts(method.span, &method.decorators))
                .collect();
            let Some(comment) = self.documented(&at) else {
                continue;
            };
            members.push(match accessor {
                true => self.accessors(&name, &group, comment),
                false => self.methods(&name, &group, comment),
            });
        }
        members
    }

    /// The method `name`, whose overloads, then implementation, are
    /// `group`, as `comment` documents it. Its signature is its overloads',
    /// if it has a body, or else all of theirs.
    fn methods(&self, name: &str, group: &[&MethodDefinition], comment: &Comment) -> Object {
        let implementation = group[group.len() - 1];
        let callable = Callable::function(&implementation.value);
        let star = if implementation.value.generator {
            "*"
        } else {
            ""
        };
        let optional = if implementation.optional { "?" } else { "" };
        let head = format!("{star}{name}{optional}");
        let signatures: Vec<String> = (overloads(group).iter())
            .map(|method| Callable::function(&method.value).signature(self.text, &head))
            .collect();
        let abstract_ = implementation.r#type == MethodDefinitionType::TSAbstractMethodDefinition;
        let modifiers = [
            ("static", implementation.r#static),
            ("abstract", abstract_),
            ("protected", protected(implementation.accessibility)),
            ("optional", implementation.optional),
            ("override", implementation.r#override),
        ];
        let mut object = self.callable(name, Kind::Method, &callable, &modifiers, comment);
        object.signature = Some(signatures.join("\n"));
        object.source = callable
            .body
            .then(|| source(self.text, implementation.span));
        object
    }

    /// The accessor `name`, whose getter, setter, or both, are `group`, as
    /// `comment` documents it: a property, read-only without a setter.
    fn accessors(&self, name: &str, group: &[&MethodDefinition], comment: &Comment) -> Object {
        let getter = group
            .iter()
            .find(|method| method.kind == MethodDefinitionKind::Get);
        let setter = group
            .iter()
            .find(|method| method.kind == MethodDefinitionKind::Set);
        let returns = getter.and_then(|getter| getter.value.return_type.as_deref());
        let annotated = returns.or_else(|| {
            let param = setter?.value.params.items.first()?;
            param.type_annotation.as_deref()
        });
        let first = group[0];
        let abstract_ = first.r#type == MethodDefinitionType::TSAbstractMethodDefinition;
        let modifiers = [
            ("static", first.r#static),
            ("abstract", abstract_),
            ("protected", protected(first.accessibility)),
            ("readonly", setter.is_none()),
            ("override", first.r#override),
        ];
        let mut object = self.field(name, annotated, None, false, &modifiers, comment);
        let sources: Vec<String> = (group.iter())
            .filter(|method| method.value.body.is_some())
            .map(|method| source(self.text, method.span))
            .collect();
        object.source = Some(sources.join("\n\n")).filter(|sources| !sources.is_empty());
        object
    }

    /// A property `name`, of a class or an interface, of the type
    /// `annotated`, and with `value`, if it has them, as `comment`
    /// documents it, with `modifiers`.
    fn field(
        &self,
        name: &str,
        annotated: Option<&TSTypeAnnotation>,
        value: Option<&Expression>,
        optional: bool,
        modifiers: &[(&'static str, bool)],
        comment: &Comment,
    ) -> Object {
        let ty = annotation(annotated);
        let mut signature = format!("{name}{}", if optional { "?" } else { "" });
        if let Some(ty) = &ty {
            signature += &format!(": {ty}");
        }
        let value = value.map(Expression::without_parentheses);
        if let Some(literal) = value.and_then(|value| literal(self.text, value)) {
            signature += &format!(" = {literal}");
        }
        let mut object = Object::new(name, Kind::Field);
        object.labels = labels(modifiers, comment);
        object.signature = Some(signature);
        let context = Context {
            typed: ty.is_some(),
            ..self.context(Vec::new())
        };
        object.docs = comment::sections(comment, &context);
        object.returns = ty;
        object
    }

    /// The interface `interface`, shown under `name`, as `comment`
    /// documents it, with each of its members in its signature, but those
    /// hidden.
    fn interface(
        &self,
        name: &str,
        interface: &TSInterfaceDeclaration,
        comment: &Comment,
    ) -> Object {
        let type_parameters_ = interface.type_parameters.as_deref();
        let mut head = format!("interface {name}{}", type_parameters(type_parameters_));
        if !interface.extends.is_empty() {
            let extends: Vec<String> = (interface.extends.iter())
                .map(|heritage| written(self.text, heritage.span))
                .collect();
            head += &format!(" extends {}", extends.join(", "));
        }
        let mut object = Object::new(name, Kind::Interface);
        object.signature = Some(format!("{head} {}", self.body(&interface.body.body)));
        object.labels = labels(&[], comment);
        let context = self.context(type_items(type_parameters_));
        object.docs = comment::sections(comment, &context);
        object.members = self.signatures(&interface.body.body);
        object
    }

    /// What an interface with `signatures` defines that its page shows:
    /// each property and method with a doc comment that doesn't hide it.
    fn signatures(&self, signatures: &[TSSignature]) -> Vec<Object> {
        // The methods of each name, and accessors or not.
        let mut methods: Vec<((String, bool), Vec<&TSMethodSignature>)> = Vec::new();
        let mut members = Vec::new();
        for signature in signatures {
            match signature {
                TSSignature::TSPropertySignature(property) => {
                    let name = key(self.text, &property.key, property.computed);
                    let comment = self.documented(&[property.span.start]);
                    let (Some(name), Some(comment)) = (name, comment) else {
                        continue;
                    };
                    let optional = property.optional;
                    let modifiers = [("readonly", property.readonly), ("optional", optional)];
                    let annotated = property.type_annotation.as_deref();
                    members.push(self.field(&name, annotated, None, optional, &modifiers, comment));
                }
                TSSignature::TSMethodSignature(method) => {
                    let Some(name) = key(self.text, &method.key, method.computed) else {
                        continue;
                    };
                    let group = (name, method.kind != TSMethodSignatureKind::Method);
                    match methods.iter_mut().find(|(key, _)| *key == group) {
                        Some((_, group)) => group.push(method),
                        None => methods.push((group, vec![method])),
                    }
                }
                _ => {}
            }
        }
        for ((name, accessor), group) in methods {
            let at: Vec<u32> = group.iter().map(|method| method.span.start).collect();
            let Some(comment) = self.documented(&at) else {
                continue;
            };
            let last = group[group.len() - 1];
            members.push(match accessor {
                true => {
                    let getter =
                        (group.iter()).find(|method| method.kind == TSMethodSignatureKind::Get);
                    let setter =
                        (group.iter()).find(|method| method.kind == TSMethodSignatureKind::Set);
                    let returns = getter.and_then(|getter| getter.return_type.as_deref());
                    let annotated = returns
                        .or_else(|| setter?.params.items.first()?.type_annotation.as_deref());
                    let modifiers = [("readonly", setter.is_none())];
                    self.field(&name, annotated, None, false, &modifiers, comment)
                }
                false => {
                    let head = format!("{name}{}", if last.optional { "?" } else { "" });
                    let signatures: Vec<String> = (group.iter())
                        .map(|method| Callable::method(method).signature(self.text, &head))
                        .collect();
                    let modifiers = [("optional", last.optional)];
                    let callable = Callable::method(last);
                    let mut object =
                        self.callable(&name, Kind::Method, &callable, &modifiers, comment);
                    object.signature = Some(signatures.join("\n"));
                    object
                }
            });
        }
        members
    }

    /// The type alias `alias`, shown under `name`, as `comment` documents
    /// it, with the properties of the object type it is, if it's one, but
    /// those hidden.
    fn alias(&self, name: &str, alias: &TSTypeAliasDeclaration, comment: &Comment) -> Object {
        let type_parameters_ = alias.type_parameters.as_deref();
        let ty = match &alias.type_annotation {
            TSType::TSTypeLiteral(literal) => self.body(&literal.members),
            ty => printed(ty),
        };
        let mut object = Object::new(name, Kind::TypeAlias);
        let head = format!("type {name}{}", type_parameters(type_parameters_));
        object.signature = Some(format!("{head} = {ty}"));
        object.labels = labels(&[], comment);
        let mut context = self.context(type_items(type_parameters_));
        context.typed = true;
        if let TSType::TSTypeLiteral(literal) = &alias.type_annotation {
            context.properties = self.properties(&literal.members);
        }
        object.docs = comment::sections(comment, &context);
        object
    }

    /// The properties of an object type with `members`, each with its
    /// type, and its doc comment's description and `@default`, but those
    /// it hides.
    fn properties(&self, members: &[TSSignature]) -> Vec<Item> {
        let properties = members.iter().filter_map(|member| {
            let TSSignature::TSPropertySignature(property) = member else {
                return None;
            };
            let name = key(self.text, &property.key, property.computed)?;
            let comment = self.comment(&[property.span.start]);
            if comment.is_some_and(Comment::is_hidden) {
                return None;
            }
            let defaults = comment.map(|comment| comment.texts(&["default", "defaultValue"]));
            let default = defaults.and_then(|mut defaults| defaults.find(|text| !text.is_empty()));
            Some(Item {
                name: Some(format!(
                    "{name}{}",
                    if property.optional { "?" } else { "" }
                )),
                ty: annotation(property.type_annotation.as_deref()),
                default: default.map(String::from),
                description: (comment.map(|comment| comment.description.clone()))
                    .unwrap_or_default(),
            })
        });
        properties.collect()
    }

    /// The enum `declared`, shown under `name`, as `comment` documents it,
    /// with each of its members, and its value, but those hidden.
    fn enumeration(&self, name: &str, declared: &TSEnumDeclaration, comment: &Comment) -> Object {
        let mut lines = Vec::new();
        let mut members = Vec::new();
        for member in &declared.body.members {
            let documented = self.comment(&[member.span.start]);
            if documented.is_some_and(Comment::is_hidden) {
                continue;
            }
            let id = written(self.text, member.id.span());
            let value = (member.initializer.as_ref())
                .map(|initializer| written(self.text, initializer.span()));
            lines.push(match &value {
                Some(value) => format!("  {id} = {value},"),
                None => format!("  {id},"),
            });
            members.push(Item {
                name: Some(member.id.static_name().to_string()),
                ty: None,
                default: value,
                description: (documented.map(|comment| comment.description.clone()))
                    .unwrap_or_default(),
            });
        }
        let body = match lines.is_empty() {
            true => "{}".to_string(),
            false => format!("{{\n{}\n}}", lines.join("\n")),
        };
        let r#const = if declared.r#const { "const " } else { "" };
        let mut object = Object::new(name, Kind::Enum);
        object.signature = Some(format!("{const}enum {name} {body}"));
        object.labels = labels(&[], comment);
        let context = Context {
            members,
            ..self.context(Vec::new())
        };
        object.docs = comment::sections(comment, &context);
        object
    }

    /// The type that `comment`, a `@typedef` or a `@callback`, defines, if
    /// it names one: an object type with its `@property`s, a function type
    /// with its `@param`s and `@returns`, or else the type it gives.
    pub fn typedef(&self, comment: &Comment) -> Option<Object> {
        let (callback, text) = match comment.texts(&["typedef"]).next() {
            Some(text) => (false, text),
            None => (true, comment.texts(&["callback"]).next()?),
        };
        let (ty, rest) = braced(text);
        let name = rest.split_whitespace().next()?;
        let name = name.rsplit('~').next().unwrap_or(name);
        let templates = comment
            .texts(&["template", "typeParam", "typeparam"])
            .flat_map(template);
        let names: Vec<String> = templates.filter_map(|item| item.name).collect();
        let head = match names.is_empty() {
            true => format!("type {name} = "),
            false => format!("type {name}<{}> = ", names.join(", ")),
        };
        let signature = if callback {
            let params: Vec<String> = (comment.params().iter())
                .map(|param| {
                    let ty = param.ty.as_deref().unwrap_or("any");
                    match ty.strip_prefix("...") {
                        Some(ty) => format!("...{}: {ty}[]", param.name),
                        None => {
                            let optional = if param.optional { "?" } else { "" };
                            format!("{}{optional}: {ty}", param.name)
                        }
                    }
                })
                .collect();
            let returns = comment
                .texts(&["returns", "return"])
                .find_map(|text| braced(text).0);
            let tail = format!(" => {}", returns.as_deref().unwrap_or("void"));
            call(&head, &params, &tail, WIDTH)
        } else {
            let properties: Vec<Param> = (comment.texts(&["property", "prop"]))
                .map(param)
                .filter(|property| !property.name.contains('.'))
                .collect();
            let object = (ty.as_deref()).is_none_or(|ty| matches!(ty, "Object" | "object"));
            match object && !properties.is_empty() {
                true => {
                    let lines: Vec<String> = (properties.iter())
                        .map(|property| {
                            let optional = if property.optional { "?" } else { "" };
                            let ty = property.ty.as_deref().unwrap_or("any");
                            format!("  {}{optional}: {ty};", property.name)
                        })
                        .collect();
                    format!("{head}{{\n{}\n}}", lines.join("\n"))
                }
                false => format!("{head}{}", ty.as_deref().unwrap_or("any")),
            }
        };
        let mut object = Object::new(name, Kind::TypeAlias);
        object.labels = labels(&[], comment);
        object.signature = Some(signature);
        object.docs = comment::sections(comment, &self.context(Vec::new()));
        Some(object)
    }

    /// The body of an interface or an object type with `signatures`, as
    /// code, with each on a line of its own, but those whose doc comments
    /// hide them.
    fn body(&self, signatures: &[TSSignature]) -> String {
        let lines: Vec<String> = (signatures.iter())
            .filter(|signature| {
                let comment = self.comment(&[signature.span().start]);
                !comment.is_some_and(Comment::is_hidden)
            })
            .map(|signature| format!("  {};", inline(signature)))
            .collect();
        match lines.is_empty() {
            true => "{}".to_string(),
            false => format!("{{\n{}\n}}", lines.join("\n")),
        }
    }
}

/// Where a member at `span` starts, and where the first of its
/// `decorators` does, either of which a doc comment on it can be before.
fn starts(span: Span, decorators: &[Decorator]) -> Vec<u32> {
    let decorator = decorators.first().map(|decorator| decorator.span.start);
    std::iter::once(span.start).chain(decorator).collect()
}
