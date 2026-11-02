//! Directives, `.. name:: arguments`, with their options and content, read
//! as docutils reads them: each the plugin knows into blocks, or the text of
//! a substitution, and any other kept as code, and warned of.

use little_markup::AlertKind;

use crate::Clock;
use crate::body::{Body, code, nested};
use crate::explicit::{parse_target, simplename, uri};
use crate::lines::{Line, indented, is_space, join, trim_end};
use crate::lists::field_marker;
use crate::state::{Dest, Role, Substitution};
use crate::tables;
use crate::text::{collapse, escape, make_id, normalize, unescape};
use crate::tree::{Node, Ref, Span};

/// What a directive takes, as its class in docutils declares it.
#[derive(Debug, Clone, Copy)]
struct Spec {
    /// How many arguments it needs, and how many more it may have.
    required: usize,
    optional: usize,
    /// Whether its last argument may hold whitespace.
    whitespace: bool,
    /// The options it may have.
    options: &'static [&'static str],
    /// Whether it may have content.
    content: bool,
}

const fn spec(
    required: usize,
    optional: usize,
    whitespace: bool,
    options: &'static [&'static str],
    content: bool,
) -> Spec {
    Spec {
        required,
        optional,
        whitespace,
        options,
        content,
    }
}

const CLASS_NAME: &[&str] = &["class", "name"];
const IMAGE: &[&str] = &[
    "alt", "height", "width", "scale", "align", "target", "loading", "class", "name",
];
const FIGURE: &[&str] = &[
    "alt", "height", "width", "scale", "align", "target", "loading", "class", "name", "figwidth",
    "figclass", "figname",
];
const LIST_TABLE: &[&str] = &[
    "header-rows",
    "stub-columns",
    "width",
    "widths",
    "class",
    "name",
    "align",
];
const CSV_TABLE: &[&str] = &[
    "header-rows",
    "stub-columns",
    "header",
    "width",
    "widths",
    "file",
    "url",
    "encoding",
    "class",
    "name",
    "align",
    "delim",
    "keepspace",
    "quote",
    "escape",
];
const INCLUDE: &[&str] = &[
    "literal",
    "code",
    "encoding",
    "parser",
    "tab-width",
    "start-line",
    "end-line",
    "start-after",
    "end-before",
    "number-lines",
    "class",
    "name",
];

/// The directives docutils has, by their names, and what each takes.
const DIRECTIVES: &[(&str, Spec)] = &[
    ("admonition", spec(1, 0, true, CLASS_NAME, true)),
    ("attention", spec(0, 0, false, CLASS_NAME, true)),
    ("caution", spec(0, 0, false, CLASS_NAME, true)),
    ("danger", spec(0, 0, false, CLASS_NAME, true)),
    ("error", spec(0, 0, false, CLASS_NAME, true)),
    ("hint", spec(0, 0, false, CLASS_NAME, true)),
    ("important", spec(0, 0, false, CLASS_NAME, true)),
    ("note", spec(0, 0, false, CLASS_NAME, true)),
    ("tip", spec(0, 0, false, CLASS_NAME, true)),
    ("warning", spec(0, 0, false, CLASS_NAME, true)),
    ("topic", spec(1, 0, true, CLASS_NAME, true)),
    (
        "sidebar",
        spec(0, 1, true, &["class", "name", "subtitle"], true),
    ),
    ("rubric", spec(1, 0, true, CLASS_NAME, false)),
    ("epigraph", spec(0, 0, false, &[], true)),
    ("highlights", spec(0, 0, false, &[], true)),
    ("pull-quote", spec(0, 0, false, &[], true)),
    ("compound", spec(0, 0, false, CLASS_NAME, true)),
    ("container", spec(0, 1, true, &["name"], true)),
    ("class", spec(1, 0, true, &[], true)),
    ("line-block", spec(0, 0, false, CLASS_NAME, true)),
    ("parsed-literal", spec(0, 0, false, CLASS_NAME, true)),
    (
        "code",
        spec(0, 1, false, &["class", "name", "number-lines"], true),
    ),
    ("math", spec(0, 0, false, CLASS_NAME, true)),
    ("image", spec(1, 0, true, IMAGE, false)),
    ("figure", spec(1, 0, true, FIGURE, true)),
    (
        "table",
        spec(
            0,
            1,
            true,
            &["class", "name", "align", "width", "widths"],
            true,
        ),
    ),
    ("list-table", spec(0, 1, true, LIST_TABLE, true)),
    ("csv-table", spec(0, 1, true, CSV_TABLE, true)),
    ("title", spec(1, 0, true, &[], false)),
    ("meta", spec(0, 0, false, &[], true)),
    ("role", spec(0, 0, false, &[], true)),
    ("default-role", spec(0, 1, false, &[], false)),
    ("include", spec(1, 0, true, INCLUDE, false)),
    (
        "sectnum",
        spec(0, 0, false, &["depth", "start", "prefix", "suffix"], false),
    ),
    ("header", spec(0, 0, false, &[], true)),
    ("footer", spec(0, 0, false, &[], true)),
    ("target-notes", spec(0, 0, false, &["class"], false)),
    (
        "contents",
        spec(0, 1, true, &["depth", "local", "backlinks", "class"], false),
    ),
    ("replace", spec(0, 0, false, &[], true)),
    (
        "unicode",
        spec(1, 0, true, &["trim", "ltrim", "rtrim"], false),
    ),
    ("date", spec(0, 0, false, &[], true)),
    (
        "raw",
        spec(1, 0, true, &["file", "url", "encoding", "class"], true),
    ),
];

/// The directive `name`, in lowercase, by its name in [`DIRECTIVES`],
/// through docutils' aliases for it, and what it takes.
fn lookup(name: &str) -> Option<(&'static str, Spec)> {
    let name = match name {
        "code-block" | "sourcecode" => "code",
        "rst-class" => "class",
        "section-numbering" => "sectnum",
        name => name,
    };
    DIRECTIVES.iter().copied().find(|(known, _)| *known == name)
}

/// A directive, as it's read.
struct Directive<'a> {
    /// Its name, as it's written.
    name: &'a str,
    /// Its first line.
    line: usize,
    /// The name of the substitution it defines, if it's in a substitution
    /// definition.
    alt: Option<&'a str>,
    arguments: Vec<String>,
    /// Its options, by their names, in lowercase, and their values.
    options: Vec<(String, String)>,
    content: Vec<Line>,
}

impl Directive<'_> {
    /// The value of the option `name`, if the directive has it.
    fn option(&self, name: &str) -> Option<&str> {
        let option = self.options.iter().find(|(key, _)| key == name);
        option.map(|(_, value)| value.as_str())
    }

    /// Its first argument, if it has one.
    fn argument(&self) -> Option<&str> {
        self.arguments.first().map(String::as_str)
    }

    /// An error, unless it has content, as docutils' `assert_has_content`
    /// gives.
    fn has_content(&self) -> Result<(), String> {
        if self.content.is_empty() {
            return Err("content block expected; none found".to_string());
        }
        Ok(())
    }
}

