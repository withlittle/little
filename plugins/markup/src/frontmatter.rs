//! YAML frontmatter, between `---` lines at the top of a page, as little
//! reads it from a page in Markdown.

/// Splits YAML frontmatter from the top of `text`, if it has some: a line
/// of `---`, then lines of YAML whose first, but for blank lines and
/// comments, is a key, like `title:`, and a line of `---`. Gives back the
/// YAML, the rest of `text`, and how many lines come before it, so a line
/// of the rest can be told as a line of the file. Asking for a key keeps a
/// title over- and underlined with `---`, as reStructuredText writes one,
/// as it is.
pub fn split_frontmatter(text: &str) -> Option<(&str, &str, usize)> {
    let mut lines = text.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim() != "---" {
        return None;
    }
    let start = first.len();
    let mut end = start;
    let mut keyed = false;
    for (index, line) in lines.enumerate() {
        let trimmed = line.trim();
        if trimmed == "---" {
            return keyed.then(|| (&text[start..end], &text[end + line.len()..], index + 2));
        }
        if !keyed && !trimmed.is_empty() && !trimmed.starts_with('#') {
            if !is_key(line) {
                return None;
            }
            keyed = true;
        }
        end += line.len();
    }
    None
}

/// Whether `line` starts a key of a YAML map, like `title: Guide`.
fn is_key(line: &str) -> bool {
    let line = line.trim_end();
    !line.starts_with([' ', '\t', '-'])
        && (line.ends_with(':') || line.contains(": ") || line.contains(":\t"))
}

/// The frontmatter of a page: its own YAML, then each key of `metadata` it
/// doesn't set, as a string, between `---` lines; or nothing, if it has
/// neither.
pub(crate) fn frontmatter(yaml: &str, metadata: &[(String, String)]) -> String {
    let mut out = yaml.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    for (key, value) in metadata {
        let set = yaml.lines().any(|line| {
            line.strip_prefix(key.as_str())
                .is_some_and(|rest| rest.trim_start().starts_with(':'))
        });
        if !set {
            let value = serde_json::to_string(value).expect("a string is JSON");
            out.push_str(&format!("{key}: {value}\n"));
        }
    }
    if out.trim().is_empty() {
        String::new()
    } else {
        format!("---\n{out}---\n")
    }
}
