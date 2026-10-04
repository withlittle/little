//! Write each formula in a page as MathML as comrak writes the page, with
//! math-core: a `<math>` for one in `$…$` or `` $`…`$ ``, and a
//! `<math display="block">` for one in `$$…$$` or a ```` ```math ```` block.
//! Each is read on its own, so a command one defines isn't in the next.

use comrak::Options;
use comrak::html::{self, ChildRendering, Context};
use comrak::nodes::{AstNode, NodeValue};
use comrak::options::Plugins;
use math_core::{LatexToMathML, MathDisplay};
use std::fmt::{self, Write};

/// A formula math-core can't read, which is written as its TeX instead, as
/// with `markdown.math: tex`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unrendered {
    /// As it's written, on one line, like `$\dv{f}{x}$`, with a ```` ```math ````
    /// block's in `$$`.
    pub written: String,
    /// Why, as math-core says, like `Unknown command "\dv".`
    pub reason: String,
}

/// Write the document at `root` to `output`, as comrak does with `options`
/// and `plugins`, but with its math as MathML. Returns each formula that
/// couldn't be.
pub fn format_document<'a>(
    root: &'a AstNode<'a>,
    options: &Options,
    plugins: &Plugins,
    output: &mut String,
) -> Result<Vec<Unrendered>, fmt::Error> {
    let writer = Writer {
        converter: LatexToMathML::default(),
        unrendered: Vec::new(),
    };
    let writer =
        html::format_document_with_formatter(root, options, output, plugins, format_node, writer)?;
    Ok(writer.unrendered)
}

/// What `format_node` keeps as it writes a document.
struct Writer {
    converter: LatexToMathML,
    unrendered: Vec<Unrendered>,
}

/// Write `node` as comrak does, but math as MathML.
fn format_node<'a>(
    context: &mut Context<Writer>,
    node: &'a AstNode<'a>,
    entering: bool,
) -> Result<ChildRendering, fmt::Error> {
    let data = node.data();
    let (tex, display, delimiter) = match &data.value {
        NodeValue::Math(math) if entering => match (math.dollar_math, math.display_math) {
            (false, _) => (&math.literal, MathDisplay::Inline, ("$`", "`$")),
            (true, false) => (&math.literal, MathDisplay::Inline, ("$", "$")),
            (true, true) => (&math.literal, MathDisplay::Block, ("$$", "$$")),
        },
        // As comrak, a block is math by its language alone.
        NodeValue::CodeBlock(block)
            if entering && block.info.split_ascii_whitespace().next() == Some("math") =>
        {
            (&block.literal, MathDisplay::Block, ("$$", "$$"))
        }
        _ => return html::format_node_default(context, node, entering),
    };
    match context
        .user
        .converter
        .convert_with_local_state(tex, display)
    {
        Ok(converted) => {
            let block = matches!(data.value, NodeValue::CodeBlock(_));
            if block {
                context.cr()?;
            }
            context.write_str(&converted.mathml)?;
            if block {
                context.lf()?;
            }
            Ok(ChildRendering::HTML)
        }
        Err(error) => {
            let (open, close) = delimiter;
            let tex = tex.split_whitespace().collect::<Vec<_>>().join(" ");
            context.user.unrendered.push(Unrendered {
                written: format!("{open}{tex}{close}"),
                reason: error.to_string(),
            });
            html::format_node_default(context, node, entering)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Markdown, SiteUrl};
    use crate::render::{Rendered, parse_document};

    /// Render `md` with little's defaults, which write math as MathML.
    fn render(md: &str) -> Rendered {
        parse_document(md, "/", &(2..=6), &SiteUrl::default(), &Markdown::default()).unwrap()
    }

    const X_SQUARED: &str = "<msup><mi>x</mi><mn>2</mn></msup>";

    #[test]
    fn each_formula_is_mathml() {
        let r = render("$x^2$, $`x^2`$, and $a < b$\n\n$$x^2$$\n\n```math\nx^2\n```\n");
        let expected = format!(
            "<p><math>{X_SQUARED}</math>, <math>{X_SQUARED}</math>, and \
             <math><mi>a</mi><mo>&lt;</mo><mi>b</mi></math></p>\n\
             <p><math display=\"block\">{X_SQUARED}</math></p>\n\
             <math display=\"block\">{X_SQUARED}</math>\n"
        );
        assert_eq!(r.html, expected);
        assert_eq!(r.unrendered_math, []);
    }

    #[test]
    fn a_formula_that_cant_be_read_is_written_as_tex() {
        let r = render(
            "$\\dv{f}{x}$ and $`\\dv{f}{x}`$\n\n$$\n\\dv{f}{x}\n  \\cdot y\n$$\n\n\
             ```math\n\\dv{f}{x}\n```\n",
        );
        let expected = "<p><span data-math-style=\"inline\">\\dv{f}{x}</span> and \
                        <code data-math-style=\"inline\">\\dv{f}{x}</code></p>\n\
                        <p><span data-math-style=\"display\">\n\\dv{f}{x}\n\\cdot y\n</span></p>\n\
                        <pre><code class=\"language-math\" data-math-style=\"display\">\
                        \\dv{f}{x}\n</code></pre>\n";
        assert_eq!(r.html, expected);
        // Each is noted as it's written, on one line, and a block in `$$`.
        let unknown = |written: &str| Unrendered {
            written: written.to_string(),
            reason: "Unknown command \"\\dv\".".to_string(),
        };
        let unrendered = [
            unknown("$\\dv{f}{x}$"),
            unknown("$`\\dv{f}{x}`$"),
            unknown("$$\\dv{f}{x} \\cdot y$$"),
            unknown("$$\\dv{f}{x}$$"),
        ];
        assert_eq!(r.unrendered_math, unrendered);
    }

    #[test]
    fn each_formula_is_read_on_its_own() {
        let r = render("$\\newcommand{\\half}{\\frac12}\\half$ and $\\half$\n");
        assert!(r.html.starts_with("<p><math><mfrac>"), "{}", r.html);
        let unrendered = Unrendered {
            written: "$\\half$".to_string(),
            reason: "Unknown command \"\\half\".".to_string(),
        };
        assert_eq!(r.unrendered_math, [unrendered]);
    }

    #[test]
    fn a_headings_math_is_mathml_but_text_elsewhere() {
        let r = render("## The $x^2$ term\n\n![$x^2$](plot.png)\n");
        // Its id, and the text a screen reader reads for its link, are TeX.
        let heading = format!(
            "<h2 id=\"the-x2-term\">The <math>{X_SQUARED}</math> term<a href=\"#the-x2-term\" \
             aria-label=\"Link to heading 'The x^2 term'\" \
             data-heading-content=\"The x^2 term\" class=\"anchor\"></a></h2>\n"
        );
        assert!(r.html.starts_with(&heading), "{}", r.html);
        // As is an image's alt text.
        assert!(r.html.contains("alt=\"x^2\""), "{}", r.html);
    }
}
