//! Grid tables and simple tables, found and read as docutils' table
//! parsers read them, and the tables of the table directives: each a table
//! of rows of cells, each cell the blocks of its text.

use std::collections::BTreeSet;
use std::ops::Range;

use crate::body::{Body, code, nested};
use crate::lines::{Line, char_width, is_space, join, skip};
use crate::state::State;
use crate::tree::{Node, Table};

/// Whether `text` is a grid table's top border, like `+---+---+`.
pub(crate) fn grid_top(text: &str) -> bool {
    grid_border(text, b'-')
}

/// Whether `text` is a simple table's top border, like `===  ===`, with two
/// columns or more.
pub(crate) fn simple_top(text: &str) -> bool {
    simple_border(text, b'=') && text.trim_end_matches(' ').contains(' ')
}

/// Whether `text` is a border of a grid table drawn with `fill`, like
/// `+---+` or `+===+`.
fn grid_border(text: &str, fill: u8) -> bool {
    let bytes = text.trim_end_matches(' ').as_bytes();
    bytes.len() >= 5
        && bytes[..2] == [b'+', fill]
        && bytes[bytes.len() - 2..] == [fill, b'+']
        && bytes.iter().all(|&b| b == b'+' || b == fill)
}

/// Whether `text` is a border or rule of a simple table drawn with `fill`,
/// like `===  ===` or `---  ---`.
fn simple_border(text: &str, fill: u8) -> bool {
    text.as_bytes().first() == Some(&fill) && text.bytes().all(|b| b == fill || b == b' ')
}

/// A table of `rows`, at `line`, the first `header_rows` of them its
/// headers: the first as Markdown's row of headers, and any others joined
/// to its body, and warned of.
pub(crate) fn table(
    state: &mut State,
    line: usize,
    mut rows: Vec<Vec<Vec<Node>>>,
    header_rows: usize,
) -> Node {
    if header_rows > 1 {
        let message = "joined the table's header rows after the first to its body, as \
                       Markdown's tables have one header row";
        state.warn(line, message);
    }
    let header = (header_rows > 0 && !rows.is_empty()).then(|| rows.remove(0));
    Node::Table(Table { line, header, rows })
}

/// A line of a table, in the columns docutils counts: one for each
/// character but a combining one, which goes with the character before it,
/// and another after a wide one, as docutils pads it with a NUL.
struct Columns {
    text: String,
    /// Where each column starts in `text`, and its character: a NUL for
    /// the column after a wide one.
    starts: Vec<(usize, char)>,
}

impl Columns {
    fn new(text: &str) -> Self {
        let mut starts = Vec::new();
        for (at, c) in text.char_indices() {
            match char_width(c) {
                0 => {}
                2 => starts.extend([(at, c), (at + c.len_utf8(), '\0')]),
                _ => starts.push((at, c)),
            }
        }
        Columns {
            text: text.to_string(),
            starts,
        }
    }

    fn len(&self) -> usize {
        self.starts.len()
    }

    /// The character in `column`, or a space past the end.
    fn get(&self, column: usize) -> char {
        self.starts.get(column).map_or(' ', |&(_, c)| c)
    }

    /// Where `column` starts in the text.
    fn start(&self, column: usize) -> usize {
        self.starts
            .get(column)
            .map_or(self.text.len(), |&(at, _)| at)
    }

    /// The text from column `from` to before column `to`.
    fn slice(&self, from: usize, to: usize) -> &str {
        let from = self.start(from);
        &self.text[from..self.start(to).max(from)]
    }

    /// Whether there's only whitespace from column `from` to before `to`.
    fn blank(&self, from: usize, to: usize) -> bool {
        let to = to.min(self.len());
        (from..to).all(|column| is_space(self.get(column)))
    }

    /// The column after the last that isn't whitespace.
    fn end(&self) -> usize {
        let text = self.starts.iter().rposition(|&(_, c)| !is_space(c));
        text.map_or(0, |column| column + 1)
    }
}

/// A cell that a table's parser finds: the rows and columns it takes after
/// its first, the number of its first line, and its lines.
struct Cell {
    more_rows: usize,
    more_columns: usize,
    line: usize,
    lines: Vec<Line>,
}

/// The rows of a table, each with a cell, or `None` where another cell
/// takes the place, in each column, and how many of them are its headers.
struct Parsed {
    rows: Vec<Vec<Option<Cell>>>,
    header_rows: usize,
}

/// Why a table is malformed, and the line of the table it's at, from 0.
struct Malformed {
    detail: String,
    offset: usize,
}

fn malformed(detail: impl Into<String>, offset: usize) -> Malformed {
    Malformed {
        detail: detail.into(),
        offset,
    }
}

