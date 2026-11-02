//! The keywords an Org file sets for itself, like `#+TITLE:` and
//! `#+MACRO:`, which count wherever they are in it, so they're read before
//! the rest of it.

use std::collections::HashMap;

use orgize::config::UseSubSuperscript;
use orgize::{ParseConfig, SyntaxKind, SyntaxNode};

/// A keyword's key, like `TITLE`, as it's written, and its value, trimmed,
/// from the `#+KEY: value` of a `KEYWORD`, `AFFILIATED_KEYWORD` or
/// `BABEL_CALL`. A short caption, `#+CAPTION[short]: long`, is left out.
pub fn keyword_parts(node: &SyntaxNode) -> (String, String) {
    let mut key = None;
    let mut value = String::new();
    let mut after_colon = false;
    for element in node.children_with_tokens() {
        match element.kind() {
            SyntaxKind::NEW_LINE | SyntaxKind::BLANK_LINE => {}
            _ if after_colon => value.push_str(&element.to_string()),
            SyntaxKind::COLON => after_colon = true,
            SyntaxKind::TEXT if key.is_none() => key = Some(element.to_string()),
            _ => {}
        }
    }
    (key.unwrap_or_default(), value.trim().to_string())
}

/// How a file asks to be read: its `#+OPTIONS:` and TODO keywords.
pub struct Options {
    /// How orgize reads it: which `_` and `^` are sub- and superscripts, as
    /// `#+OPTIONS: ^:` says, and its TODO keywords, as `#+TODO:` says, so a
    /// headline's is left out.
    pub config: ParseConfig,
    /// Whether the file sets any of those, so it's read again with them.
    pub changed: bool,
    /// Whether `--`, `---`, `...` and `\-` are dashes, an ellipsis and a
    /// soft hyphen, as `#+OPTIONS: -:` says, which they are unless it's
    /// `nil`.
    pub special_strings: bool,
}

/// The config orgize reads a file with before its options are known. It
/// reads no affiliated keywords, like `#+CAPTION:`, but `#+ATTR_…:`, which
/// it can't help, so they're each a keyword, matched to what follows them
/// as Org does.
pub fn base_config() -> ParseConfig {
    ParseConfig {
        affiliated_keywords: Vec::new(),
        ..ParseConfig::default()
    }
}

