//! Highlight each code block as comrak writes it, with a class on each
//! token, like `k` for a keyword, for the theme to colour.

use comrak::adapters::SyntaxHighlighterAdapter;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{self, Write};
use std::str::FromStr;
use std::sync::LazyLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Color, ScopeSelectors, Style, StyleModifier, Theme, ThemeItem};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;
use two_face::re_exports::syntect;

use crate::config::Highlight;

/// Pygments' classes, each for the scopes of bat's grammars nearest the
/// tokens Pygments gives it: `k` for a keyword, `s2` for a string in `"`.
const PYGMENTS: &[(&str, &str)] = &[
    // Comments.
    ("comment", "c"),
    ("comment.line", "c1"),
    ("comment.block", "cm"),
    ("meta.preprocessor", "cp"),
    // Keywords.
    ("keyword", "k"),
    ("keyword.control.import", "kn"),
    ("keyword.other.import", "kn"),
    ("keyword.other.package", "kn"),
    ("keyword.operator.module", "kn"),
    ("keyword.operator", "o"),
    ("keyword.operator.word", "ow"),
    ("storage", "kd"),
    ("storage.type", "kt"),
    ("storage.type.function", "kd"),
    ("storage.type.class", "kd"),
    ("storage.type.struct", "kd"),
    ("storage.type.enum", "kd"),
    ("storage.type.trait", "kd"),
    ("constant.language", "kc"),
    // A table in TOML, as Pygments has it.
    ("entity.name.table", "k"),
    // Strings, and numbers.
    ("string", "s"),
    ("string.quoted.single", "s1"),
    ("string.quoted.double", "s2"),
    ("string.interpolated", "sb"),
    ("keyword.other.template", "sb"),
    ("string.regexp", "sr"),
    ("storage.type.string", "sa"),
    ("markup.raw", "sb"),
    ("constant.character", "sc"),
    ("constant.character.escape", "se"),
    ("constant.other.placeholder", "si"),
    ("meta.interpolation", "si"),
    ("keyword.other.substitution", "si"),
    ("constant.numeric", "m"),
    // Names.
    ("constant.other", "no"),
    ("entity.name.constant", "no"),
    ("support.constant", "no"),
    ("variable.other.constant", "no"),
    ("constant.character.entity", "ni"),
    ("entity.name.function", "nf"),
    ("variable.function", "nf"),
    ("support.macro", "nf"),
    ("entity.name.class", "nc"),
    ("entity.name.enum", "nc"),
    ("entity.name.struct", "nc"),
    ("entity.name.type", "nc"),
    ("entity.other.inherited-class", "nc"),
    ("entity.name.namespace", "nn"),
    ("entity.name.label", "nl"),
    ("entity.name.tag", "nt"),
    // A key in JSON, as Pygments has it.
    ("meta.mapping.key.json string.quoted.double", "nt"),
    ("entity.other.attribute-name", "na"),
    ("support.function", "nb"),
    ("support.type", "nb"),
    ("support.class", "nb"),
    ("variable.language", "bp"),
    ("meta.annotation", "nd"),
    ("meta.decorator", "nd"),
    // Diffs, and markup.
    ("markup.heading", "gh"),
    ("meta.diff.header", "gh"),
    ("meta.diff.range", "gu"),
    ("markup.deleted", "gd"),
    ("meta.diff.header.from-file", "gd"),
    ("markup.inserted", "gi"),
    ("meta.diff.header.to-file", "gi"),
    ("markup.bold", "gs"),
    ("markup.italic", "ge"),
];

