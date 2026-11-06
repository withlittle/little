//! The `#fragment` of each heading, worked out as comrak works out its id
//! when little renders the page.

use finl_unicode::categories::CharacterCategories;
use std::collections::HashSet;

/// The id of the next heading on the page, whose text is `text`, as
/// comrak's anchorizer gives it: lower case, with only letters, marks,
/// numbers, `_`, `-`, and spaces, which become `-`, and `-1`, `-2`, and so
/// on after an id in `taken`, which it's added to.
pub(crate) fn slug(taken: &mut HashSet<String>, text: &str) -> String {
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
    while taken.contains(&id) {
        uniq += 1;
        id = format!("{base}-{uniq}");
    }
    taken.insert(id.clone());
    id
}