/// The options of the file whose tree is `root`, read with
/// [`base_config`].
pub fn options(root: &SyntaxNode) -> Options {
    let mut config = base_config();
    let mut changed = false;
    let mut special_strings = true;
    let (mut todo, mut done) = (Vec::new(), Vec::new());
    for node in root.descendants() {
        if node.kind() != SyntaxKind::KEYWORD {
            continue;
        }
        let (key, value) = keyword_parts(&node);
        match key.to_ascii_uppercase().as_str() {
            "OPTIONS" => {
                for option in value.split_whitespace() {
                    if let Some(setting) = option.strip_prefix("^:") {
                        changed = true;
                        config.use_sub_superscript = match setting {
                            "{}" => UseSubSuperscript::Brace,
                            "nil" => UseSubSuperscript::Nil,
                            _ => UseSubSuperscript::True,
                        };
                    } else if let Some(setting) = option.strip_prefix("-:") {
                        special_strings = setting != "nil";
                    }
                }
            }
            "TODO" | "SEQ_TODO" | "TYP_TODO" => {
                // Each word may have a key and how it's logged, `WAIT(w@/!)`.
                let words: Vec<&str> = value
                    .split_whitespace()
                    .map(|word| word.split('(').next().unwrap_or_default())
                    .filter(|word| !word.is_empty())
                    .collect();
                match words.iter().position(|word| *word == "|") {
                    Some(bar) => {
                        todo.extend(words[..bar].iter().map(|word| word.to_string()));
                        done.extend(words[bar + 1..].iter().map(|word| word.to_string()));
                    }
                    None => {
                        if let Some((last, rest)) = words.split_last() {
                            todo.extend(rest.iter().map(|word| word.to_string()));
                            done.push(last.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if !todo.is_empty() || !done.is_empty() {
        changed = true;
        config.todo_keywords = (todo, done);
    }
    Options {
        config,
        changed,
        special_strings,
    }
}

/// What a file's keywords say of it, and of the macros and blocks of code
/// in it.
#[derive(Default)]
pub struct Keywords {
    /// Each keyword's values, by its key, in capitals, in order.
    values: HashMap<String, Vec<String>>,
    /// Where the first `#+TITLE:` is, in the file.
    pub title_at: usize,
    /// The macros `#+MACRO:` defines, by their names, in lower case, as Org
    /// matches them, each with its template.
    pub macros: HashMap<String, String>,
    /// The header arguments `#+PROPERTY:` gives every block of code.
    pub header_args: HeaderArgs,
}

impl Keywords {
    /// The keywords in the tree `root`.
    pub fn new(root: &SyntaxNode) -> Keywords {
        let mut keywords = Keywords::default();
        for node in root.descendants() {
            if node.kind() != SyntaxKind::KEYWORD {
                continue;
            }
            let (key, value) = keyword_parts(&node);
            let key = key.to_ascii_uppercase();
            match key.as_str() {
                "MACRO" => {
                    let (name, template) = value
                        .split_once(char::is_whitespace)
                        .unwrap_or((&value, ""));
                    keywords
                        .macros
                        .insert(name.to_lowercase(), template.trim().to_string());
                }
                "PROPERTY" => {
                    let (name, args) = value
                        .split_once(char::is_whitespace)
                        .unwrap_or((&value, ""));
                    keywords.header_args.set(name, args.trim());
                }
                "TITLE" if !keywords.values.contains_key("TITLE") => {
                    keywords.title_at = usize::from(node.text_range().start());
                }
                _ => {}
            }
            keywords.values.entry(key).or_default().push(value);
        }
        keywords
    }

    /// The values of the keyword `key`, in capitals, joined with spaces, as
    /// Org joins those of `#+TITLE:` and `#+AUTHOR:`, or `None`, if the file
    /// doesn't set it.
    pub fn joined(&self, key: &str) -> Option<String> {
        let values = self.values.get(key)?;
        let values: Vec<&str> = values
            .iter()
            .map(String::as_str)
            .filter(|value| !value.is_empty())
            .collect();
        Some(values.join(" "))
    }

    /// The last value of the keyword `key`, in capitals, as Org keeps that
    /// of `#+EMAIL:`.
    pub fn last(&self, key: &str) -> Option<&str> {
        self.values.get(key)?.last().map(String::as_str)
    }
}

/// The header arguments blocks of code inherit, from `#+PROPERTY:` and the
/// headlines they're under: those of every language, and of each.
#[derive(Clone, Default)]
pub struct HeaderArgs {
    every: String,
    languages: HashMap<String, String>,
}

impl HeaderArgs {
    /// Sets the property `name` to `value`, if it's one of header arguments,
    /// like `header-args`, or `header-args:python+`, whose `+` adds to them.
    pub fn set(&mut self, name: &str, value: &str) {
        let Some(rest) = name
            .get(.."header-args".len())
            .filter(|prefix| prefix.eq_ignore_ascii_case("header-args"))
            .map(|prefix| &name[prefix.len()..])
        else {
            return;
        };
        let (rest, add) = match rest.strip_suffix('+') {
            Some(rest) => (rest, true),
            None => (rest, false),
        };
        let args = if rest.is_empty() {
            &mut self.every
        } else if let Some(language) = rest.strip_prefix(':') {
            self.languages.entry(language.to_string()).or_default()
        } else {
            return;
        };
        if add {
            args.push(' ');
            args.push_str(value);
        } else {
            *args = value.to_string();
        }
    }

    /// The header arguments a block in `language` inherits, those of every
    /// language first, so its own win.
    pub fn of(&self, language: &str) -> String {
        let own = self.languages.get(language).map_or("", String::as_str);
        format!("{} {own}", self.every)
    }
}

/// What a block of code with the header arguments `args` exports: the last
/// `:exports` in them, in lower case, or `code`, Org's default.
pub fn exports(args: &str) -> String {
    let mut words = args.split_whitespace();
    let mut exports = "code";
    while let Some(word) = words.next() {
        if word.eq_ignore_ascii_case(":exports")
            && let Some(value) = words.next()
        {
            exports = value;
        }
    }
    exports.to_ascii_lowercase()
}
