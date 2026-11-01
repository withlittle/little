//! Org's tables, `| a | b |`, as Markdown's, as Org's HTML export shows
//! them: without their special rows and columns, with a row of headers,
//! if a rule ends their first rows, and each column aligned as its cookie,
//! like `<r>`, says, or else to the right, if most of its cells are
//! numbers.

use little_markup::{Align, Block, Cell, Table};
use orgize::{SyntaxElement, SyntaxKind, SyntaxNode};

use crate::convert::{Converter, start};

/// The marks in the first column of a table that make it special, a column
/// for Org's spreadsheet, which export leaves out.
const SPECIAL_COLUMN: [&str; 7] = ["/", "#", "!", "$", "*", "_", "^"];

impl Converter<'_> {
    /// The table `node`, as a [`Block::Table`].
    pub(crate) fn table(&mut self, node: &SyntaxNode) -> Block {
        // Each row, or `None` for a rule.
        let rows: Vec<Option<Vec<SyntaxNode>>> = node
            .children()
            .filter_map(|child| match child.kind() {
                SyntaxKind::ORG_TABLE_RULE_ROW => Some(None),
                SyntaxKind::ORG_TABLE_STANDARD_ROW => Some(Some(
                    child
                        .children()
                        .filter(|cell| cell.kind() == SyntaxKind::ORG_TABLE_CELL)
                        .collect(),
                )),
                _ => None,
            })
            .collect();
        let line = node
            .children()
            .find(|child| child.kind() == SyntaxKind::ORG_TABLE_STANDARD_ROW)
            .map_or_else(|| self.line(start(node)), |row| self.line(start(&row)));
        let mut marked = false;
        let special_column = rows.iter().flatten().all(|cells| {
            let first = cells.first().map(raw).unwrap_or_default();
            marked |= SPECIAL_COLUMN.contains(&first.as_str());
            first.is_empty() || SPECIAL_COLUMN.contains(&first.as_str())
        }) && marked;
        let mut cookies: Vec<Option<Align>> = Vec::new();
        let mut groups: Vec<Vec<Vec<SyntaxNode>>> = vec![Vec::new()];
        for row in rows {
            let Some(mut cells) = row else {
                groups.push(Vec::new());
                continue;
            };
            let first = cells.first().map(raw).unwrap_or_default();
            if first == "/" || (special_column && ["^", "_", "$", "!"].contains(&first.as_str())) {
                continue;
            }
            if special_column && !cells.is_empty() {
                cells.remove(0);
            }
            if let Some(row) = cookie_row(&cells) {
                if cookies.len() < row.len() {
                    cookies.resize(row.len(), None);
                }
                for (column, align) in row.into_iter().enumerate() {
                    if align.is_some() {
                        cookies[column] = align;
                    }
                }
                continue;
            }
            if let Some(group) = groups.last_mut() {
                group.push(cells);
            }
        }
        groups.retain(|group| !group.is_empty());
        let columns = groups.iter().flatten().map(Vec::len).max().unwrap_or(0);
        let align = (0..columns)
            .map(|column| match cookies.get(column).copied().flatten() {
                Some(align) => align,
                None => numeric(groups.iter().flatten(), column),
            })
            .collect();
        let mut groups = groups.into_iter();
        let mut header = None;
        let mut rows = Vec::new();
        if groups.len() >= 2
            && let Some(mut first) = groups.next()
        {
            let head = first.remove(0);
            if let Some(next) = first.first() {
                self.warn(
                    start(&next[0]),
                    "joined the table's header rows after the first to its body, as Markdown's \
                     tables have one header row",
                );
            }
            header = Some(self.cells(&head, columns));
            for row in first {
                rows.push(self.cells(&row, columns));
            }
        }
        for row in groups.flatten() {
            rows.push(self.cells(&row, columns));
        }
        Block::Table(Table {
            line: Some(line),
            align,
            header,
            rows,
        })
    }

    /// The cells of a row, `cells`, as blocks, with empty ones after them
    /// up to `columns`.
    fn cells(&mut self, cells: &[SyntaxNode], columns: usize) -> Vec<Cell> {
        let mut row: Vec<Cell> = cells
            .iter()
            .map(|cell| {
                let children: Vec<SyntaxElement> = cell.children_with_tokens().collect();
                let inlines = self.objects(&children, true);
                if inlines.is_empty() {
                    Vec::new()
                } else {
                    vec![Block::Paragraph(inlines)]
                }
            })
            .collect();
        row.resize_with(columns, Vec::new);
        row
    }
}

/// The text of `cell`, trimmed.
fn raw(cell: &SyntaxNode) -> String {
    cell.to_string().trim().to_string()
}

/// How the row `cells` aligns each column, if it's a row of cookies, like
/// `<l>` or `<10>`, and empty cells, with at least one cookie.
fn cookie_row(cells: &[SyntaxNode]) -> Option<Vec<Option<Align>>> {
    let mut any = false;
    let mut row = Vec::new();
    for cell in cells {
        let text = raw(cell);
        if text.is_empty() {
            row.push(None);
            continue;
        }
        let inner = text.strip_prefix('<')?.strip_suffix('>')?;
        let (align, width) = match inner.chars().next() {
            Some('l') => (Some(Align::Left), &inner[1..]),
            Some('c') => (Some(Align::Center), &inner[1..]),
            Some('r') => (Some(Align::Right), &inner[1..]),
            _ => (None, inner),
        };
        if !width.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        any = true;
        row.push(align);
    }
    any.then_some(row)
}

/// How the column `column` of `rows` is aligned without a cookie: to the
/// right, if at least half its cells are numbers, counting an empty cell
/// after a number as one, as Org's export does, or else as the theme
/// aligns it.
fn numeric<'a>(rows: impl Iterator<Item = &'a Vec<SyntaxNode>>, column: usize) -> Align {
    let (mut numbers, mut total, mut previous) = (0, 0, false);
    for row in rows {
        let text = row.get(column).map(raw).unwrap_or_default();
        total += 1;
        previous = is_number(&text) || (text.is_empty() && previous);
        if previous {
            numbers += 1;
        }
    }
    if total > 0 && numbers * 2 >= total {
        Align::Right
    } else {
        Align::None
    }
}

/// Whether `text` is a number, as `org-table-number-regexp` matches one,
/// like `12`, `-1.5e3`, `50%`, `0xff`, `2#101`, or `inf`.
fn is_number(text: &str) -> bool {
    if ["nan", "inf", "-inf", "+inf", "uinf"].contains(&text) {
        return true;
    }
    let text = text.strip_prefix(['<', '>']).unwrap_or(text);
    let signed = text.strip_prefix(['-', '+']).unwrap_or(text);
    if let Some(hex) = signed
        .strip_prefix("0x")
        .or_else(|| signed.strip_prefix("0X"))
        && !hex.is_empty()
        && hex.chars().all(|c| c.is_ascii_hexdigit() || c == '.')
    {
        return true;
    }
    if let Some((base, digits)) = signed.split_once('#')
        && !base.is_empty()
        && base.chars().all(|c| c.is_ascii_digit())
        && !digits.is_empty()
        && digits
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.')
    {
        return true;
    }
    let Some(first) = text.find(|c: char| c.is_ascii_digit()) else {
        return false;
    };
    text[..first].chars().all(|c| "-+^.".contains(c))
        && text[first..]
            .chars()
            .all(|c| c.is_ascii_digit() || "-+^.eEdDx()%:".contains(c))
}
