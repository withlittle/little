//! Turn a raw Markdown file (with optional YAML frontmatter) into
//! frontmatter data + an HTML fragment.

use anyhow::{Context, Result};
use serde_yaml::Value;

/// Parse a document into its frontmatter and rendered HTML body.
pub fn parse_document(raw: &str) -> Result<(Value, String)> {
    let (frontmatter_src, body) = split_frontmatter(raw);

    let frontmatter = if frontmatter_src.trim().is_empty() {
        Value::Null
    } else {
        serde_yaml::from_str(&frontmatter_src).context("parsing frontmatter")?
    };

    Ok((frontmatter, markdown_to_html(&body)))
}

/// Split a `---` fenced YAML frontmatter block from the body.
fn split_frontmatter(raw: &str) -> (String, String) {
    let first_line = raw.lines().next().map(str::trim);
    if first_line != Some("---") {
        return (String::new(), raw.to_string());
    }

    let mut frontmatter = String::new();
    let mut body = String::new();
    let mut in_frontmatter = true;
    let mut closed = false;

    for (i, line) in raw.lines().enumerate() {
        if i == 0 {
            continue; // skip the opening `---`
        }
        if in_frontmatter && line.trim() == "---" {
            in_frontmatter = false;
            closed = true;
            continue;
        }
        if in_frontmatter {
            frontmatter.push_str(line);
            frontmatter.push('\n');
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }

    if closed {
        (frontmatter, body)
    } else {
        (String::new(), raw.to_string())
    }
}

fn markdown_to_html(body: &str) -> String {
    let mut options = comrak::Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    comrak::markdown_to_html(body, &options)
}