/// GitHub's classes, each that github-markdown-css colours on its own, for
/// the scopes it's named for: `pl-k` for `keyword`, `pl-en` for
/// `entity.name`.
const GITHUB: &[(&str, &str)] = &[
    ("comment", "pl-c"),
    ("keyword", "pl-k"),
    ("storage", "pl-k"),
    ("storage.modifier.import", "pl-smi"),
    ("storage.modifier.package", "pl-smi"),
    ("string", "pl-s"),
    ("string.regexp", "pl-sr"),
    ("constant", "pl-c1"),
    ("support", "pl-c1"),
    ("variable.language", "pl-c1"),
    ("variable.other.constant", "pl-c1"),
    ("entity", "pl-e"),
    ("entity.name", "pl-en"),
    ("entity.name.tag", "pl-ent"),
    // A key in JSON, as GitHub colours it.
    ("meta.mapping.key.json string.quoted.double", "pl-ent"),
    ("variable", "pl-v"),
    // Names, like `x`, are plain, as on GitHub.
    ("variable.other", ""),
    ("markup.heading", "pl-mh"),
    ("meta.separator", "pl-ms"),
    ("markup.italic", "pl-mi"),
    ("markup.bold", "pl-mb"),
    ("markup.deleted", "pl-md"),
    ("meta.diff.header.from-file", "pl-md"),
    ("markup.inserted", "pl-mi1"),
    ("meta.diff.header.to-file", "pl-mi1"),
    ("markup.changed", "pl-mc"),
    ("markup.ignored", "pl-mi2"),
    ("meta.diff.range", "pl-mdr"),
    ("constant.other.reference.link", "pl-corl"),
];

/// bat's grammars, read the first time a block in a language is highlighted.
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

static PYGMENTS_CLASSES: LazyLock<Classes> = LazyLock::new(|| Classes::new(PYGMENTS));
static GITHUB_CLASSES: LazyLock<Classes> = LazyLock::new(|| Classes::new(GITHUB));

/// What highlights code blocks with `highlight`'s classes, if it's not
/// `false`.
pub fn highlighter(highlight: Option<Highlight>) -> Option<&'static dyn SyntaxHighlighterAdapter> {
    Some(match highlight? {
        Highlight::Pygments => &*PYGMENTS_CLASSES,
        Highlight::GitHub => &*GITHUB_CLASSES,
    })
}

/// A set of classes, each for the scopes its selector matches, like
/// `keyword`. A token's class is the one whose selector matches its scopes
/// best, as a colour scheme picks a token's colour.
struct Classes {
    classes: &'static [(&'static str, &'static str)],
    /// The classes as a colour scheme, so syntect matches their selectors as
    /// it matches a scheme's: each class's colour is its place in `classes`,
    /// from 1, and a token no selector matches has the scheme's own colour,
    /// black, 0.
    scheme: Theme,
}

impl Classes {
    fn new(classes: &'static [(&'static str, &'static str)]) -> Self {
        let scopes = classes.iter().enumerate().map(|(index, (selector, _))| {
            let index = u8::try_from(index + 1).expect("fewer than 256 classes");
            ThemeItem {
                scope: ScopeSelectors::from_str(selector).expect("a scope selector"),
                style: StyleModifier {
                    foreground: Some(Color {
                        r: index,
                        g: 0,
                        b: 0,
                        a: 0xff,
                    }),
                    ..StyleModifier::default()
                },
            }
        });
        let scheme = Theme {
            scopes: scopes.collect(),
            ..Theme::default()
        };
        Classes { classes, scheme }
    }

    /// The class of a token with `style`, if it has one.
    fn class(&self, style: Style) -> Option<&'static str> {
        let index = usize::from(style.foreground.r).checked_sub(1)?;
        Some(self.classes[index].1).filter(|class| !class.is_empty())
    }

    /// Each token of `code`, in the language of `syntax`, with its class, if
    /// it has one. A line's end is a token of its own, without a class.
    fn tokens<'c>(
        &self,
        syntax: &SyntaxReference,
        code: &'c str,
    ) -> Result<Vec<(Option<&'static str>, &'c str)>, syntect::Error> {
        let mut highlighter = HighlightLines::new(syntax, &self.scheme);
        let mut tokens = Vec::new();
        for line in LinesWithEndings::from(code) {
            let text = line.trim_end_matches(['\r', '\n']).len();
            let mut start = 0;
            for (style, token) in highlighter.highlight_line(line, &SYNTAXES)? {
                let end = (start + token.len()).min(text);
                if start < end {
                    tokens.push((self.class(style), &line[start..end]));
                }
                start += token.len();
            }
            tokens.push((None, &line[text..]));
        }
        Ok(tokens)
    }
}

