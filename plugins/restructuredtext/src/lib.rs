//! A little plugin that builds a page from each reStructuredText file, an
//! `.rst` file, as docutils would: it reads the file with its own parser,
//! which follows docutils' specification, without Sphinx's additions, into
//! a [`little_markup::Document`], which the markup library writes as the
//! Markdown the site's settings allow.
//!
//! The parser reads a file in steps: [`lines`] splits it into lines,
//! [`body`] reads their blocks, with [`explicit`], [`directives`] and
//! [`tables`], and [`inline`] the text in them, into the tree of
//! [`tree`]; [`resolve`] then leads each reference to its target, and
//! [`convert`] makes the tree a document, with its title and docinfo.

// Built for anything but WebAssembly, the plugin is only its tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod body;
mod convert;
mod data;
mod directives;
mod explicit;
mod inline;
mod lines;
mod lists;
mod resolve;
mod state;
mod tables;
mod text;
mod tree;
mod uri;

use std::time::{SystemTime, UNIX_EPOCH};

use little_markup::{Part, Settings, Warning, split_frontmatter};

/// Check the plugin's settings, `json`, as little gives them to it: it has
/// none, but the `read` every plugin has.
fn check_settings(json: &str) -> Result<(), String> {
    little_markup::check_settings(json, "reStructuredText")
}

/// The parts of the page of the reStructuredText file `text`, written in
/// the Markdown that `markdown`, the site's `markdown` settings, as JSON,
/// allow, and the warnings of what's left out or changed, each with its
/// line in the file: the parser's, then the writer's.
fn read(text: &str, markdown: &str) -> Result<(Vec<Part>, Vec<Warning>), String> {
    let settings = Settings::from_json(markdown)?;
    let (frontmatter, rest, before) = split_frontmatter(text).unwrap_or(("", text, 0));
    let (document, mut warnings) = convert::document(rest, frontmatter, Clock::now());
    for warning in &mut warnings {
        warning.line = warning.line.map(|line| line + before);
    }
    let (parts, written) = little_markup::write(document, &settings);
    warnings.extend(written);
    Ok((parts, warnings))
}

/// The time a page is built at, in UTC, as the `date` directive writes it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Clock {
    /// The seconds since the Unix epoch.
    pub seconds: u64,
}

impl Clock {
    /// The time now: in little, the time its build started.
    fn now() -> Clock {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        Clock { seconds }
    }

    /// The year, month and day, from 1, of the date.
    pub fn date(self) -> (i64, u32, u32) {
        // Howard Hinnant's `civil_from_days`.
        let days = (self.seconds / 86_400) as i64 + 719_468;
        let era = days.div_euclid(146_097);
        let of_era = days.rem_euclid(146_097);
        let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
        let day_of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let shifted = (5 * day_of_year + 2) / 153;
        let day = (day_of_year - (153 * shifted + 2) / 5 + 1) as u32;
        let month = if shifted < 10 {
            shifted + 3
        } else {
            shifted - 9
        } as u32;
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        (year, month, day)
    }

    /// The hour, minute and second of the time.
    pub fn time(self) -> (u64, u64, u64) {
        let of_day = self.seconds % 86_400;
        (of_day / 3600, of_day / 60 % 60, of_day % 60)
    }

    /// The day of the week, from 0, Monday, to 6, Sunday.
    pub fn weekday(self) -> u64 {
        // 1 January 1970 was a Thursday.
        (self.seconds / 86_400 + 3) % 7
    }
}

little_markup::plugin!("rst");

#[cfg(test)]
mod tests;