/// A directive's arguments, options, and content.
type Parts = (Vec<String>, Vec<(String, String)>, Vec<Line>);

/// The arguments, options, and content of a directive that takes `spec`,
/// from `block`, its lines from the end of its name, as docutils'
/// `parse_directive_block` tells them apart.
fn parse_block(mut block: Vec<Line>, spec: &Spec) -> Result<Parts, String> {
    if block.first().is_some_and(|line| line.text.is_empty()) {
        block.remove(0);
    }
    trim_end(&mut block);
    let takes_arguments = spec.required + spec.optional > 0;
    let split = takes_arguments || !spec.options.is_empty();
    let blank = if split {
        block
            .iter()
            .position(|line| line.text.is_empty())
            .unwrap_or(block.len())
    } else {
        0
    };
    let mut head = block[..blank].to_vec();
    let mut content = if split {
        block.get(blank + 1..).unwrap_or_default().to_vec()
    } else {
        block.clone()
    };
    let mut options = Vec::new();
    if !spec.options.is_empty() {
        let fields = head
            .iter()
            .position(|line| field_marker(&line.text).is_some());
        let fields = fields.unwrap_or(head.len());
        options = parse_options(&head[fields..], spec.options)?;
        head.truncate(fields);
    }
    if !head.is_empty() && !takes_arguments {
        head.extend_from_slice(&block[blank..]);
        content = std::mem::take(&mut head);
    }
    let leading = content
        .iter()
        .take_while(|line| line.text.is_empty())
        .count();
    content.drain(..leading);
    let arguments = if takes_arguments {
        arguments(&join(&head), spec)?
    } else {
        Vec::new()
    };
    if !content.is_empty() && !spec.content {
        return Err("no content permitted".to_string());
    }
    Ok((arguments, options, content))
}

/// The arguments in `text`, split at whitespace, but for the last, which
/// keeps it, if the directive's spec lets it, as docutils splits them.
fn arguments(text: &str, spec: &Spec) -> Result<Vec<String>, String> {
    let words: Vec<&str> = text
        .split(is_space)
        .filter(|word| !word.is_empty())
        .collect();
    let most = spec.required + spec.optional;
    if words.len() < spec.required {
        let (required, supplied) = (spec.required, words.len());
        return Err(format!(
            "{required} argument(s) required, {supplied} supplied"
        ));
    }
    if words.len() <= most {
        return Ok(words.into_iter().map(String::from).collect());
    }
    if !spec.whitespace {
        let supplied = words.len();
        return Err(format!(
            "maximum {most} argument(s) allowed, {supplied} supplied"
        ));
    }
    let mut rest = text.trim_start_matches(is_space);
    let mut arguments = Vec::new();
    for _ in 1..most {
        let end = rest.find(is_space).unwrap_or(rest.len());
        arguments.push(rest[..end].to_string());
        rest = rest[end..].trim_start_matches(is_space);
    }
    arguments.push(rest.to_string());
    Ok(arguments)
}

/// The options in `block`, each a field, `:name: value`, and each one of
/// `known`, in order, as docutils' `parse_extension_options` reads them.
fn parse_options(block: &[Line], known: &[&str]) -> Result<Vec<(String, String)>, String> {
    let mut fields = Vec::new();
    let mut at = 0;
    while at < block.len() {
        let Some((raw, start)) = field_marker(&block[at].text) else {
            return Err("invalid option block".to_string());
        };
        let body = indented(block, at, false, None, Some(start));
        at = body.end;
        let value = (!body.lines.is_empty()).then(|| join(&body.lines));
        fields.push((unescape(&escape(&raw)), value));
    }
    for (name, _) in &fields {
        if name.split(is_space).filter(|word| !word.is_empty()).count() != 1 {
            return Err(format!("option name \"{name}\" must be a single word"));
        }
    }
    let mut options: Vec<(String, String)> = Vec::new();
    for (name, value) in fields {
        let name = name.to_lowercase();
        if !known.contains(&name.as_str()) {
            return Err(format!("unknown option: \"{name}\""));
        }
        if options.iter().any(|(key, _)| *key == name) {
            return Err(format!("duplicate option \"{name}\""));
        }
        let value = convert(&name, value)?;
        options.push((name, value));
    }
    Ok(options)
}

/// The value of the option `name`, as docutils' function for it converts
/// it, for the options whose values the plugin reads, or that docutils
/// refuses: a flag takes none, and most other options need one.
fn convert(name: &str, value: Option<String>) -> Result<String, String> {
    const FLAGS: &[&str] = &["local", "literal", "keepspace", "trim", "ltrim", "rtrim"];
    const OPTIONAL: &[&str] = &[
        "alt",
        "name",
        "header",
        "figname",
        "code",
        "start-after",
        "end-before",
        "parser",
        "number-lines",
        "format",
        "language",
    ];
    if FLAGS.contains(&name) {
        return match value {
            Some(value) if !value.trim_matches(is_space).is_empty() => {
                Err(format!("option \"{name}\" takes no value"))
            }
            _ => Ok(String::new()),
        };
    }
    let Some(value) = value else {
        if OPTIONAL.contains(&name) {
            return Ok(String::new());
        }
        return Err(format!("option \"{name}\" needs a value"));
    };
    match name {
        "header-rows" | "stub-columns" => match value.trim_matches(is_space).parse::<i64>() {
            Ok(number) if number >= 0 => Ok(number.to_string()),
            _ => Err(format!(
                "option \"{name}\" must be zero or a positive integer, not \"{value}\""
            )),
        },
        "number-lines" if !value.is_empty() => match value.trim_matches(is_space).parse::<i64>() {
            Ok(number) => Ok(number.to_string()),
            Err(_) => Err(format!(
                "option \"{name}\" must be an integer, not \"{value}\""
            )),
        },
        "delim" | "quote" | "escape" => {
            let character = match value.as_str() {
                "tab" if name == "delim" => Some("\t".to_string()),
                "space" if name == "delim" => Some(" ".to_string()),
                value => unicode_code(value),
            };
            match character {
                Some(character) if character.chars().count() == 1 => Ok(character),
                _ => Err(format!(
                    "option \"{name}\" must be a single character or a Unicode code, not \
                     \"{value}\""
                )),
            }
        }
        _ => Ok(value),
    }
}