impl SyntaxHighlighterAdapter for Classes {
    fn write_highlighted(
        &self,
        out: &mut dyn Write,
        lang: Option<&str>,
        code: &str,
    ) -> fmt::Result {
        // A grammar can fail on code it can't read, which is left plain.
        let tokens = syntax(lang.unwrap_or_default())
            .and_then(|syntax| self.tokens(syntax, code).ok())
            .unwrap_or_else(|| vec![(None, code)]);
        write_spans(out, &tokens)
    }

    fn write_pre_tag(
        &self,
        out: &mut dyn Write,
        mut attributes: HashMap<&'static str, Cow<'_, str>>,
    ) -> fmt::Result {
        attributes.insert("class", "highlight".into());
        comrak::html::write_opening_tag(out, "pre", sorted(attributes))
    }

    fn write_code_tag(
        &self,
        out: &mut dyn Write,
        attributes: HashMap<&'static str, Cow<'_, str>>,
    ) -> fmt::Result {
        comrak::html::write_opening_tag(out, "code", sorted(attributes))
    }
}

/// A tag's attributes, in the same order each build.
fn sorted<'a>(
    attributes: HashMap<&'static str, Cow<'a, str>>,
) -> BTreeMap<&'static str, Cow<'a, str>> {
    attributes.into_iter().collect()
}

/// The grammar for a code block in `lang`, the first word after its fence:
/// by the grammar's name or its files' extension, in any case, up to a comma,
/// like `rust` in `rust,ignore`, or by a name GitHub knows for it.
fn syntax(lang: &str) -> Option<&'static SyntaxReference> {
    let name = lang.split(',').next().unwrap_or_default().to_lowercase();
    let name = match name.as_str() {
        "" => return None,
        "shell" => "bash",
        "golang" => "go",
        "csharp" => "cs",
        "jsonc" => "json",
        "docker" => "dockerfile",
        "pwsh" => "powershell",
        name => name,
    };
    SYNTAXES.find_syntax_by_token(name)
}

