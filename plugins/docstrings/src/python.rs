//! Python's modules, read with Ruff's parser into what they document.

use ruff_python_ast::visitor::transformer::{Transformer, walk_expr};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_codegen::{Generator, Indentation};
use ruff_python_parser::{parse_expression, parse_module};
use ruff_source_file::LineEnding;
use ruff_text_size::{Ranged, TextRange};

use crate::docstring;
use crate::model::{Kind, Object, Parameter, Section};

/// The longest a signature is on one line, and the longest value an
/// attribute's signature shows.
const WIDTH: usize = 60;

/// The module or package called `name`, written in `source`, or why it
/// can't be read.
pub fn module(name: &str, kind: Kind, source: &str) -> Result<Object, String> {
    let parsed = parse_module(source).map_err(|error| {
        let before = source.get(..error.location.start().to_usize());
        let line = before.unwrap_or(source).matches('\n').count() + 1;
        format!("{} on line {line}", error.error)
    })?;
    let body = &parsed.syntax().body;
    let reader = Reader { source };
    let mut module = Object::new(name, kind);
    module.docs = docs(body);
    module.types = types(body);
    module.members = reader.members(body, false);
    if let Some(all) = dunder_all(body) {
        module.members.retain(|member| all.contains(&member.name));
    }
    Ok(module)
}

/// The sections of `docstring`, with the types it gives by their names, as
/// annotations are.
fn parse_docs(docstring: &str) -> Vec<Section> {
    let mut sections = docstring::parse(docstring);
    for section in &mut sections {
        let Section::Items { items, .. } = section else {
            continue;
        };
        for ty in items.iter_mut().filter_map(|item| item.ty.as_mut()) {
            if let Ok(parsed) = parse_expression(ty) {
                *ty = annotation(parsed.expr());
            }
        }
    }
    sections
}

/// Reads what a module's code defines.
struct Reader<'a> {
    source: &'a str,
}