/// The character `code` stands for, as docutils' `unicode_code` reads it: a
/// decimal number; a hexadecimal one, after `0x`, `x`, `\x`, `U+`, `u` or
/// `\u`, or as `&#x...;`; or else `code` itself. None, if the number isn't
/// a Unicode character.
fn unicode_code(code: &str) -> Option<String> {
    let decimal = !code.is_empty() && code.bytes().all(|b| b.is_ascii_digit());
    if decimal {
        let number = code.parse::<u32>().ok()?;
        return char::from_u32(number).map(String::from);
    }
    let lower = code.to_ascii_lowercase();
    let hex = |digits: &str| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_hexdigit());
    let entity = lower
        .strip_prefix("&#x")
        .and_then(|rest| rest.strip_suffix(';'));
    let digits = ["0x", "x", "\\x", "u+", "u", "\\u"]
        .iter()
        .filter_map(|prefix| lower.strip_prefix(prefix))
        .chain(entity)
        .find(|digits| hex(digits));
    match digits {
        Some(digits) => {
            let number = u32::from_str_radix(digits, 16).ok()?;
            char::from_u32(number).map(String::from)
        }
        None => Some(code.to_string()),
    }
}

impl Body<'_, '_> {
    /// The directive `name`, whose arguments start after the first `start`
    /// characters of the line `self.at`, in the definition of the
    /// substitution `alt`, if it's in one: the substitution it gives, if it
    /// gives one, and whether it ends with a blank line. Its blocks go in
    /// `self.nodes`; one the plugin doesn't know, or that docutils reports
    /// an error in, is kept as code, as docutils shows it, and warned of.
    pub fn directive(
        &mut self,
        name: &str,
        start: usize,
        alt: Option<&str>,
    ) -> (Option<Substitution>, bool) {
        let lines = self.lines;
        let at = self.at;
        let line = lines[at].num;
        let block = indented(lines, at, false, None, Some(start));
        self.at = block.end;
        let source = || {
            let mut source = lines[at..block.end].to_vec();
            trim_end(&mut source);
            Node::Problem(vec![code(join(&source))])
        };
        let Some((known, spec)) = lookup(&name.to_lowercase()) else {
            let message = format!(
                "kept `.. {name}::` as code, as `{name}` isn't a directive the plugin reads"
            );
            self.state.warn(line, message);
            self.nodes.push(source());
            return (None, block.blank_finish);
        };
        // docutils keeps the blank lines at the top, and takes off one.
        let blank = Line {
            text: String::new(),
            num: line,
        };
        let mut all = vec![blank; block.skipped.min(2)];
        all.extend(block.lines);
        let read = parse_block(all, &spec).and_then(|(arguments, options, content)| {
            let directive = Directive {
                name,
                line,
                alt,
                arguments,
                options,
                content,
            };
            self.run_directive(known, &directive)
        });
        match read {
            Ok(substitution) => (substitution, block.blank_finish),
            Err(detail) => {
                self.state
                    .warn(line, format!("kept `.. {name}::` as code: {detail}"));
                self.nodes.push(source());
                (None, block.blank_finish)
            }
        }
    }

    /// Run the directive `d`, known as `known`: the substitution it gives,
    /// in a substitution definition, or an error, as docutils reports it.
    fn run_directive(
        &mut self,
        known: &str,
        d: &Directive,
    ) -> Result<Option<Substitution>, String> {
        let only_in_substitutions = matches!(known, "replace" | "unicode" | "date");
        if only_in_substitutions && d.alt.is_none() {
            return Err("it can only be used within a substitution definition".to_string());
        }
        let nodes = match known {
            "image" => return self.image(d),
            "replace" => return self.replace(d),
            "unicode" => return unicode(d),
            "date" => return Ok(Some(self.date(d))),
            "raw" => return self.raw(d),
            "admonition" | "attention" | "caution" | "danger" | "error" | "hint" | "important"
            | "note" | "tip" | "warning" => self.admonition(known, d)?,
            "topic" => self.topic(d)?,
            "sidebar" => self.sidebar(d)?,
            "rubric" => {
                let title = self.inline(d.argument().unwrap_or_default(), d.line);
                let mut nodes = self.anchor(d, "rubric");
                nodes.push(Node::Para(vec![Span::Strong(title)]));
                nodes
            }
            "epigraph" | "highlights" | "pull-quote" => {
                d.has_content()?;
                self.quotes(&d.content)
            }
            "compound" => {
                d.has_content()?;
                let mut nodes = self.anchor(d, "compound");
                nodes.push(Node::Group(nested(self.state, &d.content)));
                nodes
            }
            "container" => {
                d.has_content()?;
                class_check(d.argument().unwrap_or_default())?;
                let mut nodes = self.anchor(d, "container");
                nodes.push(Node::Group(nested(self.state, &d.content)));
                nodes
            }
            "class" => {
                class_check(d.argument().unwrap_or_default())?;
                if d.content.is_empty() {
                    vec![Node::Nothing { prebib: true }]
                } else {
                    self.loose(&d.content, false)
                }
            }
            "line-block" => self.line_block_directive(d)?,
            "parsed-literal" => {
                d.has_content()?;
                let spans = self.inline(&join(&d.content), d.content[0].num);
                let mut nodes = self.anchor(d, "literal-block");
                nodes.push(Node::Literal(spans, d.line));
                nodes
            }
            "code" => {
                d.has_content()?;
                let mut nodes = self.anchor(d, "literal-block");
                nodes.push(Node::Code {
                    language: d.argument().unwrap_or_default().to_string(),
                    text: join(&d.content),
                });
                nodes
            }
            "math" => self.math(d)?,
            "figure" => self.figure(d)?,
            "table" => self.table_directive(d)?,
            "list-table" => self.list_table(d)?,
            "csv-table" => self.csv_table(d)?,
            "title" => {
                let title = collapse(d.argument().unwrap_or_default());
                self.state.metadata.retain(|(key, _)| key != "title");
                self.state.metadata.push(("title".to_string(), title));
                Vec::new()
            }
            "meta" => {
                d.has_content()?;
                self.meta(d);
                Vec::new()
            }
            "role" => {
                self.role(d)?;
                Vec::new()
            }
            "default-role" => {
                self.default_role(d)?;
                Vec::new()
            }
            "contents" => self.contents(d),
            _ => {
                if matches!(known, "header" | "footer") {
                    d.has_content()?;
                }
                let why = match known {
                    "include" => "the plugin doesn't follow includes",
                    "sectnum" => "the plugin doesn't number sections",
                    "header" | "footer" => "the theme writes the page's header and footer",
                    _ => "the plugin doesn't list the targets' URLs",
                };
                let what = match known {
                    "include" => format!("`.. include:: {}`", d.argument().unwrap_or_default()),
                    _ => format!("`.. {}::`", d.name),
                };
                self.state
                    .warn(d.line, format!("left out {what}, as {why}"));
                // docutils keeps a header and a footer apart from the body.
                if matches!(known, "header" | "footer") {
                    Vec::new()
                } else {
                    vec![Node::Nothing { prebib: true }]
                }
            }
        };
        self.nodes.extend(nodes);
        Ok(None)
    }

    /// The anchor of the directive's `:name:`, if it has one, outside a
    /// substitution definition: a target here, with an id made from the
    /// name, else from `prefix`, as docutils makes it.
    fn anchor(&mut self, d: &Directive, prefix: &str) -> Vec<Node> {
        self.named(d, "name", prefix)
    }