/// Write tokens as HTML: a `<span>` with its class around each run of tokens
/// with the same class, as Pygments writes them. Spaces stay in the span
/// before them if they share its class, but don't start one, and a line's
/// end, which has no class, ends the span before it.
fn write_spans(out: &mut dyn Write, tokens: &[(Option<&str>, &str)]) -> fmt::Result {
    let mut open = None;
    for &(class, text) in tokens {
        let class = if class != open && text.trim().is_empty() {
            None
        } else {
            class
        };
        if class != open {
            if open.is_some() {
                out.write_str("</span>")?;
            }
            if let Some(class) = class {
                write!(out, "<span class=\"{class}\">")?;
            }
            open = class;
        }
        comrak::html::escape(out, text)?;
    }
    if open.is_some() {
        out.write_str("</span>")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn highlight(classes: Highlight, lang: &str, code: &str) -> String {
        let mut html = String::new();
        let highlighter = highlighter(Some(classes)).unwrap();
        highlighter
            .write_highlighted(&mut html, Some(lang), code)
            .unwrap();
        html
    }

    const RUST: &str = "fn main() {\n    let s = \"a b\"; // c\n}\n";

    #[test]
    fn pygments_classes_mark_each_token() {
        assert_eq!(
            highlight(Highlight::Pygments, "rust", RUST),
            "<span class=\"kd\">fn</span> <span class=\"nf\">main</span>() {\n    \
             <span class=\"kt\">let</span> s <span class=\"o\">=</span> \
             <span class=\"s2\">&quot;a b&quot;</span>; <span class=\"c1\">// c</span>\n}\n"
        );
    }

    #[test]
    fn github_classes_mark_each_token() {
        assert_eq!(
            highlight(Highlight::GitHub, "rust", RUST),
            "<span class=\"pl-k\">fn</span> <span class=\"pl-en\">main</span>() {\n    \
             <span class=\"pl-k\">let</span> s <span class=\"pl-k\">=</span> \
             <span class=\"pl-s\">&quot;a b&quot;</span>; <span class=\"pl-c\">// c</span>\n}\n"
        );
    }

    #[test]
    fn a_selector_can_match_a_token_in_its_context() {
        // A key in JSON is a string to its grammar, but a tag to Pygments.
        let json = "{\"a\": \"b\"}\n";
        assert_eq!(
            highlight(Highlight::Pygments, "json", json),
            "{<span class=\"nt\">&quot;a&quot;</span>: <span class=\"s2\">&quot;b&quot;</span>}\n"
        );
        assert_eq!(
            highlight(Highlight::GitHub, "json", json),
            "{<span class=\"pl-ent\">&quot;a&quot;</span>: <span class=\"pl-s\">&quot;b&quot;</span>}\n"
        );
    }

    #[test]
    fn languages_are_named_as_on_github() {
        let name = |lang| syntax(lang).map(|syntax| syntax.name.as_str());
        for lang in ["rust", "Rust", "RS", "rust,ignore"] {
            assert_eq!(name(lang), Some("Rust"), "{lang}");
        }
        for (lang, grammar) in [
            ("shell", "bash"),
            ("golang", "go"),
            ("csharp", "cs"),
            ("jsonc", "json"),
            ("docker", "dockerfile"),
            ("pwsh", "powershell"),
        ] {
            assert!(name(lang).is_some(), "{lang}");
            assert_eq!(name(lang), name(grammar), "{lang}");
        }
        for lang in ["", ",rust", "mermaid", "console", "text"] {
            assert_eq!(name(lang), None, "{lang}");
        }
    }

    #[test]
    fn code_in_a_language_little_doesnt_know_is_plain() {
        for lang in ["", "mermaid"] {
            let html = highlight(Highlight::Pygments, lang, "A-->B & \"C\"\n");
            assert_eq!(html, "A--&gt;B &amp; &quot;C&quot;\n", "{lang}");
        }
    }

    #[test]
    fn highlighted_code_is_escaped() {
        assert_eq!(
            highlight(Highlight::Pygments, "html", "<b>&amp;</b>\n"),
            "&lt;<span class=\"nt\">b</span>&gt;<span class=\"ni\">&amp;amp;</span>\
             &lt;/<span class=\"nt\">b</span>&gt;\n"
        );
    }

    #[test]
    fn a_span_never_holds_the_end_of_a_line() {
        for end in ["\n", "\r\n"] {
            let code = format!("/* a{end}b */{end}");
            assert_eq!(
                highlight(Highlight::Pygments, "c", &code),
                format!("<span class=\"cm\">/* a</span>{end}<span class=\"cm\">b */</span>{end}")
            );
        }
    }

    #[test]
    fn tokens_with_a_class_in_a_row_share_a_span_and_spaces_dont_start_one() {
        let tokens = [
            (Some("k"), "let"),
            (Some("s"), " "),
            (Some("s"), "x"),
            (Some("s"), " "),
            (Some("s"), "y"),
            (None, " "),
            (None, "z"),
            (None, "\n"),
        ];
        let mut html = String::new();
        write_spans(&mut html, &tokens).unwrap();
        assert_eq!(
            html,
            "<span class=\"k\">let</span> <span class=\"s\">x y</span> z\n"
        );
    }

    #[test]
    fn pre_has_the_highlight_class() {
        let highlighter = highlighter(Some(Highlight::Pygments)).unwrap();
        let mut html = String::new();
        let attributes = HashMap::from([("data-x", "1".into())]);
        highlighter.write_pre_tag(&mut html, attributes).unwrap();
        let attributes = HashMap::from([("class", "language-rust".into())]);
        highlighter.write_code_tag(&mut html, attributes).unwrap();
        assert_eq!(
            html,
            "<pre class=\"highlight\" data-x=\"1\"><code class=\"language-rust\">"
        );
    }
}