impl Reader<'_> {
    /// What `body` defines that's documented, or that defines something
    /// that is, in a class if `in_class`.
    fn members(&self, body: &[Stmt], in_class: bool) -> Vec<Object> {
        let mut members: Vec<Object> = Vec::new();
        for (i, stmt) in body.iter().enumerate() {
            let next = body.get(i + 1);
            let object = match stmt {
                Stmt::ClassDef(class) => self.class(class),
                Stmt::FunctionDef(function) => self.function(function, in_class),
                Stmt::Assign(assign) => match assign.targets.as_slice() {
                    [Expr::Name(name)] => {
                        self.attribute(name.id.as_str(), None, Some(&assign.value), next)
                    }
                    _ => continue,
                },
                Stmt::AnnAssign(assign) => match assign.target.as_ref() {
                    Expr::Name(name) => self.attribute(
                        name.id.as_str(),
                        Some(&assign.annotation),
                        assign.value.as_deref(),
                        next,
                    ),
                    _ => continue,
                },
                _ => continue,
            };
            // The first of a name that's documented wins, like a property's
            // getter over its setter; else the last.
            match members.iter_mut().find(|member| member.name == object.name) {
                Some(member) if !member.is_documented() => *member = object,
                Some(_) => {}
                None => members.push(object),
            }
        }
        members.retain(|member| {
            !member.name.starts_with('_') && (member.is_documented() || !member.members.is_empty())
        });
        members
    }

    /// The class `class` defines.
    fn class(&self, class: &ast::StmtClassDef) -> Object {
        let mut object = Object::new(class.name.as_str(), Kind::Class);
        let dataclass = (class.decorator_list.iter())
            .map(|decorator| &decorator.expression)
            .find(|decorator| last_name(decorator) == Some("dataclass"));
        if dataclass.is_some() {
            object.labels.push("dataclass".to_string());
        }
        object.docs = docs(&class.body);
        object.types = types(&class.body);
        object.members = self.members(&class.body, true);
        let init = class.body.iter().rev().find_map(|stmt| match stmt {
            Stmt::FunctionDef(function) if function.name.as_str() == "__init__" => Some(function),
            _ => None,
        });
        let parameters = match (init, dataclass) {
            (Some(init), _) => {
                object.docs.extend(docs(&init.body));
                object.source = Some(self.source(init.range()));
                Some(self.parameters(&init.parameters, true))
            }
            (None, Some(dataclass)) if !matches!(dataclass, Expr::Call(call) if sets_false(call, "init")) => {
                Some(self.fields(&class.body))
            }
            _ => None,
        };
        if let Some((written, parameters)) = parameters {
            object.signature = Some(signature(&object.name, &written, None));
            object.parameters = parameters;
        }
        object
    }

    /// The function, method, or property `function` defines, in a class if
    /// `in_class`.
    fn function(&self, function: &ast::StmtFunctionDef, in_class: bool) -> Object {
        let decorators: Vec<&str> = (function.decorator_list.iter())
            .filter_map(|decorator| last_name(&decorator.expression))
            .collect();
        let has = |name: &str| decorators.contains(&name);
        let kind = match in_class {
            true if has("property") || has("cached_property") => Kind::Property,
            true => Kind::Method,
            false => Kind::Function,
        };
        let mut object = Object::new(function.name.as_str(), kind);
        if function.is_async {
            object.labels.push("async".to_string());
        }
        for label in ["staticmethod", "classmethod", "abstractmethod"] {
            if has(label) {
                object.labels.push(label.to_string());
            }
        }
        if has("cached_property") {
            object.labels.push("cached".to_string());
        }
        object.docs = docs(&function.body);
        object.returns = function.returns.as_deref().map(annotation);
        if kind == Kind::Property {
            object.signature = (object.returns.as_ref()).map(|ty| format!("{}: {ty}", object.name));
        } else {
            let method = in_class && !has("staticmethod");
            let (written, parameters) = self.parameters(&function.parameters, method);
            object.signature = Some(signature(&object.name, &written, object.returns.as_deref()));
            object.parameters = parameters;
        }
        object.source = Some(self.source(function.range()));
        object
    }

    /// The attribute called `name`, of the type `annotation`, set to
    /// `value`, and documented by `next`, the statement after it, if it's a
    /// string.
    fn attribute(
        &self,
        name: &str,
        annotation: Option<&Expr>,
        value: Option<&Expr>,
        next: Option<&Stmt>,
    ) -> Object {
        let mut object = Object::new(name, Kind::Attribute);
        object.docs = next.and_then(string).map(parse_docs).unwrap_or_default();
        object.returns = annotation.map(self::annotation);
        let mut signature = name.to_string();
        if let Some(ty) = &object.returns {
            signature += &format!(": {ty}");
        }
        let value = value.map(|value| self.code(value));
        if let Some(value) = value.filter(|value| value.chars().count() <= WIDTH) {
            signature += &format!(" = {value}");
        }
        object.signature = (signature != name).then_some(signature);
        object
    }

    /// The parameters of a signature, as they're written and as they are,
    /// but the first, if `method`.
    fn parameters(
        &self,
        parameters: &ast::Parameters,
        method: bool,
    ) -> (Vec<String>, Vec<Parameter>) {
        let mut written = Vec::new();
        let mut listed = Vec::new();
        let mut add = |written: &mut Vec<String>,
                       stars: &str,
                       parameter: &ast::Parameter,
                       default: Option<&Expr>| {
            let name = parameter.name.as_str().to_string();
            let annotation = parameter.annotation.as_deref().map(annotation);
            let default = default.map(|default| self.code(default));
            written.push(match (&annotation, &default) {
                (Some(ty), Some(default)) => format!("{stars}{name}: {ty} = {default}"),
                (Some(ty), None) => format!("{stars}{name}: {ty}"),
                (None, Some(default)) => format!("{stars}{name}={default}"),
                (None, None) => format!("{stars}{name}"),
            });
            let default = match stars {
                "*" => Some("()".to_string()),
                "**" => Some("{}".to_string()),
                _ => default,
            };
            listed.push(Parameter {
                name,
                annotation,
                default,
            });
        };
        let skip = usize::from(method);
        let only = parameters.posonlyargs.len().saturating_sub(skip);
        let positional = (parameters.posonlyargs.iter())
            .chain(&parameters.args)
            .skip(skip);
        for (i, parameter) in positional.enumerate() {
            add(
                &mut written,
                "",
                &parameter.parameter,
                parameter.default.as_deref(),
            );
            if i + 1 == only {
                written.push("/".to_string());
            }
        }
        if let Some(parameter) = &parameters.vararg {
            add(&mut written, "*", parameter, None);
        } else if !parameters.kwonlyargs.is_empty() {
            written.push("*".to_string());
        }
        for parameter in &parameters.kwonlyargs {
            add(
                &mut written,
                "",
                &parameter.parameter,
                parameter.default.as_deref(),
            );
        }
        if let Some(parameter) = &parameters.kwarg {
            add(&mut written, "**", parameter, None);
        }
        (written, listed)
    }

    /// The parameters of a dataclass's `__init__`, as Python makes them from
    /// the fields `body` defines, as they're written and as they are.
    fn fields(&self, body: &[Stmt]) -> (Vec<String>, Vec<Parameter>) {
        let mut written = Vec::new();
        let mut listed = Vec::new();
        for stmt in body {
            let Stmt::AnnAssign(assign) = stmt else {
                continue;
            };
            let Expr::Name(name) = assign.target.as_ref() else {
                continue;
            };
            let kind = match assign.annotation.as_ref() {
                Expr::Subscript(subscript) => last_name(&subscript.value),
                annotation => last_name(annotation),
            };
            match kind {
                Some("ClassVar") => continue,
                Some("KW_ONLY") => {
                    written.push("*".to_string());
                    continue;
                }
                _ => {}
            }
            let default = match assign.value.as_deref() {
                Some(Expr::Call(call)) if last_name(&call.func) == Some("field") => {
                    if sets_false(call, "init") {
                        continue;
                    }
                    match (keyword(call, "default"), keyword(call, "default_factory")) {
                        (Some(default), _) => Some(self.code(default)),
                        (None, Some(factory)) => Some(self.made_by(factory)),
                        (None, None) => None,
                    }
                }
                value => value.map(|value| self.code(value)),
            };
            let name = name.id.as_str().to_string();
            let annotation = annotation(&assign.annotation);
            written.push(match &default {
                Some(default) => format!("{name}: {annotation} = {default}"),
                None => format!("{name}: {annotation}"),
            });
            listed.push(Parameter {
                name,
                annotation: Some(annotation),
                default,
            });
        }
        (written, listed)
    }

    /// The value `factory` makes, as code, as a dataclass's field's default.
    fn made_by(&self, factory: &Expr) -> String {
        match factory {
            Expr::Name(_) | Expr::Attribute(_) => format!("{}()", self.code(factory)),
            Expr::Lambda(lambda) if lambda.parameters.is_none() => self.code(&lambda.body),
            _ => format!("({})()", self.code(factory)),
        }
    }

    /// `expr` as it's written, or, if it's over more than one line, as
    /// Ruff writes it.
    fn code(&self, expr: &Expr) -> String {
        match self.source.get(expr.range().to_std_range()) {
            Some(text) if !text.contains('\n') => text.to_string(),
            _ => code(expr),
        }
    }

    /// The code in `range`, from the start of its first line, without the
    /// indentation of that line.
    fn source(&self, range: TextRange) -> String {
        let start = range.start().to_usize();
        let start = self.source[..start]
            .rfind('\n')
            .map_or(0, |newline| newline + 1);
        let code = &self.source[start..range.end().to_usize()];
        let whitespace = |line: &str| line.len() - line.trim_start_matches([' ', '\t']).len();
        let margin = code.lines().next().map_or(0, whitespace);
        (code.lines())
            .map(|line| &line[margin.min(whitespace(line))..])
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// The sections of the docstring `body` starts with, if it does.
fn docs(body: &[Stmt]) -> Vec<Section> {
    body.first()
        .and_then(string)
        .map(parse_docs)
        .unwrap_or_default()
}

/// The string `stmt` is, if it's just a string, as a docstring is.
fn string(stmt: &Stmt) -> Option<&str> {
    let Stmt::Expr(ast::StmtExpr { value, .. }) = stmt else {
        return None;
    };
    match value.as_ref() {
        Expr::StringLiteral(string) => Some(string.value.to_str()),
        _ => None,
    }
}

/// The types of the attributes and properties `body` defines, by name,
/// documented or not, for the types in its docstring.
fn types(body: &[Stmt]) -> Vec<(String, String)> {
    (body.iter())
        .filter_map(|stmt| match stmt {
            Stmt::AnnAssign(assign) => match assign.target.as_ref() {
                Expr::Name(name) => {
                    Some((name.id.as_str().to_string(), annotation(&assign.annotation)))
                }
                _ => None,
            },
            Stmt::FunctionDef(function) => {
                let property = (function.decorator_list.iter()).any(|decorator| {
                    matches!(
                        last_name(&decorator.expression),
                        Some("property" | "cached_property")
                    )
                });
                let returns = function.returns.as_deref().filter(|_| property)?;
                Some((function.name.as_str().to_string(), annotation(returns)))
            }
            _ => None,
        })
        .collect()
}

/// How `name` is called with the parameters `written`, returning
/// `returns`: on one line, if it fits, else with a parameter to a line.
fn signature(name: &str, written: &[String], returns: Option<&str>) -> String {
    let returns = returns.map(|ty| format!(" -> {ty}")).unwrap_or_default();
    let line = format!("{name}({}){returns}", written.join(", "));
    if written.is_empty() || line.chars().count() <= WIDTH {
        return line;
    }
    let parameters: String = (written.iter())
        .map(|parameter| format!("    {parameter},\n"))
        .collect();
    format!("{name}(\n{parameters}){returns}")
}

/// `expr`, a type, as code, with each name on its own, like `ndarray` for
/// `np.ndarray`, and the code in a string, like `"Solver"`, as code.
fn annotation(expr: &Expr) -> String {
    let mut expr = expr.clone();
    Shorten.visit_expr(&mut expr);
    code(&expr)
}

/// `expr` as Ruff writes it.
fn code(expr: &Expr) -> String {
    Generator::new(&Indentation::default(), LineEnding::Lf).expr(expr)
}

/// Turns each name in a type into the name on its own, and the code in a
/// string into code, but in `Literal[...]`.
struct Shorten;

impl Transformer for Shorten {
    fn visit_expr(&self, expr: &mut Expr) {
        match expr {
            Expr::Attribute(attribute) => {
                *expr = Expr::Name(ast::ExprName {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: attribute.range,
                    id: attribute.attr.id.clone(),
                    ctx: ast::ExprContext::Load,
                });
            }
            Expr::StringLiteral(string) => {
                if let Ok(parsed) = parse_expression(string.value.to_str()) {
                    *expr = parsed.into_expr();
                    self.visit_expr(expr);
                }
            }
            Expr::Subscript(subscript) if last_name(&subscript.value) == Some("Literal") => {
                self.visit_expr(&mut subscript.value);
            }
            _ => walk_expr(self, expr),
        }
    }
}

/// The last name in `expr`, like `dataclass` in `dataclasses.dataclass`
/// or `dataclass(frozen=True)`.
fn last_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str()),
        Expr::Attribute(attribute) => Some(attribute.attr.as_str()),
        Expr::Call(call) => last_name(&call.func),
        _ => None,
    }
}