    /// The `contents` directive, left out, as the theme writes the page's
    /// table of contents, but for an anchor where it would be, named by its
    /// title, as docutils names it, unless something has that name already,
    /// so a link to it still leads there.
    fn contents(&mut self, d: &Directive) -> Vec<Node> {
        let title = match d.argument() {
            Some(title) => Span::text(&self.inline(title, d.line)),
            None => "Contents".to_string(),
        };
        let name = normalize(&title);
        let mut nodes = Vec::new();
        if !name.is_empty() && !self.state.names.contains_key(&name) {
            let index = self
                .state
                .target(Dest::Here, &[name], false, "contents", d.line);
            nodes.push(Node::Anchor(self.state.targets[index].id.clone()));
        }
        // Unlike its anchor, the table of contents doesn't come before a
        // title, or docinfo.
        nodes.push(Node::Nothing { prebib: false });
        nodes
    }

    /// The anchor of the directive's option `option`, which names it, as
    /// [`Body::anchor`] has it.
    fn named(&mut self, d: &Directive, option: &str, prefix: &str) -> Vec<Node> {
        let name = d
            .option(option)
            .map(normalize)
            .filter(|name| !name.is_empty());
        match name {
            Some(name) if d.alt.is_none() => {
                let index = self.state.target(Dest::Here, &[name], true, prefix, d.line);
                vec![Node::Anchor(self.state.targets[index].id.clone())]
            }
            _ => Vec::new(),
        }
    }

    /// The blocks of `content`, read as docutils reads a directive's content
    /// into an element of no kind, where a topic may be, and a sidebar, but
    /// for a sidebar's own content.
    fn loose(&mut self, content: &[Line], sidebar: bool) -> Vec<Node> {
        let saved = (self.state.loose, self.state.sidebar);
        let depth = Some(self.state.depth + 1);
        self.state.loose = depth;
        if sidebar {
            self.state.sidebar = depth;
        }
        let nodes = nested(self.state, content);
        (self.state.loose, self.state.sidebar) = saved;
        nodes
    }

    /// An error, unless a topic or a sidebar may be here: where a section
    /// may be, or in a directive's content that docutils reads into an
    /// element of no kind, or a sidebar's.
    fn context(&self, d: &Directive) -> Result<(), String> {
        let loose = self.state.loose == Some(self.state.depth);
        if d.alt.is_none() && (self.titles || loose) {
            return Ok(());
        }
        Err("it may not be used within topics or body elements".to_string())
    }

    /// An admonition: an alert of the nearest kind, with its title, if
    /// GitHub's kind has another name, or it's an `admonition`, whose
    /// argument is its title.
    fn admonition(&mut self, known: &str, d: &Directive) -> Result<Vec<Node>, String> {
        d.has_content()?;
        let kind = match known {
            "tip" | "hint" => AlertKind::Tip,
            "important" => AlertKind::Important,
            "warning" | "attention" | "caution" => AlertKind::Warning,
            "danger" | "error" => AlertKind::Caution,
            _ => AlertKind::Note,
        };
        let mut nodes = self.anchor(d, known);
        let title = match known {
            "admonition" => {
                let title = self.inline(d.argument().unwrap_or_default(), d.line);
                collapse(&Span::text(&title))
            }
            "hint" | "attention" | "caution" | "danger" | "error" => {
                let mut title = known.to_string();
                title[..1].make_ascii_uppercase();
                title
            }
            _ => String::new(),
        };
        let blocks = nested(self.state, &d.content);
        nodes.push(Node::Alert {
            kind,
            title,
            blocks,
        });
        Ok(nodes)
    }

    /// A topic: its title, in bold, then its blocks.
    fn topic(&mut self, d: &Directive) -> Result<Vec<Node>, String> {
        self.context(d)?;
        d.has_content()?;
        let title = self.inline(d.argument().unwrap_or_default(), d.line);
        let mut nodes = self.anchor(d, "topic");
        nodes.push(Node::Para(vec![Span::Strong(title)]));
        nodes.extend(nested(self.state, &d.content));
        Ok(vec![Node::Group(nodes)])
    }

    /// A sidebar: its title, in bold, and subtitle, in italics, if it has
    /// them, then its blocks.
    fn sidebar(&mut self, d: &Directive) -> Result<Vec<Node>, String> {
        if d.alt.is_none() && self.state.sidebar == Some(self.state.depth) {
            return Err("it may not be used within a sidebar element".to_string());
        }
        let subtitle = d.option("subtitle");
        if subtitle.is_some() && d.arguments.is_empty() {
            return Err("the \"subtitle\" option may not be used without a title".to_string());
        }
        self.context(d)?;
        d.has_content()?;
        let mut titles = Vec::new();
        if let Some(title) = d.argument() {
            titles.push(Node::Para(vec![Span::Strong(self.inline(title, d.line))]));
            if let Some(subtitle) = subtitle {
                let subtitle = self.inline(subtitle, d.line);
                titles.push(Node::Para(vec![Span::Emphasis(subtitle)]));
            }
        }
        let mut nodes = self.anchor(d, "sidebar");
        nodes.extend(titles);
        nodes.extend(self.loose(&d.content, true));
        Ok(vec![Node::Group(nodes)])
    }

    /// The `line-block` directive: each line kept apart, with its indent,
    /// as a line block is.
    fn line_block_directive(&mut self, d: &Directive) -> Result<Vec<Node>, String> {
        d.has_content()?;
        let mut nodes = self.anchor(d, "line-block");
        let entries: Vec<(Option<usize>, String, usize)> = d
            .content
            .iter()
            .map(|line| {
                let text = line.text.trim_matches(is_space);
                let indent = (!text.is_empty())
                    .then(|| line.text.chars().take_while(|&c| is_space(c)).count());
                (indent, text.to_string(), line.num)
            })
            .collect();
        nodes.push(Node::Para(self.line_spans(&entries)));
        Ok(nodes)
    }

    /// The `math` directive: a formula for each run of lines between blank
    /// lines.
    fn math(&mut self, d: &Directive) -> Result<Vec<Node>, String> {
        d.has_content()?;
        let text = join(&d.content);
        let mut nodes = Vec::new();
        for block in text.split("\n\n") {
            let block = block.trim_matches(is_space);
            if block.is_empty() {
                continue;
            }
            if nodes.is_empty() {
                nodes = self.anchor(d, "math-block");
            }
            nodes.push(Node::Math(block.to_string()));
        }
        Ok(nodes)
    }