/// The lines of a cell: those of `rows`, from column `left` to before
/// `right`, without trailing whitespace, or the indent they share.
fn cell_lines(
    lines: &[Columns],
    block: &[Line],
    rows: Range<usize>,
    left: usize,
    right: usize,
) -> Vec<Line> {
    let mut cell: Vec<Line> = rows
        .map(|row| Line {
            text: lines[row]
                .slice(left, right)
                .trim_end_matches(is_space)
                .to_string(),
            num: block[row].num,
        })
        .collect();
    let indent = cell
        .iter()
        .filter(|line| !line.text.is_empty())
        .map(|line| line.text.chars().take_while(|&c| is_space(c)).count())
        .min();
    if let Some(indent) = indent
        && indent > 0
    {
        for line in &mut cell {
            line.text = skip(&line.text, indent).to_string();
        }
    }
    cell
}

/// The head/body separator of a table, the one line in `lines` that
/// `separator` matches, with its `=`s made `-`s.
fn separator(
    lines: &mut [Columns],
    is_separator: impl Fn(&str) -> bool,
) -> Result<Option<usize>, Malformed> {
    let mut found = None;
    for (index, line) in lines.iter_mut().enumerate() {
        if !is_separator(&line.text) {
            continue;
        }
        if let Some(first) = found {
            let detail = format!(
                "multiple head/body row separators (table lines {} and {}); only one allowed",
                first + 1,
                index + 1
            );
            return Err(malformed(detail, index));
        }
        found = Some(index);
        *line = Columns::new(&line.text.replace('=', "-"));
    }
    let last = lines.len() - 1;
    if found.is_some_and(|index| index == 0 || index == last) {
        let detail = "the head/body row separator may not be the first or last line of the table";
        return Err(malformed(detail, last));
    }
    Ok(found)
}

/// The rows of a grid table, `block`, as docutils' `GridTableParser`
/// finds them: tracing each cell from its top-left corner, right, down,
/// left, and up, from the table's own, then those of the cells it finds.
fn parse_grid(block: &[Line]) -> Result<Parsed, Malformed> {
    let mut lines: Vec<Columns> = block.iter().map(|line| Columns::new(&line.text)).collect();
    let separator = separator(&mut lines, |text| grid_border(text, b'='))?;
    let bottom = lines.len() - 1;
    let right = lines[0].len().saturating_sub(1);
    // The last row of each column a cell has been found in.
    let mut done = vec![-1_isize; right + 1];
    let mut corners = BTreeSet::from([(0, 0)]);
    let mut cells = Vec::new();
    let mut row_seps = BTreeSet::from([0]);
    let mut column_seps = BTreeSet::from([0]);
    while let Some((top, left)) = corners.pop_first() {
        if top == bottom || left >= right || top as isize <= done[left] {
            continue;
        }
        let Some(cell) = scan(&lines, top, left, bottom, right) else {
            continue;
        };
        row_seps.extend(cell.rows);
        column_seps.extend(cell.columns);
        // docutils asserts that each column of the cell was done to the row
        // above it.
        for column in &mut done[left..cell.right] {
            if *column != top as isize - 1 {
                return Err(malformed("parse incomplete", 0));
            }
            *column = cell.bottom as isize - 1;
        }
        cells.push((top, left, cell.bottom, cell.right));
        corners.insert((top, cell.right));
        corners.insert((cell.bottom, left));
    }
    if done[..right].iter().any(|&row| row != bottom as isize - 1) {
        return Err(malformed("parse incomplete", 0));
    }
    let row_seps: Vec<usize> = row_seps.into_iter().collect();
    let column_seps: Vec<usize> = column_seps.into_iter().collect();
    let index = |seps: &[usize], at: usize| {
        seps.binary_search(&at)
            .map_err(|_| malformed("parse incomplete", 0))
    };
    let columns = column_seps.len() - 1;
    let mut rows: Vec<Vec<Option<Cell>>> = (0..row_seps.len() - 1)
        .map(|_| (0..columns).map(|_| None).collect())
        .collect();
    let mut remaining = (rows.len() * columns) as isize;
    for (top, left, cell_bottom, cell_right) in cells {
        let row = index(&row_seps, top)?;
        let column = index(&column_seps, left)?;
        let more_rows = index(&row_seps, cell_bottom)? - row - 1;
        let more_columns = index(&column_seps, cell_right)? - column - 1;
        let place = &mut rows[row][column];
        if place.is_some() {
            let detail = format!("cell (row {}, column {}) already used", row + 1, column + 1);
            return Err(malformed(detail, 0));
        }
        remaining -= ((more_rows + 1) * (more_columns + 1)) as isize;
        *place = Some(Cell {
            more_rows,
            more_columns,
            line: block[top + 1].num,
            lines: cell_lines(&lines, block, top + 1..cell_bottom, left + 1, cell_right),
        });
    }
    if remaining != 0 {
        return Err(malformed("unused cells remaining", 0));
    }
    let header_rows = match separator {
        Some(at) => index(&row_seps, at)?,
        None => 0,
    };
    Ok(Parsed { rows, header_rows })
}