/// The value of the argument called `name` in `call`, if it's given.
fn keyword<'a>(call: &'a ast::ExprCall, name: &str) -> Option<&'a Expr> {
    (call.arguments.keywords.iter())
        .find(|keyword| keyword.arg.as_ref().is_some_and(|arg| arg.as_str() == name))
        .map(|keyword| &keyword.value)
}

/// Whether `call` sets `name` to `False`, like `field(init=False)`.
fn sets_false(call: &ast::ExprCall, name: &str) -> bool {
    matches!(keyword(call, name), Some(Expr::BooleanLiteral(literal)) if !literal.value)
}

/// The names a module lists in `__all__`, if it lists them in a way the
/// plugin can read.
fn dunder_all(body: &[Stmt]) -> Option<Vec<String>> {
    let mut all: Option<Vec<String>> = None;
    for stmt in body {
        let (target, value, extend) = match stmt {
            Stmt::Assign(assign) => match assign.targets.as_slice() {
                [target] => (target, assign.value.as_ref(), false),
                _ => continue,
            },
            Stmt::AnnAssign(ast::StmtAnnAssign {
                target,
                value: Some(value),
                ..
            }) => (target.as_ref(), value.as_ref(), false),
            Stmt::AugAssign(assign) if assign.op == ast::Operator::Add => {
                (assign.target.as_ref(), assign.value.as_ref(), true)
            }
            _ => continue,
        };
        if !matches!(target, Expr::Name(name) if name.id.as_str() == "__all__") {
            continue;
        }
        let names = strings(value)?;
        all = Some(match (extend, all.take()) {
            (true, Some(mut all)) => {
                all.extend(names);
                all
            }
            _ => names,
        });
    }
    all
}