    /// The image of `d`, an `image` or a `figure`: in a link, if it has a
    /// `:target:`, a URL, or a reference to a target.
    fn image_span(&self, d: &Directive) -> Span {
        let url = uri(&escape(d.argument().unwrap_or_default()));
        let alt = d.option("alt").or(d.alt).unwrap_or(&url).to_string();
        let image = Span::Image {
            url: url.clone(),
            alt,
        };
        let Some(target) = d.option("target") else {
            return image;
        };
        let block: Vec<String> = escape(target).lines().map(String::from).collect();
        let to = match parse_target(&block) {
            Dest::Url(url) => Ref::Url(url),
            Dest::Name(name) => Ref::Name(name),
            _ => return image,
        };
        Span::Link {
            content: vec![image],
            to,
            raw: String::new(),
            line: d.line,
        }
    }

    /// The `image` directive: the image, in a paragraph of its own, or the
    /// text of a substitution.
    fn image(&mut self, d: &Directive) -> Result<Option<Substitution>, String> {
        let mut nodes = self.anchor(d, "image");
        let span = self.image_span(d);
        if d.alt.is_some() {
            return Ok(Some(Substitution {
                spans: vec![span],
                ltrim: false,
                rtrim: false,
            }));
        }
        nodes.push(Node::Para(vec![span]));
        self.nodes.push(Node::Group(nodes));
        Ok(None)
    }

    /// A figure: its image, then its caption, the paragraph its content
    /// starts with, in italics, unless it starts with an empty comment,
    /// then the rest, its legend.
    fn figure(&mut self, d: &Directive) -> Result<Vec<Node>, String> {
        // docutils names the image before the figure, so its id comes first.
        let image = self.anchor(d, "image");
        let mut nodes = self.named(d, "figname", "figure");
        nodes.extend(image);
        nodes.push(Node::Para(vec![self.image_span(d)]));
        let Some(first) = d.content.first() else {
            return Ok(vec![Node::Group(nodes)]);
        };
        let next = d.content.get(1);
        let empty_comment =
            first.text.trim_end() == ".." && next.is_none_or(|line| !line.text.starts_with(' '));
        let mut children = self.loose(&d.content, false).into_iter();
        for child in children.by_ref() {
            match child {
                Node::Para(spans) => {
                    nodes.push(Node::Para(vec![Span::Emphasis(spans)]));
                    break;
                }
                Node::Nothing { prebib: true } if empty_comment => break,
                // docutils keeps targets, and nodes its transforms remove,
                // before the caption.
                Node::Target(_) | Node::Nothing { prebib: true } => nodes.push(child),
                _ => {
                    let message = "left out the caption and legend of `.. figure::`, as its \
                                   caption must be a paragraph or an empty comment";
                    self.state.warn(d.line, message);
                    return Ok(vec![Node::Group(nodes), Node::Problem(Vec::new())]);
                }
            }
        }
        nodes.extend(children);
        Ok(vec![Node::Group(nodes)])
    }

    /// The `replace` directive: the text of the paragraph it holds.
    fn replace(&mut self, d: &Directive) -> Result<Option<Substitution>, String> {
        d.has_content()?;
        let mut spans = None;
        for node in self.loose(&d.content, false) {
            match node {
                Node::Para(para) if spans.is_none() => spans = Some(para),
                Node::Problem(_) => self.nodes.push(node),
                _ => return Err("it may contain a single paragraph only".to_string()),
            }
        }
        Ok(spans.map(|spans| Substitution {
            spans,
            ltrim: false,
            rtrim: false,
        }))
    }

    /// The `date` directive: the time the page is built, in UTC, written
    /// in the format its content gives, as `strftime` writes it.
    fn date(&mut self, d: &Directive) -> Substitution {
        let format = join(&d.content);
        let format = if format.is_empty() {
            "%Y-%m-%d"
        } else {
            &format
        };
        let (text, unknown) = strftime(format, self.state.clock);
        for code in unknown {
            let message = format!("kept `{code}` in `.. date::`, as the plugin doesn't know it");
            self.state.warn(d.line, message);
        }
        Substitution {
            spans: vec![Span::Text(text)],
            ltrim: false,
            rtrim: false,
        }
    }

    /// The `raw` directive: its content, as HTML, if HTML is one of its
    /// formats, else nothing.
    fn raw(&mut self, d: &Directive) -> Result<Option<Substitution>, String> {
        let external = d.option("file").is_some() || d.option("url").is_some();
        if external && !d.content.is_empty() {
            let message = "it may not both specify an external file and have content";
            return Err(message.to_string());
        }
        let format = d.argument().unwrap_or_default().to_lowercase();
        let html = format.split(is_space).any(|word| word == "html");
        if !external {
            d.has_content()?;
        } else if html {
            let message = "left out `.. raw::`, as the plugin doesn't follow includes";
            self.state.warn(d.line, message);
        }
        let text = (html && !external).then(|| join(&d.content));
        if d.alt.is_some() {
            let spans = text
                .map(|text| Span::Html(text, d.line))
                .into_iter()
                .collect();
            return Ok(Some(Substitution {
                spans,
                ltrim: false,
                rtrim: false,
            }));
        }
        self.nodes.push(match text {
            Some(text) => Node::Html(text, d.line),
            None => Node::Nothing { prebib: true },
        });
        Ok(None)
    }

    /// The `meta` directive: the page's description, from its first
    /// `description` field, unless it has one already. Its other fields are
    /// for HTML's `<head>`, which the theme writes.
    fn meta(&mut self, d: &Directive) {
        let content = &d.content;
        let mut at = 0;
        while let Some(line) = content.get(at) {
            if line.text.is_empty() {
                at += 1;
                continue;
            }
            let Some((raw, start)) = field_marker(&line.text) else {
                let message = "left out the rest of `.. meta::`, as it holds something other \
                               than fields";
                self.state.warn(line.num, message);
                return;
            };
            let body = indented(content, at, false, None, Some(start));
            at = body.end;
            let name = unescape(&escape(&raw));
            let first = name
                .split(is_space)
                .find(|word| !word.is_empty())
                .unwrap_or_default();
            let description = match first.split_once('=') {
                Some((key, value)) => {
                    let value = value.trim_matches(|c| c == '"' || c == '\'');
                    key.eq_ignore_ascii_case("name") && value.eq_ignore_ascii_case("description")
                }
                None => first.eq_ignore_ascii_case("description"),
            };
            let texts: Vec<&str> = body.lines.iter().map(|line| line.text.as_str()).collect();
            let value = collapse(&unescape(&escape(&texts.join(" "))));
            let metadata = &mut self.state.metadata;
            if description
                && !value.is_empty()
                && !metadata.iter().any(|(key, _)| key == "description")
            {
                metadata.push(("description".to_string(), value));
            }
        }
    }

