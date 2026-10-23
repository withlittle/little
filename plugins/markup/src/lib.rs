//! A page, as little's plugins for other formats read it, and the Markdown
//! it's written as: only what the site's `markdown` settings turn on,
//! escaped, so it reads back as it was.
//!
//! A plugin reads its format into a [`Document`], and gives it to
//! [`write`], with the site's [`Settings`], for the parts of its page, and
//! what it lost on the way, to warn of.

mod block;
mod frontmatter;
mod inline;
mod model;
mod normalize;
mod settings;
mod slug;
mod targets;
#[cfg(test)]
mod tests;

pub use frontmatter::split_frontmatter;
pub use model::*;
pub use settings::Settings;

/// A part of a page, as the `formats` hook gives it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    /// Markdown, read on its own.
    Markdown(String),
    /// HTML, placed as it is.
    Html(String),
}

/// Something a page loses as it's written, to warn of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// The line of the file it's on, if it's known.
    pub line: Option<usize>,
    /// What's lost.
    pub message: String,
}

impl Warning {
    /// A warning of `message`, on `line`, if it's known.
    pub fn new(line: Option<usize>, message: impl Into<String>) -> Self {
        Warning {
            line,
            message: message.into(),
        }
    }

    /// The warning, as little shows it: the page's path, its line, if it's
    /// known, and what's lost.
    pub fn display(&self, page: &str) -> String {
        match self.line {
            Some(line) => format!("{page}:{line}: {}", self.message),
            None => format!("{page}: {}", self.message),
        }
    }
}

/// The warnings of a page, each once.
#[derive(Debug, Default)]
pub(crate) struct Warnings(pub Vec<Warning>);

impl Warnings {
    pub fn push(&mut self, line: Option<usize>, message: impl Into<String>) {
        let warning = Warning::new(line, message);
        if !self.0.contains(&warning) {
            self.0.push(warning);
        }
    }
}

/// Writes `document` as the parts of its page, in Markdown that `settings`
/// allow, with the warnings of what it loses: a part of HTML for each block
/// of raw HTML at its top, so it's placed whole, and one of Markdown for
/// what's around them, the first with its frontmatter.
pub fn write(mut document: Document, settings: &Settings) -> (Vec<Part>, Vec<Warning>) {
    let mut warnings = Warnings::default();
    targets::fill_links(&mut document);
    normalize::Normalizer::new(settings, &document, &mut warnings).document(&mut document);
    let targets = targets::Targets::new(&document, settings);
    let parts = block::Writer::new(settings, &targets, &document, &mut warnings).write();
    (parts, warnings.0)
}