/// The strings in `expr`, a list or tuple of them.
fn strings(expr: &Expr) -> Option<Vec<String>> {
    let elements = match expr {
        Expr::List(list) => &list.elts,
        Expr::Tuple(tuple) => &tuple.elts,
        _ => return None,
    };
    (elements.iter())
        .map(|element| match element {
            Expr::StringLiteral(string) => Some(string.value.to_str().to_string()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Item, Items};

    const MODULE: &str = r#"
"""A module.

More about it.
"""

import abc
import typing
from dataclasses import KW_ONLY, dataclass, field

import numpy as np

__all__ = ["LIMIT", "Problem", "Solver"]
__all__ += ["solve", "UNDOCUMENTED"]

LIMIT: int = 10
"""The most steps."""

UNDOCUMENTED = 1


@dataclass(frozen=True)
class Problem:
    """A problem.

    Parameters
    ----------
    matrix : np.ndarray
        The matrix.
    """

    matrix: np.ndarray
    names: list[str] = field(default_factory=list)
    scale: float = field(default=1e-6)
    cache: dict = field(init=False)
    count: typing.ClassVar[int] = 0
    _: KW_ONLY
    made: "np.ndarray" = field(default_factory=lambda: np.zeros(2))
    other: Thing = field(default_factory=make(1))


class Solver:
    """Solves problems.

    Attributes
    ----------
    tolerance
        How close.
    """

    limit: int = LIMIT

    def __init__(self, problem: "Problem", tolerance: float = 1e-6, *, verbose=False) -> None:
        """And makes them."""
        self.problem = problem

    @property
    def tolerance(self) -> float:
        """How close."""
        return 0.1

    @tolerance.setter
    def tolerance(self, value): ...

    @abc.abstractmethod
    async def solve(self, *args: int, key: "typing.Literal['a.b']", **kwargs) -> "np.ndarray":
        """Solve it."""

    @staticmethod
    def make(x, /, y):
        """Make one."""

    def _private(self):
        """Hidden."""

    def undocumented(self):
        pass


def solve(problem: Problem) -> int:
    """Solve a problem."""


def hidden():
    """Not in `__all__`."""
"#;

    fn module() -> Object {
        super::module("solving", Kind::Module, MODULE).unwrap()
    }

    fn member<'a>(object: &'a Object, name: &str) -> &'a Object {
        (object.members.iter())
            .find(|member| member.name == name)
            .unwrap_or_else(|| panic!("no {name} in {}", object.name))
    }

    fn names(object: &Object) -> Vec<&str> {
        object
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect()
    }

    #[test]
    fn a_module_documents_what_it_lists_in_all_that_has_a_docstring() {
        let module = module();
        assert_eq!(
            module.docs,
            [Section::Text {
                title: None,
                text: "A module.\n\nMore about it.".to_string()
            }]
        );
        assert_eq!(names(&module), ["LIMIT", "Problem", "Solver", "solve"]);
    }

    #[test]
    fn an_attribute_is_documented_by_the_string_after_it() {
        let limit = member(&module(), "LIMIT").clone();
        assert_eq!(limit.kind, Kind::Attribute);
        assert_eq!(limit.signature.as_deref(), Some("LIMIT: int = 10"));
        assert_eq!(limit.returns.as_deref(), Some("int"));
        assert!(limit.is_documented());
    }

    #[test]
    fn a_dataclass_is_called_with_its_fields() {
        let problem = member(&module(), "Problem").clone();
        assert_eq!(problem.labels, ["dataclass"]);
        assert_eq!(
            problem.signature.as_deref(),
            Some(
                "Problem(\n    matrix: ndarray,\n    names: list[str] = list(),\n    \
                 scale: float = 1e-6,\n    *,\n    made: ndarray = np.zeros(2),\n    \
                 other: Thing = (make(1))(),\n)"
            )
        );
        assert_eq!(problem.source, None);
        let Section::Items { kind, items } = &problem.docs[1] else {
            panic!("no parameters");
        };
        assert_eq!(*kind, Items::Parameters);
        assert_eq!(
            items[0],
            Item {
                name: Some("matrix".into()),
                ty: Some("ndarray".into()),
                default: None,
                description: "The matrix.".into()
            }
        );
    }

    #[test]
    fn a_class_is_called_as_its_init_is() {
        let solver = member(&module(), "Solver").clone();
        assert_eq!(solver.labels, Vec::<String>::new());
        assert_eq!(
            solver.signature.as_deref(),
            Some(
                "Solver(\n    problem: Problem,\n    tolerance: float = 1e-6,\n    *,\n    \
                 verbose=False,\n)"
            )
        );
        assert_eq!(solver.docs.len(), 3);
        assert_eq!(
            solver.source.as_deref(),
            Some(
                "def __init__(self, problem: \"Problem\", tolerance: float = 1e-6, *, verbose=False) -> None:\n    \
                 \"\"\"And makes them.\"\"\"\n    self.problem = problem"
            )
        );
        assert_eq!(names(&solver), ["tolerance", "solve", "make"]);
        assert_eq!(
            solver.types,
            [
                ("limit".to_string(), "int".to_string()),
                ("tolerance".to_string(), "float".to_string())
            ]
        );
        let defaults: Vec<_> = (solver.parameters.iter())
            .map(|parameter| (parameter.name.as_str(), parameter.default.as_deref()))
            .collect();
        assert_eq!(
            defaults,
            [
                ("problem", None),
                ("tolerance", Some("1e-6")),
                ("verbose", Some("False"))
            ]
        );
    }

    #[test]
    fn a_property_is_its_getter() {
        let tolerance = member(member(&module(), "Solver"), "tolerance").clone();
        assert_eq!(tolerance.kind, Kind::Property);
        assert_eq!(tolerance.signature.as_deref(), Some("tolerance: float"));
        assert!(
            tolerance
                .source
                .unwrap()
                .starts_with("@property\ndef tolerance(self)")
        );
    }

    #[test]
    fn a_method_leaves_out_self_but_a_static_method_does_not() {
        let module = module();
        let solve = member(member(&module, "Solver"), "solve");
        assert_eq!(solve.kind, Kind::Method);
        assert_eq!(solve.labels, ["async", "abstractmethod"]);
        assert_eq!(
            solve.signature.as_deref(),
            Some("solve(*args: int, key: Literal['a.b'], **kwargs) -> ndarray")
        );
        let defaults: Vec<_> = (solve.parameters.iter())
            .map(|parameter| (parameter.name.as_str(), parameter.default.as_deref()))
            .collect();
        assert_eq!(
            defaults,
            [("args", Some("()")), ("key", None), ("kwargs", Some("{}"))]
        );
        let make = member(member(&module, "Solver"), "make");
        assert_eq!(make.labels, ["staticmethod"]);
        assert_eq!(make.signature.as_deref(), Some("make(x, /, y)"));
        let solve = member(&module, "solve");
        assert_eq!(solve.kind, Kind::Function);
        assert_eq!(
            solve.signature.as_deref(),
            Some("solve(problem: Problem) -> int")
        );
    }

    #[test]
    fn a_module_that_cant_be_parsed_says_where() {
        let error = super::module("broken", Kind::Module, "x = 1\ndef f(:\n").unwrap_err();
        assert!(error.ends_with("on line 2"), "{error}");
    }

    #[test]
    fn all_that_cant_be_read_is_ignored() {
        let source = "__all__ = [name for name in dir()]\n\ndef f():\n    \"\"\"F.\"\"\"\n";
        let module = super::module("m", Kind::Module, source).unwrap();
        assert_eq!(names(&module), ["f"]);
    }

    #[test]
    fn a_long_value_is_left_out_of_an_attributes_signature() {
        let source = format!("X = {:?}\n\"\"\"X.\"\"\"\n", "x".repeat(60));
        let module = super::module("m", Kind::Module, &source).unwrap();
        assert_eq!(module.members[0].signature, None);
    }
}