    /// The `role` directive: a custom role, named on its first line, with
    /// the role it's based on, if any, in brackets after it, and its
    /// options.
    fn role(&mut self, d: &Directive) -> Result<(), String> {
        let first = d.content.first().filter(|line| line.num <= d.line + 1);
        let Some(first) = first else {
            return Err("it requires arguments on the first line".to_string());
        };
        let text = first.text.as_str();
        let invalid = || format!("arguments not valid role names: \"{text}\"");
        let end = simplename(text);
        let (name, rest) = text.split_at(end);
        let rest = rest.trim_start_matches(is_space);
        let mut base = None;
        if !rest.is_empty() {
            let inner = rest.strip_prefix('(').ok_or_else(invalid)?;
            let inner = inner.trim_start_matches(is_space);
            let (base_name, after) = inner.split_at(simplename(inner));
            let after = after.trim_start_matches(is_space).strip_prefix(')');
            if base_name.is_empty()
                || !after.is_some_and(|after| after.trim_matches(is_space).is_empty())
            {
                return Err(invalid());
            }
            base = Some(base_name);
        }
        if name.is_empty() {
            return Err(invalid());
        }
        let role = match base {
            None => Role::Plain,
            Some(base) => {
                let lower = base.to_lowercase();
                let role = self
                    .state
                    .roles
                    .get(&lower)
                    .cloned()
                    .or_else(|| Role::standard(&lower));
                role.ok_or_else(|| format!("unknown interpreted text role \"{base}\""))?
            }
        };
        let options: &'static [&'static str] = match role {
            Role::Raw(_) => &["class", "format"],
            Role::Code => &["class", "language"],
            _ => &["class"],
        };
        let (_, options, _) =
            parse_block(d.content[1..].to_vec(), &spec(0, 0, false, options, false))?;
        let option = |key: &str| {
            options
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value)
        };
        if option("class").is_none() && make_id(name).is_empty() {
            return Err(format!("cannot make \"{name}\" into a class name"));
        }
        let role = match (role, option("format")) {
            (Role::Raw(_), Some(format)) => {
                Role::Raw(Some(format.split(is_space).any(|word| word == "html")))
            }
            (role, _) => role,
        };
        self.state.roles.insert(name.to_lowercase(), role);
        Ok(())
    }

    /// The `default-role` directive: the role of interpreted text without
    /// one, from here on, or `title-reference`, if it has no argument.
    fn default_role(&mut self, d: &Directive) -> Result<(), String> {
        let Some(name) = d.argument() else {
            self.state.default_role = ("title-reference".to_string(), Role::Title);
            return Ok(());
        };
        let lower = name.to_lowercase();
        let role = self
            .state
            .roles
            .get(&lower)
            .cloned()
            .or_else(|| Role::standard(&lower));
        let role = role.ok_or_else(|| format!("unknown interpreted text role \"{name}\""))?;
        self.state.default_role = (name.to_string(), role);
        Ok(())
    }

    /// The `table` directive: the table it holds, after its title.
    fn table_directive(&mut self, d: &Directive) -> Result<Vec<Node>, String> {
        d.has_content()?;
        let title = d.argument().map(|title| self.inline(title, d.line));
        let mut nodes = self.loose(&d.content, false);
        let table = match nodes.pop() {
            Some(table @ Node::Table(_)) if nodes.is_empty() => table,
            _ => return Err("exactly one table expected".to_string()),
        };
        Ok(self.titled(d, title, table))
    }

    /// The `list-table` directive: a table of the bullet list it holds,
    /// whose items are its rows, each a bullet list of its cells.
    fn list_table(&mut self, d: &Directive) -> Result<Vec<Node>, String> {
        d.has_content()?;
        let title = d.argument().map(|title| self.inline(title, d.line));
        let mut nodes = self.loose(&d.content, false);
        let items = match nodes.pop() {
            Some(Node::List { start: None, items }) if nodes.is_empty() => items,
            _ => return Err("exactly one bullet list expected".to_string()),
        };
        let mut rows: Vec<Vec<Vec<Node>>> = Vec::new();
        for (index, mut item) in items.into_iter().enumerate() {
            let row = index + 1;
            let cells = match item.pop() {
                Some(Node::List { start: None, items }) if item.is_empty() => items,
                _ => {
                    return Err(format!(
                        "two-level bullet list expected, but row {row} does not contain a \
                         second-level bullet list"
                    ));
                }
            };
            if let Some(columns) = rows.first().map(Vec::len)
                && cells.len() != columns
            {
                let len = cells.len();
                return Err(format!(
                    "uniform two-level bullet list expected, but row {row} does not contain \
                     the same number of items as row 1 ({len} vs {columns})"
                ));
            }
            rows.push(cells);
        }
        let header_rows = dimensions(d, &rows)?;
        let table = tables::table(self.state, d.line, rows, header_rows);
        Ok(self.titled(d, title, table))
    }

    /// The `csv-table` directive: a table of the comma-separated values it
    /// holds, as Python's `csv` module reads them, after those of its
    /// `:header:`.
    fn csv_table(&mut self, d: &Directive) -> Result<Vec<Node>, String> {
        if d.option("file").is_some() || d.option("url").is_some() {
            if !d.content.is_empty() {
                let message = "it may not both specify an external file and have content";
                return Err(message.to_string());
            }
            let message = "left out `.. csv-table::`, as the plugin doesn't follow includes";
            self.state.warn(d.line, message);
            return Ok(vec![Node::Nothing { prebib: true }]);
        }
        d.has_content()?;
        let title = d.argument().map(|title| self.inline(title, d.line));
        let dialect = Dialect::new(d)?;
        let csv = |lines: &[Line]| {
            records(lines, &dialect).map_err(|detail| format!("error with its CSV data: {detail}"))
        };
        let head = match d.option("header") {
            Some(header) => {
                let line = |text: &str| Line {
                    text: text.to_string(),
                    num: d.line,
                };
                csv(&header.split('\n').map(line).collect::<Vec<_>>())?
            }
            None => Vec::new(),
        };
        let records = csv(&d.content)?;
        let header_rows = dimensions(d, &records)?;
        let all = head.iter().chain(&records);
        let columns = all.map(Vec::len).max().unwrap_or(0);
        if columns == 0 {
            return Err("no table data detected".to_string());
        }
        let head_rows = head.len() + header_rows;
        let mut rows = Vec::new();
        for record in head.into_iter().chain(records) {
            let mut cells = Vec::new();
            for (text, line) in record {
                let lines: Vec<Line> = text
                    .split('\n')
                    .enumerate()
                    .map(|(index, text)| Line {
                        text: text.trim_end_matches(is_space).to_string(),
                        num: line + index,
                    })
                    .collect();
                let empty = lines.iter().all(|line| line.text.is_empty());
                cells.push(if empty {
                    Vec::new()
                } else {
                    nested(self.state, &lines)
                });
            }
            cells.resize_with(columns, Vec::new);
            rows.push(cells);
        }
        let table = tables::table(self.state, d.line, rows, head_rows);
        Ok(self.titled(d, title, table))
    }

    /// The blocks of a table directive: its anchor, its title, in italics,
    /// if it has one, and its table.
    fn titled(&mut self, d: &Directive, title: Option<Vec<Span>>, table: Node) -> Vec<Node> {
        let mut nodes = self.anchor(d, "table");
        if let Some(title) = title {
            nodes.push(Node::Para(vec![Span::Emphasis(title)]));
        }
        nodes.push(table);
        nodes
    }
}

