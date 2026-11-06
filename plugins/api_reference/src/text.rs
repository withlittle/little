//! Text a plugin reads from code: the code that defines something, and the
//! lines of its docs, without the indentation they share.

use std::ops::Range;

/// The code at `range` in `text`, from the start of its first line, without
/// the indentation of that line; or none if `range` isn't in `text`.
pub fn source(text: &str, range: Range<usize>) -> Option<String> {
    let start = (text.get(..range.start)?.rfind('\n')).map_or(0, |newline| newline + 1);
    let code = text.get(start..range.end)?;
    let whitespace = |line: &str| line.len() - line.trim_start_matches([' ', '\t']).len();
    let margin = code.lines().next().map_or(0, whitespace);
    let lines: Vec<&str> = (code.lines())
        .map(|line| &line[margin.min(whitespace(line))..])
        .collect();
    Some(lines.join("\n"))
}

/// `lines` without the margin of spaces and tabs they share. Blank lines
/// don't count, and those shorter than the margin are left empty.
pub fn dedent<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    let margin = (lines.iter())
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    (lines.iter())
        .map(|line| line.get(margin..).unwrap_or_default())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_starts_at_its_first_line_without_its_indentation() {
        let text = "class A:\n    def f(self):\n        pass\n";
        let code = "def f(self):\n    pass".to_string();
        assert_eq!(source(text, 13..38), Some(code));
        assert_eq!(source(text, 13..99), None);
    }

    #[test]
    fn dedent_takes_off_the_margin_lines_share() {
        assert_eq!(dedent(&["    a", "  ", "      b"]), ["a", "", "  b"]);
        assert_eq!(dedent(&["\ta", "\t\tb"]), ["a", "\tb"]);
    }
}