/// A cell traced from its top-left corner: its bottom and right, and the
/// rows and columns it found borders at.
struct Traced {
    bottom: usize,
    right: usize,
    rows: Vec<usize>,
    columns: Vec<usize>,
}

/// The cell whose top-left corner is at `top` and `left`, traced right
/// along its top to each `+`, then down from it, as docutils traces it.
fn scan(lines: &[Columns], top: usize, left: usize, bottom: usize, right: usize) -> Option<Traced> {
    if lines[top].get(left) != '+' {
        return None;
    }
    let mut columns = Vec::new();
    for corner in left + 1..=right {
        match lines[top].get(corner) {
            '+' => {
                columns.push(corner);
                if let Some(mut cell) = scan_down(lines, top, left, corner, bottom) {
                    cell.columns.extend(columns);
                    return Some(cell);
                }
            }
            '-' => {}
            _ => return None,
        }
    }
    None
}

/// The cell with its top-right corner at `top` and `right`, traced down
/// to each `+`, then left from it.
fn scan_down(
    lines: &[Columns],
    top: usize,
    left: usize,
    right: usize,
    bottom: usize,
) -> Option<Traced> {
    let mut rows = Vec::new();
    for corner in top + 1..=bottom {
        match lines[corner].get(right) {
            '+' => {
                rows.push(corner);
                if let Some(mut cell) = scan_left(lines, top, left, corner, right) {
                    cell.rows.extend(rows);
                    return Some(cell);
                }
            }
            '|' => {}
            _ => return None,
        }
    }
    None
}

/// The cell with its bottom-right corner at `bottom` and `right`, if its
/// bottom leads left to its bottom-left corner, and its left up to its
/// top-left one.
fn scan_left(
    lines: &[Columns],
    top: usize,
    left: usize,
    bottom: usize,
    right: usize,
) -> Option<Traced> {
    let mut columns = Vec::new();
    for column in (left + 1..right).rev() {
        match lines[bottom].get(column) {
            '+' => columns.push(column),
            '-' => {}
            _ => return None,
        }
    }
    if lines[bottom].get(left) != '+' {
        return None;
    }
    let mut rows = Vec::new();
    for row in (top + 1..bottom).rev() {
        match lines[row].get(left) {
            '+' => rows.push(row),
            '|' => {}
            _ => return None,
        }
    }
    Some(Traced {
        bottom,
        right,
        rows,
        columns,
    })
}

/// A simple table's parser, as docutils' `SimpleTableParser`: its lines,
/// the columns of its top border, the last of them widened by any text
/// that runs past it, and the rows it's found, each with its first line.
struct Simple<'b> {
    block: &'b [Line],
    lines: Vec<Columns>,
    columns: Vec<(usize, usize)>,
    /// Where the top border's last column ends.
    border_end: usize,
    rows: Vec<(usize, Vec<Option<Cell>>)>,
}

/// The rows of a simple table, `block`, as docutils' `SimpleTableParser`
/// finds them: a row starts at each line with text in its first column,
/// or after a rule of `-`s, which spans the cells of the row above it.
fn parse_simple(block: &[Line]) -> Result<Parsed, Malformed> {
    let mut lines: Vec<Columns> = block.iter().map(|line| Columns::new(&line.text)).collect();
    let last = lines.len() - 1;
    for index in [0, last] {
        lines[index] = Columns::new(&lines[index].text.replace('=', "-"));
    }
    let separator = separator(&mut lines, |text| simple_border(text, b'='))?;
    let mut table = Simple {
        block,
        lines,
        columns: Vec::new(),
        border_end: 0,
        rows: Vec::new(),
    };
    table.columns = table.spans(0)?;
    table.border_end = table.columns.last().map_or(0, |&(_, end)| end);
    let (first_start, first_end) = table.columns[0];
    let mut start = 1;
    let mut text_found = false;
    for offset in 1..table.lines.len() {
        let line = &table.lines[offset];
        if simple_border(&line.text, b'-') {
            table.row(start..offset, Some(offset))?;
            start = offset + 1;
            text_found = false;
        } else if !line.blank(first_start, first_end) {
            if text_found && offset != start {
                table.row(start..offset, None)?;
            }
            start = offset;
            text_found = true;
        } else if !text_found {
            start = offset + 1;
        }
    }
    let header_rows = match separator {
        Some(at) => table
            .rows
            .iter()
            .position(|&(start, _)| start > at)
            .unwrap_or(0),
        None => 0,
    };
    let rows = table.rows.into_iter().map(|(_, row)| row).collect();
    Ok(Parsed { rows, header_rows })
}

