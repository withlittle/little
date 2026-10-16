//! Code as a page shows it, in signatures: types printed with oxc's
//! codegen, without comments, and values as the source writes them.

use oxc_ast::ast::{
    Expression, FormalParameter, FormalParameters, PropertyKey, TSThisParameter,
    TSTypeParameterDeclaration, TSTypeParameterInstantiation, UnaryOperator,
};
use oxc_codegen::{Codegen, CodegenOptions, CommentOptions, Context, Gen, IndentChar};
use oxc_span::{GetSpan, Span};

/// The width a signature fits in on one line, or else has a line for each
/// parameter.
pub const WIDTH: usize = 80;

/// `node` printed as code, with two spaces to an indent, and no comments.
pub fn printed<T: Gen>(node: &T) -> String {
    let options = CodegenOptions {
        indent_char: IndentChar::Space,
        indent_width: 2,
        comments: CommentOptions::disabled(),
        ..CodegenOptions::default()
    };
    let mut codegen = Codegen::new().with_options(options);
    node.print(&mut codegen, Context::default());
    codegen.into_source_text()
}

/// `node` printed as code on one line, as a type is in a signature, like
/// `{ x: number }`.
pub fn inline<T: Gen>(node: &T) -> String {
    let printed = printed(node);
    let lines: Vec<&str> = (printed.lines().map(str::trim))
        .filter(|line| !line.is_empty())
        .collect();
    lines.join(" ").replace("; }", " }")
}