/// The number of header rows of a table directive's `rows`, from its
/// `header-rows`, or an error, as docutils' `check_table_dimensions` gives,
/// if they, or its `stub-columns`, leave no row or column for its body.
fn dimensions<T>(d: &Directive, rows: &[Vec<T>]) -> Result<usize, String> {
    let number = |name: &str| d.option(name).and_then(|value| value.parse().ok());
    let header_rows: usize = number("header-rows").unwrap_or(0);
    let stub_columns: usize = number("stub-columns").unwrap_or(0);
    let len = rows.len();
    if len < header_rows {
        return Err(format!(
            "{header_rows} header row(s) specified but only {len} row(s) of data supplied"
        ));
    }
    if len == header_rows && len > 0 {
        return Err(format!(
            "insufficient data supplied ({len} row(s)); no data remaining for table body"
        ));
    }
    for row in rows {
        let len = row.len();
        if len < stub_columns {
            return Err(format!(
                "{stub_columns} stub column(s) specified but only {len} column(s) of data \
                 supplied"
            ));
        }
        if len == stub_columns && len > 0 {
            return Err(format!(
                "insufficient data supplied ({len} column(s)); no data remaining for table body"
            ));
        }
    }
    Ok(header_rows)
}

/// How a `csv-table` reads its values: docutils' `DocutilsDialect`, with
/// the characters its options set.
struct Dialect {
    delimiter: char,
    quote: char,
    escape: Option<char>,
    /// Whether a quote in a quoted value is written twice, as it is unless
    /// there's an escape.
    double: bool,
    /// Whether spaces at the start of a value are left out, as they are
    /// unless `keepspace` is set.
    skip_space: bool,
}

impl Dialect {
    /// The dialect `d`'s options set, or an error for a character Python's
    /// `csv` module refuses, where docutils crashes.
    fn new(d: &Directive) -> Result<Dialect, String> {
        let option = |name| d.option(name).and_then(|value| value.chars().next());
        let dialect = Dialect {
            delimiter: option("delim").unwrap_or(','),
            quote: option("quote").unwrap_or('"'),
            escape: option("escape"),
            double: option("escape").is_none(),
            skip_space: d.option("keepspace").is_none(),
        };
        let space = !dialect.skip_space;
        let characters = [
            ("delim", Some(dialect.delimiter), true),
            ("escape", dialect.escape, space),
            ("quote", Some(dialect.quote), space),
        ];
        for (name, c, space) in characters {
            if matches!(c, Some('\r' | '\n')) || (c == Some(' ') && !space) {
                return Err(format!("bad \"{name}\" value"));
            }
        }
        let pairs = [
            ("delim", "escape", Some(dialect.delimiter), dialect.escape),
            (
                "delim",
                "quote",
                Some(dialect.delimiter),
                Some(dialect.quote),
            ),
            ("escape", "quote", dialect.escape, Some(dialect.quote)),
        ];
        for (a, b, x, y) in pairs {
            if x.is_some() && x == y {
                return Err(format!("bad \"{a}\" or \"{b}\" value"));
            }
        }
        Ok(dialect)
    }
}

/// Where Python's `csv` reader is in a record: the states of CPython's
/// `_csv.c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Csv {
    StartRecord,
    StartField,
    Escaped,
    AfterEscapedBreak,
    InField,
    InQuoted,
    EscapeInQuoted,
    QuoteInQuoted,
    EatBreak,
}

/// Reads the records of comma-separated values, a character at a time.
struct Reader<'d> {
    dialect: &'d Dialect,
    state: Csv,
    field: String,
    /// The number of the line the field starts on.
    start: usize,
    fields: Vec<(String, usize)>,
}

impl Reader<'_> {
    fn save(&mut self) {
        self.fields
            .push((std::mem::take(&mut self.field), self.start));
    }

    /// Ends the field at a line break, or at the end of a line, `None`.
    fn end_line(&mut self, c: Option<char>) {
        self.save();
        self.state = if c.is_some() {
            Csv::EatBreak
        } else {
            Csv::StartRecord
        };
    }

    /// Reads `c`, on the line `line`, or the end of the line, `None`, as
    /// CPython's `parse_process_char` does, strictly.
    fn read(&mut self, c: Option<char>, line: usize) -> Result<(), String> {
        let d = self.dialect;
        let escape = |c: char| Some(c) == d.escape;
        match (self.state, c) {
            (Csv::StartRecord, None) => {}
            (Csv::StartRecord, Some('\n' | '\r')) => self.state = Csv::EatBreak,
            (Csv::StartRecord, Some(_)) => {
                self.state = Csv::StartField;
                return self.read(c, line);
            }
            (Csv::StartField, None | Some('\n' | '\r')) => {
                self.start = line;
                self.end_line(c);
            }
            (Csv::StartField, Some(c)) => {
                self.start = line;
                if c == d.quote {
                    self.state = Csv::InQuoted;
                } else if escape(c) {
                    self.state = Csv::Escaped;
                } else if c == ' ' && d.skip_space {
                } else if c == d.delimiter {
                    self.save();
                } else {
                    self.field.push(c);
                    self.state = Csv::InField;
                }
            }
            (Csv::Escaped, Some(c @ ('\n' | '\r'))) => {
                self.field.push(c);
                self.state = Csv::AfterEscapedBreak;
            }
            (Csv::Escaped, _) => {
                self.field.push(c.unwrap_or('\n'));
                self.state = Csv::InField;
            }
            (Csv::AfterEscapedBreak, None) => {}
            (Csv::AfterEscapedBreak | Csv::InField, None | Some('\n' | '\r')) => self.end_line(c),
            // After an escaped line break, as in a field, but for staying
            // in this state.
            (Csv::AfterEscapedBreak | Csv::InField, Some(c)) => {
                if escape(c) {
                    self.state = Csv::Escaped;
                } else if c == d.delimiter {
                    self.save();
                    self.state = Csv::StartField;
                } else {
                    self.field.push(c);
                }
            }
            (Csv::InQuoted, None) => {}
            (Csv::InQuoted, Some(c)) => {
                if escape(c) {
                    self.state = Csv::EscapeInQuoted;
                } else if c == d.quote {
                    self.state = if d.double {
                        Csv::QuoteInQuoted
                    } else {
                        Csv::InField
                    };
                } else {
                    self.field.push(c);
                }
            }
            (Csv::EscapeInQuoted, _) => {
                self.field.push(c.unwrap_or('\n'));
                self.state = Csv::InQuoted;
            }
            (Csv::QuoteInQuoted, Some(c)) if c == d.quote => {
                self.field.push(c);
                self.state = Csv::InQuoted;
            }
            (Csv::QuoteInQuoted, Some(c)) if c == d.delimiter => {
                self.save();
                self.state = Csv::StartField;
            }
            (Csv::QuoteInQuoted, None | Some('\n' | '\r')) => self.end_line(c),
            (Csv::QuoteInQuoted, Some(_)) => {
                return Err(format!("'{}' expected after '{}'", d.delimiter, d.quote));
            }
            (Csv::EatBreak, Some('\n' | '\r')) => {}
            (Csv::EatBreak, None) => self.state = Csv::StartRecord,
            (Csv::EatBreak, Some(_)) => {
                return Err("new-line character seen in unquoted field".to_string());
            }
        }
        Ok(())
    }
}

