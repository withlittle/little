//! Tables: AsciiDoc's, as Markdown's, which have a row of headers, and no
//! merged cells.

use asciidoc_parser::HasSpan;
use asciidoc_parser::blocks::{
    ColumnStyle, HorizontalAlignment, TableBlock, TableCell, TableCellContent, TableRow,
};
use little_markup::{Align, Block, Cell, Inline, Table};

use crate::inlines;
use crate::reader::Reader;

impl Reader<'_> {
    /// A table: its header row, if it has one, then its rows, and its
    /// footer row, as the last. A cell that spans columns or rows is
    /// written in the first of them, and the rest are left empty.
    pub(crate) fn table<'b>(&mut self, table: &'b TableBlock<'b>, out: &mut Vec<Block>) {
        // The default, `<`, can't be told from no alignment, and is the
        // theme's to choose.
        let align = table
            .columns()
            .iter()
            .map(|column| match column.h_align() {
                HorizontalAlignment::Left => Align::None,
                HorizontalAlignment::Center => Align::Center,
                HorizontalAlignment::Right => Align::Right,
            })
            .collect();
        // For each column, the rows to come that a cell above takes.
        let mut taken = vec![0; table.columns().len()];
        let header = table.header_row().map(|row| self.row(row, &mut taken));
        let mut rows = Vec::new();
        for row in table.body_rows().iter().chain(table.footer_row()) {
            rows.push(self.row(row, &mut taken));
        }
        out.push(Block::Table(Table {
            line: Some(self.line(table.span())),
            align,
            header,
            rows,
        }));
    }

    /// A row's cells, with an empty cell in each place a cell of another
    /// takes, and `taken`, for the rows to come, updated.
    fn row<'b>(&mut self, row: &'b TableRow<'b>, taken: &mut Vec<usize>) -> Vec<Cell> {
        let above: Vec<bool> = taken.iter().map(|rows| *rows > 0).collect();
        for rows in taken.iter_mut() {
            *rows = rows.saturating_sub(1);
        }
        let mut cells = Vec::new();
        let mut column = 0;
        let fill = |column: &mut usize, cells: &mut Vec<Cell>| {
            while above.get(*column) == Some(&true) {
                cells.push(Vec::new());
                *column += 1;
            }
        };
        for cell in row.cells() {
            fill(&mut column, &mut cells);
            let columns = cell.colspan().max(1);
            let rows = cell.rowspan().max(1);
            if columns > 1 || rows > 1 {
                let line = self.line(cell.span());
                self.warn(
                    line,
                    "wrote a merged cell in the first of its places, and left the rest empty, as \
                     Markdown's tables can't merge cells",
                );
            }
            cells.push(self.cell(cell));
            cells.extend(std::iter::repeat_with(Vec::new).take(columns - 1));
            if taken.len() < column + columns {
                taken.resize(column + columns, 0);
            }
            for each in &mut taken[column..column + columns] {
                *each = (*each).max(rows - 1);
            }
            column += columns;
        }
        fill(&mut column, &mut cells);
        cells
    }

    /// A cell's blocks: AsciiDoc's, in an `a` cell, else its text, in its
    /// style.
    fn cell<'b>(&mut self, cell: &'b TableCell<'b>) -> Cell {
        let content = match cell.content() {
            TableCellContent::AsciiDoc(document) => {
                let mut blocks = Vec::new();
                self.children(document.blocks().iter(), &mut blocks);
                return blocks;
            }
            TableCellContent::Simple(content) => content,
        };
        if cell.style() == ColumnStyle::Literal {
            return vec![Block::Code {
                language: String::new(),
                text: inlines::verbatim(content.inlines()),
            }];
        }
        let inlines = self.inlines(content.inlines());
        if inlines.is_empty() {
            return Vec::new();
        }
        vec![Block::Paragraph(match cell.style() {
            ColumnStyle::Emphasis => vec![Inline::Emphasis(inlines)],
            ColumnStyle::Strong | ColumnStyle::Header => vec![Inline::Strong(inlines)],
            ColumnStyle::Monospace => vec![Inline::Code(inlines::flatten(&inlines))],
            _ => inlines,
        })]
    }
}
