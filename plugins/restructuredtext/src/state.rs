//! What the parser knows of the document as it reads it: the names of its
//! targets and their ids, its footnotes, substitutions and roles, and the
//! warnings it gives.

use std::collections::{HashMap, HashSet};

use little_markup::Warning;

use crate::Clock;
use crate::tree::{Node, Span};

/// How deep blocks may nest, and substitutions refer to each other, before
/// the parser stops reading them as such, so it never runs out of stack.
pub(crate) const MAX_DEPTH: usize = 32;

/// Where a target leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Dest {
    /// A URL: an external target's.
    Url(String),
    /// The target with this name, normalized: an indirect target's.
    Name(String),
    /// Its own place, by its id: an internal target's, or a section's.
    Here,
    /// The footnote or citation with this index.
    Footnote(usize),
}

/// A target: where it leads, and its id.
#[derive(Debug, Clone)]
pub(crate) struct Target {
    pub dest: Dest,
    pub id: String,
}

/// The target a name leads to, or none, if two have it, and whether it was
/// given explicitly.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Name {
    pub target: Option<usize>,
    pub explicit: bool,
}

/// What a footnote is: numbered by hand, automatically, with a label or
/// not, with a symbol, or a citation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Kind {
    Manual,
    Auto { labelled: bool },
    Symbol,
    Citation,
}

/// A footnote or citation, its label, as written between its brackets,
/// and its line.
#[derive(Debug, Clone)]
pub(crate) struct Footnote {
    pub kind: Kind,
    pub label: String,
    pub blocks: Vec<Node>,
    pub line: usize,
}

/// What a substitution is replaced with, and whether it takes the
/// whitespace before or after it.
#[derive(Debug, Clone)]
pub(crate) struct Substitution {
    pub spans: Vec<Span>,
    pub ltrim: bool,
    pub rtrim: bool,
}

/// What an interpreted text role makes of its text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum Role {
    Emphasis,
    Strong,
    /// `literal`, which takes backslashes out.
    Literal,
    /// `code`, which keeps them.
    Code,
    Math,
    Sub,
    Sup,
    /// `title-reference`, the default.
    #[default]
    Title,
    /// Its text alone: `abbreviation`, `acronym`, or a custom role with no
    /// base.
    Plain,
    Pep,
    Rfc,
    /// `raw`: in a format with HTML, or another, or none, if it's used
    /// directly.
    Raw(Option<bool>),
    /// A role docutils knows, but doesn't implement.
    Unimplemented,
}

impl Role {
    /// The standard role named `name`, in lowercase, in English.
    pub fn standard(name: &str) -> Option<Role> {
        Some(match name {
            "emphasis" => Role::Emphasis,
            "strong" => Role::Strong,
            "literal" => Role::Literal,
            "code" => Role::Code,
            "math" => Role::Math,
            "subscript" | "sub" => Role::Sub,
            "superscript" | "sup" => Role::Sup,
            "title-reference" | "title" | "t" => Role::Title,
            "abbreviation" | "ab" | "acronym" | "ac" => Role::Plain,
            "pep-reference" | "pep" => Role::Pep,
            "rfc-reference" | "rfc" => Role::Rfc,
            "raw" => Role::Raw(None),
            "index"
            | "i"
            | "named-reference"
            | "anonymous-reference"
            | "footnote-reference"
            | "citation-reference"
            | "substitution-reference"
            | "target"
            | "uri-reference"
            | "uri"
            | "url" => Role::Unimplemented,
            _ => return None,
        })
    }
}