/// The records of `lines`, each a list of its values, with the number of
/// the line each starts on, as Python's `csv` module reads them in
/// `dialect`, strictly, as docutils has it: a line at a time, each with a
/// newline after it.
fn records(lines: &[Line], dialect: &Dialect) -> Result<Vec<Vec<(String, usize)>>, String> {
    let mut reader = Reader {
        dialect,
        state: Csv::StartRecord,
        field: String::new(),
        start: 0,
        fields: Vec::new(),
    };
    let mut records = Vec::new();
    for line in lines {
        for c in line.text.chars().chain(['\n']) {
            reader.read(Some(c), line.num)?;
        }
        reader.read(None, line.num)?;
        if reader.state == Csv::StartRecord {
            records.push(std::mem::take(&mut reader.fields));
        }
    }
    if !reader.field.is_empty() || reader.state == Csv::InQuoted {
        return Err("unexpected end of data".to_string());
    }
    Ok(records)
}

/// The `unicode` directive: the characters its argument's codes stand for,
/// up to a comment, and the whitespace it takes out around it.
fn unicode(d: &Directive) -> Result<Option<Substitution>, String> {
    let argument = before_comment(d.argument().unwrap_or_default());
    let mut text = String::new();
    for code in argument.split(is_space).filter(|code| !code.is_empty()) {
        let decoded =
            unicode_code(code).ok_or_else(|| format!("invalid character code: {code}"))?;
        text.push_str(&decoded);
    }
    if text.is_empty() {
        return Ok(None);
    }
    let trim = d.option("trim").is_some();
    Ok(Some(Substitution {
        spans: vec![Span::Text(text)],
        ltrim: trim || d.option("ltrim").is_some(),
        rtrim: trim || d.option("rtrim").is_some(),
    }))
}

/// `text` up to a comment, `.. ` at its start or after a space or a line
/// break, which the `unicode` directive leaves out.
fn before_comment(text: &str) -> &str {
    let mut from = 0;
    while let Some(at) = text[from..].find(".. ") {
        let at = from + at;
        if at == 0 || text[..at].ends_with([' ', '\n']) {
            return &text[..at];
        }
        from = at + 1;
    }
    text
}

/// `format`, with each of C's `strftime` codes in it, as glibc's, with the
/// `-` flag, which leaves a number unpadded, replaced with what it writes
/// of `clock`'s time, in UTC, in the C locale; and the codes it doesn't
/// know, kept as they are.
fn strftime(format: &str, clock: Clock) -> (String, Vec<String>) {
    let mut text = String::new();
    let mut unknown = Vec::new();
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            text.push(c);
            continue;
        }
        let flag = chars.next_if_eq(&'-').is_some();
        let code = chars.next();
        match code.and_then(|code| strftime_code(code, flag, clock)) {
            Some(written) => text.push_str(&written),
            None => {
                let mut kept = if flag { "%-" } else { "%" }.to_string();
                kept.extend(code);
                text.push_str(&kept);
                if code.is_some() {
                    unknown.push(kept);
                }
            }
        }
    }
    (text, unknown)
}

/// What the `strftime` code `code` writes of `clock`'s time, unpadded if
/// `flag`, or None if it isn't one.
fn strftime_code(code: char, flag: bool, clock: Clock) -> Option<String> {
    const DAYS: [&str; 7] = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let (year, month, day) = clock.date();
    let (hour, minute, second) = clock.time();
    let (month, day, hour) = (i64::from(month), i64::from(day), hour as i64);
    let (minute, second) = (minute as i64, second as i64);
    // From 0, Monday, as `Clock` counts them, and from 0, Sunday, as C does.
    let monday = clock.weekday() as i64;
    let sunday = (monday + 1) % 7;
    let year_day = day_of_year(year, month, day);
    let number = |value: i64, width: usize, zero: bool| match (flag, zero) {
        (true, _) => value.to_string(),
        (false, true) => format!("{value:0width$}"),
        (false, false) => format!("{value:width$}"),
    };
    let formatted = |format: &str| strftime(format, clock).0;
    let month_name = MONTHS[(month - 1) as usize];
    let day_name = DAYS[monday as usize];
    let written = match code {
        'Y' => year.to_string(),
        'y' => number(year.rem_euclid(100), 2, true),
        'C' => number(year.div_euclid(100), 2, true),
        'm' => number(month, 2, true),
        'd' => number(day, 2, true),
        'e' => number(day, 2, false),
        'H' => number(hour, 2, true),
        'I' => number((hour + 11) % 12 + 1, 2, true),
        'M' => number(minute, 2, true),
        'S' => number(second, 2, true),
        'j' => number(year_day + 1, 3, true),
        'u' => (monday + 1).to_string(),
        'w' => sunday.to_string(),
        'U' => number((year_day + 7 - sunday) / 7, 2, true),
        'W' => number((year_day + 7 - monday) / 7, 2, true),
        'p' => if hour < 12 { "AM" } else { "PM" }.to_string(),
        'a' => day_name[..3].to_string(),
        'A' => day_name.to_string(),
        'b' | 'h' => month_name[..3].to_string(),
        'B' => month_name.to_string(),
        'F' => formatted("%Y-%m-%d"),
        'T' | 'X' => formatted("%H:%M:%S"),
        'D' | 'x' => formatted("%m/%d/%y"),
        'R' => formatted("%H:%M"),
        'r' => formatted("%I:%M:%S %p"),
        'c' => formatted("%a %b %e %H:%M:%S %Y"),
        'Z' => "UTC".to_string(),
        'z' => "+0000".to_string(),
        's' => clock.seconds.to_string(),
        'n' => "\n".to_string(),
        't' => "\t".to_string(),
        '%' => "%".to_string(),
        _ => return None,
    };
    Some(written)
}

/// The day of the year of a date, from 0.
fn day_of_year(year: i64, month: i64, day: i64) -> i64 {
    const BEFORE: [i64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    BEFORE[(month - 1) as usize] + day - 1 + i64::from(leap && month > 2)
}

/// An error unless each word of a `class` or `container` directive's
/// argument makes a class name, as docutils' `class_option` makes them.
fn class_check(argument: &str) -> Result<(), String> {
    let mut names = argument.split(is_space).filter(|name| !name.is_empty());
    if names.all(|name| !make_id(name).is_empty()) {
        Ok(())
    } else {
        Err(format!("invalid class attribute value: \"{argument}\""))
    }
}
