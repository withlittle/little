//! The `#fragment` of each heading, worked out as comrak works out its id
//! when little renders the page.

use finl_unicode::categories::CharacterCategories;
use std::collections::HashSet;

/// Gives each heading's text its id, as comrak's anchorizer does: lower case,
/// with only letters, marks, numbers, `_`, `-`, and spaces, which become
/// `-`, and `-1`, `-2`, and so on after an id that's taken.
#[derive(Debug, Default)]
pub(crate) struct Slugger(HashSet<String>);

impl Slugger {
    /// The id of the next heading on the page, whose text is `text`.
    pub fn slug(&mut self, text: &str) -> String {
        let base: String = text
            .to_lowercase()
            .chars()
            .filter(|&c| {
                c == ' '
                    || c == '-'
                    || c.is_letter()
                    || c.is_mark()
                    || c.is_number()
                    || c.is_punctuation_connector()
            })
            .map(|c| if c == ' ' { '-' } else { c })
            .collect();
        let mut id = base.clone();
        let mut uniq = 0;
        while self.0.contains(&id) {
            uniq += 1;
            id = format!("{base}-{uniq}");
        }
        self.0.insert(id.clone());
        id
    }
}