impl Simple<'_> {
    /// The columns of the rule at `offset`, each a run of `-`s, the last
    /// reaching as far as the table's; or an error if it doesn't reach the
    /// end of the top border's.
    fn spans(&self, offset: usize) -> Result<Vec<(usize, usize)>, Malformed> {
        // A rule is all `-`s and spaces, so its bytes are its columns.
        let text = &self.lines[offset].text;
        let mut spans = Vec::new();
        let mut end = 0;
        while let Some(begin) = text[end..].find('-').map(|at| end + at) {
            end = text[begin..].find(' ').map_or(text.len(), |at| begin + at);
            spans.push((begin, end));
        }
        if let Some(&(_, main_end)) = self.columns.last()
            && let Some(last) = spans.last_mut()
        {
            if last.1 != self.border_end {
                let detail = format!("column span incomplete in table line {}", offset + 1);
                return Err(malformed(detail, offset));
            }
            last.1 = main_end;
        }
        Ok(spans)
    }

    /// Adds the row of the lines `rows`, in the columns of the rule at
    /// `rule`, if it has one, else the table's.
    fn row(&mut self, rows: Range<usize>, rule: Option<usize>) -> Result<(), Malformed> {
        if rows.is_empty() && rule.is_none() {
            return Ok(());
        }
        let mut columns = match rule {
            Some(offset) => self.spans(offset)?,
            None => self.columns.clone(),
        };
        self.check_columns(rows.clone(), &mut columns)?;
        let spans = self.init_row(&columns, rows.start)?;
        let mut row = Vec::new();
        for (&(left, right), more_columns) in columns.iter().zip(spans) {
            row.push(Some(Cell {
                more_rows: 0,
                more_columns,
                line: self.block[rows.start].num,
                lines: cell_lines(&self.lines, self.block, rows.clone(), left, right),
            }));
            row.extend((0..more_columns).map(|_| None));
        }
        self.rows.push((rows.start, row));
        Ok(())
    }

    /// Widens the last of `columns`, and the table's, to the text of
    /// `rows` that runs past it, or an error for text between columns.
    fn check_columns(
        &mut self,
        rows: Range<usize>,
        columns: &mut [(usize, usize)],
    ) -> Result<(), Malformed> {
        let last = columns.len() - 1;
        for index in 0..columns.len() {
            let (start, end) = columns[index];
            let next_start = columns
                .get(index + 1)
                .map_or(usize::MAX, |&(start, _)| start);
            for row in rows.clone() {
                let line = &self.lines[row];
                if index == last && !line.blank(end, usize::MAX) {
                    let new_end = line.end();
                    let main = self.columns.last_mut().expect("a table has columns");
                    columns[index] = (start, main.1.max(new_end));
                    main.1 = main.1.max(new_end);
                } else if !line.blank(end, next_start) {
                    let detail = format!("text in column margin in table line {}", row + 1);
                    return Err(malformed(detail, row));
                }
            }
        }
        Ok(())
    }

    /// The columns of the table each of `columns` takes after its first,
    /// or an error if one doesn't start and end with the table's.
    fn init_row(&self, columns: &[(usize, usize)], start: usize) -> Result<Vec<usize>, Malformed> {
        let problem = || {
            let detail = format!("column span alignment problem in table line {}", start + 2);
            malformed(detail, start + 1)
        };
        let mut main = 0;
        let mut spans = Vec::new();
        for &(left, right) in columns {
            if self.columns.get(main).map(|&(start, _)| start) != Some(left) {
                return Err(problem());
            }
            let mut more_columns = 0;
            while self.columns.get(main).ok_or_else(problem)?.1 != right {
                main += 1;
                more_columns += 1;
            }
            spans.push(more_columns);
            main += 1;
        }
        Ok(spans)
    }
}