/// What the parser knows of the document.
#[derive(Debug, Default)]
pub(crate) struct State {
    pub warnings: Vec<Warning>,
    /// The ids given so far.
    ids: HashSet<String>,
    /// The last number given each prefix of an id.
    counters: HashMap<String, usize>,
    /// The target each name leads to.
    pub names: HashMap<String, Name>,
    pub targets: Vec<Target>,
    /// The anonymous targets, in order.
    pub anonymous: Vec<usize>,
    /// The anonymous references read so far.
    pub anonymous_refs: usize,
    pub footnotes: Vec<Footnote>,
    /// The references to footnotes numbered automatically without a
    /// label, `[#]_`, and to footnotes with symbols, `[*]_`, read so far.
    pub auto_refs: usize,
    pub symbol_refs: usize,
    /// The substitutions, by their names, as defined.
    pub substitutions: HashMap<String, Substitution>,
    /// The name each substitution is defined with, by that name in
    /// lowercase.
    pub substitution_names: HashMap<String, String>,
    /// The custom roles, by their names, in lowercase.
    pub roles: HashMap<String, Role>,
    /// The role of interpreted text without one, and its name.
    pub default_role: (String, Role),
    pub metadata: Vec<(String, String)>,
    /// The section titles' styles, in the order they first appear: the
    /// adornment's character, and whether it has an overline.
    pub styles: Vec<(char, bool)>,
    /// The level of the section being read, from 0, for none.
    pub section_level: usize,
    pub clock: Clock,
    /// How deep the blocks being read are nested.
    pub depth: usize,
    /// The depth of the content of the directive being read, if docutils
    /// reads it into an element of no kind, where a topic or a sidebar may
    /// be, as it may in a section; and of a sidebar's, where a topic may be,
    /// but not another sidebar.
    pub loose: Option<usize>,
    pub sidebar: Option<usize>,
    /// Whether blocks nested too deep have been warned of.
    deep: bool,
}

impl State {
    pub fn new(clock: Clock) -> State {
        State {
            default_role: ("title-reference".to_string(), Role::Title),
            clock,
            ..State::default()
        }
    }

    /// Warn of `message`, on `line`.
    pub fn warn(&mut self, line: usize, message: impl Into<String>) {
        self.warnings.push(Warning::new(Some(line), message));
    }

    /// Whether blocks nested one level deeper may be read as such: if not,
    /// it warns, the first time.
    pub fn may_nest(&mut self, line: usize) -> bool {
        if self.depth < MAX_DEPTH {
            return true;
        }
        if !self.deep {
            self.deep = true;
            self.warn(
                line,
                format!("blocks are nested over {MAX_DEPTH} deep; read the deeper ones as text"),
            );
        }
        false
    }

    /// A new id, for something with `names`, as docutils' `set_id` makes
    /// it: the first name's id that's free, else the last name's, or
    /// `prefix`, followed by the next number free.
    pub fn new_id(&mut self, names: &[String], prefix: &str) -> String {
        let mut base = String::new();
        for name in names {
            base = crate::text::make_id(name);
            if !base.is_empty() && !self.ids.contains(&base) {
                self.ids.insert(base.clone());
                return base;
            }
        }
        let prefix = format!("{}-", if base.is_empty() { prefix } else { &base });
        let counter = self.counters.entry(prefix.clone()).or_default();
        loop {
            *counter += 1;
            let id = format!("{prefix}{counter}");
            if self.ids.insert(id.clone()) {
                return id;
            }
        }
    }

    /// A new target, leading to `dest`, with `names`, explicit or not, and
    /// an id made from them, or `prefix`: its index.
    pub fn target(
        &mut self,
        dest: Dest,
        names: &[String],
        explicit: bool,
        prefix: &str,
        line: usize,
    ) -> usize {
        let id = self.new_id(names, prefix);
        let index = self.targets.len();
        self.targets.push(Target { dest, id });
        for name in names {
            self.name(name, index, explicit, line);
        }
        index
    }

    /// Give the target `index` the name `name`, as docutils does: an
    /// explicit name wins over an implicit one, and two of the same kind
    /// lead nowhere, but for explicit ones to the same URL.
    fn name(&mut self, name: &str, index: usize, explicit: bool, line: usize) {
        let Some(old) = self.names.get(name).copied() else {
            let new = Name {
                target: Some(index),
                explicit,
            };
            self.names.insert(name.to_string(), new);
            return;
        };
        let mut new = Name {
            target: old.target,
            explicit: old.explicit || explicit,
        };
        if explicit && old.explicit {
            let same = old.target.is_some_and(|old| {
                matches!(
                    (&self.targets[old].dest, &self.targets[index].dest),
                    (Dest::Url(a), Dest::Url(b)) if a == b
                )
            });
            if !same {
                new.target = None;
                self.warn(line, format!("duplicate explicit target name: \"{name}\""));
            }
        } else if explicit {
            new.target = Some(index);
        } else if !old.explicit {
            new.target = None;
        }
        self.names.insert(name.to_string(), new);
    }
}