/// The code at `span` in `text`, as it's written, but on one line.
pub fn written(text: &str, span: Span) -> String {
    let code = &text[span.start as usize..span.end as usize];
    code.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The code at `span` in `text`, from the start of its first line, without
/// the margin its lines share, or the whitespace they end with.
pub fn source(text: &str, span: Span) -> String {
    let (start, end) = (span.start as usize, span.end as usize);
    let line = text[..start].rfind('\n').map_or(0, |newline| newline + 1);
    let code = match text[line..start].trim().is_empty() {
        true => &text[line..end],
        false => &text[start..end],
    };
    let lines: Vec<&str> = (code.split('\n'))
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    let margin = (lines.iter())
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    let lines: Vec<&str> = (lines.iter())
        .map(|line| line.get(margin..).unwrap_or_default().trim_end())
        .collect();
    lines.join("\n")
}

/// `text` with `width` spaces before each of its lines but blank ones.
pub fn indent(text: &str, width: usize) -> String {
    let lines = text.split('\n').map(|line| match line.is_empty() {
        true => String::new(),
        false => format!("{}{line}", " ".repeat(width)),
    });
    lines.collect::<Vec<_>>().join("\n")
}

/// The type parameters `declaration` declares, as code, like
/// `<T extends object = {}>`, or nothing if there's none.
pub fn type_parameters(declaration: Option<&TSTypeParameterDeclaration>) -> String {
    let Some(declaration) = declaration else {
        return String::new();
    };
    let params = declaration.params.iter().map(|param| {
        let mut written = String::new();
        for (modifier, word) in [
            (param.r#const, "const "),
            (param.r#in, "in "),
            (param.out, "out "),
        ] {
            if modifier {
                written += word;
            }
        }
        written += param.name.name.as_str();
        if let Some(constraint) = &param.constraint {
            written += &format!(" extends {}", inline(constraint));
        }
        if let Some(default) = &param.default {
            written += &format!(" = {}", inline(default));
        }
        written
    });
    format!("<{}>", params.collect::<Vec<_>>().join(", "))
}

/// The type arguments of `arguments`, as code, like `<string>`, or nothing.
pub fn type_arguments(arguments: Option<&TSTypeParameterInstantiation>) -> String {
    let arguments = arguments.map(|arguments| arguments.params.iter().map(inline));
    match arguments {
        Some(arguments) => format!("<{}>", arguments.collect::<Vec<_>>().join(", ")),
        None => String::new(),
    }
}

/// Each parameter in `params`, after `this`, if a function declares its
/// type, as a signature shows it, like `name?: string`, `{ x, y }: Point`,
/// or `...rest: number[]`, with its default as it's written in `text`.
pub fn parameters(
    text: &str,
    this: Option<&TSThisParameter>,
    params: &FormalParameters,
) -> Vec<String> {
    let this = this.map(|this| match &this.type_annotation {
        Some(annotation) => format!("this: {}", inline(&annotation.type_annotation)),
        None => "this".to_string(),
    });
    let items = params.items.iter().map(|param| parameter(text, param));
    let rest = params.rest.iter().map(|rest| {
        let mut written = format!("...{}", inline(&rest.rest.argument));
        if let Some(annotation) = &rest.type_annotation {
            written += &format!(": {}", inline(&annotation.type_annotation));
        }
        written
    });
    this.into_iter().chain(items).chain(rest).collect()
}

/// `param` as a signature shows it, with its default as it's written in
/// `text`.
fn parameter(text: &str, param: &FormalParameter) -> String {
    let mut written = String::new();
    if let Some(accessibility) = param.accessibility {
        written += &format!("{} ", accessibility.as_str());
    }
    if param.r#override {
        written += "override ";
    }
    if param.readonly {
        written += "readonly ";
    }
    written += &inline(&param.pattern);
    if param.optional {
        written += "?";
    }
    if let Some(annotation) = &param.type_annotation {
        written += &format!(": {}", inline(&annotation.type_annotation));
    }
    if let Some(initializer) = &param.initializer {
        written += &format!(" = {}", self::written(text, initializer.span()));
    }
    written
}

/// A call, like `name(a, b): R`: `head`, then `params` in parentheses,
/// then `tail`, on one line if it fits in `width`, or else with each
/// parameter on a line of its own, and a comma after each but a rest
/// parameter.
pub fn call(head: &str, params: &[String], tail: &str, width: usize) -> String {
    let line = format!("{head}({}){tail}", params.join(", "));
    if params.is_empty() || line.lines().all(|line| line.chars().count() <= width) {
        return line;
    }
    let mut lines: Vec<String> = params.iter().map(|param| format!("  {param},")).collect();
    if let Some(last) = lines.last_mut().filter(|last| last.starts_with("  ...")) {
        last.pop();
    }
    format!("{head}(\n{}\n){tail}", lines.join("\n"))
}

/// The value of `expression`, as it's written in `text`, if it's a
/// literal: a string, a number, a boolean, `null`, `undefined`, a regular
/// expression, a template with nothing in it, or a number with a sign.
pub fn literal(text: &str, expression: &Expression) -> Option<String> {
    let literal = match expression {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::RegExpLiteral(_) => true,
        Expression::TemplateLiteral(template) => template.expressions.is_empty(),
        Expression::UnaryExpression(unary) => {
            matches!(
                unary.operator,
                UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus
            ) && matches!(
                unary.argument,
                Expression::NumericLiteral(_) | Expression::BigIntLiteral(_)
            )
        }
        Expression::Identifier(identifier) => identifier.name.as_str() == "undefined",
        _ => false,
    };
    let span = expression.span();
    literal.then(|| text[span.start as usize..span.end as usize].to_string())
}

/// The name of a property or method with `key`, as its heading shows it,
/// like `size`, or `[Symbol.iterator]`, if `computed`; none for a private
/// one, like `#size`.
pub fn key(text: &str, key: &PropertyKey, computed: bool) -> Option<String> {
    if key.is_private_identifier() {
        return None;
    }
    match (computed, key.static_name()) {
        (false, Some(name)) => Some(name.into_owned()),
        _ => Some(format!("[{}]", written(text, key.span()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_ast::ast::{Declaration, Statement};
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    /// What `check` makes of the first function `text` declares.
    fn function(text: &str, check: impl Fn(&oxc_ast::ast::Function)) {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, text, SourceType::ts()).parse();
        let declared = parsed
            .program
            .body
            .iter()
            .find_map(|statement| match statement {
                Statement::FunctionDeclaration(function) => Some(&**function),
                Statement::ExportDeclaration(export) => match &export.declaration {
                    Declaration::FunctionDeclaration(function) => Some(&**function),
                    _ => None,
                },
                _ => None,
            });
        check(declared.expect("a function"));
    }

    #[test]
    fn parameters_are_written_as_a_signature_shows_them() {
        let text = "function f<const T extends object = {}>(this: Window, \
                    { x, y }: { x: number;\n y: number }, b?: 'a' | 'b', c = {x: 1}, \
                    ...rest: string[]) {}";
        function(text, |function| {
            let declared = type_parameters(function.type_parameters.as_deref());
            assert_eq!(declared, "<const T extends object = {}>");
            let params = parameters(text, function.this_param.as_deref(), &function.params);
            let expected = [
                "this: Window",
                "{ x, y }: { x: number; y: number }",
                "b?: \"a\" | \"b\"",
                "c = {x: 1}",
                "...rest: string[]",
            ];
            assert_eq!(params, expected);
            let long = call("function f", &params, ": void", WIDTH);
            let lines: Vec<&str> = long.lines().collect();
            assert_eq!(lines[0], "function f(");
            assert_eq!(lines[1], "  this: Window,");
            assert_eq!(lines[5], "  ...rest: string[]");
            assert_eq!(lines[6], "): void");
            assert_eq!(call("f", &params[3..4], "", WIDTH), "f(c = {x: 1})");
        });
    }

    #[test]
    fn literals_are_written_as_the_source_writes_them() {
        let text = "f('a', -1, 2n, `t`, `${x}`, undefined, null, /a/g, x, [1])";
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, text, SourceType::ts()).parse();
        let Some(Statement::ExpressionStatement(statement)) = parsed.program.body.first() else {
            panic!("not an expression");
        };
        let Expression::CallExpression(call) = &statement.expression else {
            panic!("not a call");
        };
        let values: Vec<Option<String>> = (call.arguments.iter())
            .map(|argument| literal(text, argument.as_expression().expect("an expression")))
            .collect();
        let written = [
            "'a'",
            "-1",
            "2n",
            "`t`",
            "",
            "undefined",
            "null",
            "/a/g",
            "",
            "",
        ];
        let expected: Vec<Option<String>> = (written.iter())
            .map(|value| Some(value.to_string()).filter(|value| !value.is_empty()))
            .collect();
        assert_eq!(values, expected);
    }

    #[test]
    fn source_starts_at_its_first_line_without_its_margin() {
        let text = "class A {\n    m() {\n      return 1;\n    }\n}";
        let start = text.find("m()").unwrap_or_default() as u32;
        let span = Span::new(start, text.rfind('}').unwrap_or_default() as u32 - 1);
        assert_eq!(source(text, span), "m() {\n  return 1;\n}");
        assert_eq!(indent("a\n\nb", 2), "  a\n\n  b");
    }
}