/// The table of `block`, from the rows its parser found: each cell's lines
/// read as blocks, and a merged cell in the first of its places, warned of.
fn build(state: &mut State, block: &[Line], parsed: Parsed) -> Node {
    let mut rows = Vec::new();
    for row in parsed.rows {
        let mut cells = Vec::new();
        for cell in row {
            // Another cell's place, which it doesn't show in.
            let Some(cell) = cell else {
                cells.push(Vec::new());
                continue;
            };
            if cell.more_rows > 0 || cell.more_columns > 0 {
                let message = "wrote a merged cell in the first of its places, and left the rest \
                               empty, as Markdown's tables can't merge cells";
                state.warn(cell.line, message);
            }
            let text = cell.lines.iter().any(|line| !line.text.is_empty());
            cells.push(if text {
                nested(state, &cell.lines)
            } else {
                Vec::new()
            });
        }
        rows.push(cells);
    }
    table(state, block[0].num, rows, parsed.header_rows)
}

impl Body<'_, '_> {
    /// A grid table, from its top border, the line to read next: its
    /// lines, as docutils' `isolate_grid_table` finds them, up to a blank
    /// or indented line, or one that isn't the table's, then back to the
    /// last border.
    pub fn grid_table(&mut self) -> bool {
        let lines = self.lines;
        let start = self.at;
        let mut blank_finish = true;
        let mut end = start;
        while let Some(line) = lines.get(end)
            && !line.text.is_empty()
        {
            if line.text.starts_with(' ') {
                self.state.warn(line.num, "unexpected indentation");
                blank_finish = false;
                break;
            }
            end += 1;
        }
        let edge = lines[start..end]
            .iter()
            .position(|line| !line.text.starts_with(['+', '|']));
        if let Some(edge) = edge {
            end = start + edge;
            blank_finish = false;
        }
        if !grid_top(&lines[end - 1].text) {
            let border = (start + 2..end - 1)
                .rev()
                .find(|&at| grid_top(&lines[at].text));
            match border {
                Some(border) => {
                    end = border + 1;
                    blank_finish = false;
                }
                None => {
                    self.at = end;
                    let problem = malformed("bottom border missing or corrupt", end - 1 - start);
                    self.finish_table(start..end, Err(problem), blank_finish);
                    return true;
                }
            }
        }
        self.at = end;
        let width = Columns::new(&lines[start].text).len();
        let unaligned = lines[start..end].iter().position(|line| {
            Columns::new(&line.text).len() != width || !line.text.ends_with(['+', '|'])
        });
        let parsed = match unaligned {
            Some(offset) => Err(malformed("right border not aligned or missing", offset)),
            None => parse_grid(&lines[start..end]),
        };
        self.finish_table(start..end, parsed, blank_finish);
        true
    }

    /// A simple table, from its top border, the line to read next: its
    /// lines, as docutils' `isolate_simple_table` finds them, up to its
    /// second border of `=`s, or its first, if a blank line follows it.
    pub fn simple_table(&mut self) -> bool {
        let lines = self.lines;
        let start = self.at;
        let limit = lines.len() - 1;
        let top = lines[start].text.len();
        let blank_after = |at: usize| at == limit || lines[at + 1].text.is_empty();
        let mut found = 0;
        let mut found_at = start;
        for at in start + 1..=limit {
            let text = &lines[at].text;
            if !simple_border(text, b'=') {
                continue;
            }
            if text.len() != top {
                self.at = at + 1;
                let detail = "bottom border or header rule does not match top border";
                let problem = malformed(detail, at - start);
                self.finish_table(start..at + 1, Err(problem), blank_after(at));
                return true;
            }
            found += 1;
            found_at = at;
            if found == 2 || blank_after(at) {
                self.at = at + 1;
                let parsed = parse_simple(&lines[start..=at]);
                self.finish_table(start..at + 1, parsed, blank_after(at));
                return true;
            }
        }
        let (end, detail) = if found > 0 {
            let detail = "no bottom table border found or no blank line after table bottom";
            (found_at + 1, detail)
        } else {
            (lines.len(), "no bottom table border found")
        };
        self.at = end;
        self.finish_table(start..end, Err(malformed(detail, 0)), found == 0);
        true
    }

    /// Adds the table of the lines `block`, or, if it's malformed, its
    /// source, as code, warned of; and warns if no blank line follows it.
    fn finish_table(
        &mut self,
        block: Range<usize>,
        parsed: Result<Parsed, Malformed>,
        blank_finish: bool,
    ) {
        let lines = &self.lines[block];
        let node = match parsed {
            Ok(parsed) => build(self.state, lines, parsed),
            Err(problem) => {
                let message = format!("kept a malformed table as code: {}", problem.detail);
                self.state.warn(lines[0].num + problem.offset, message);
                Node::Problem(vec![code(join(lines))])
            }
        };
        self.nodes.push(node);
        if !blank_finish {
            let line = self.num(self.at);
            self.state.warn(line, "table ends without a blank line");
        }
    }
}
