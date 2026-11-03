# restructuredtext

A [plugin](../../docs/plugins/using-plugins.md) for little that builds a
page from each [reStructuredText](https://docutils.sourceforge.io/rst.html)
file, an `.rst` file, in a site's source, without docutils, Sphinx, or
Python. It reads each with little's
[`formats` hook](../../docs/reference/plugin-hooks.md#formats), parses it
with a parser of its own, which follows docutils' specification, without
Sphinx's additions, as no parser in Rust reads enough of it, and gives
little back the page's parts, which [`plugins/markup/`](../markup/), the
library little's plugins for other formats share, writes as Markdown, with
the extensions to it the site turns on, so little writes them as it writes
Markdown. Why a plugin reads it, not little, is in
[ADR 0060](../../decisions/0060-other-formats-in-plugins.md), why its
parser is its own in
[ADR 0063](../../decisions/0063-a-plugin-for-restructuredtext.md), and
how the library writes its page in
[ADR 0062](../../decisions/0062-plugins-for-asciidoc-and-orgmode.md).

## Building it

In a clone of little's repository:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/restructuredtext/Cargo.toml
```

That builds
`plugins/restructuredtext/target/wasm32-wasip2/release/little_restructuredtext.wasm`,
about 570 KB, which a site can name where it is, or copy.

## Using it

[reStructuredText](../../docs/plugins/restructuredtext.rst), in little's
manual, says how a site names it in its `plugins`, what a
reStructuredText file's page shows, and its limits.

## Developing it

Its tests run without WebAssembly, as only its glue to little's
[`wit/plugin.wit`](../../wit/plugin.wit), at the end of `src/lib.rs`, needs
it:

```sh
cargo test --manifest-path plugins/restructuredtext/Cargo.toml
cargo clippy --manifest-path plugins/restructuredtext/Cargo.toml --all-targets --target wasm32-wasip2
```

`RST_SAMPLES=<folder> cargo test --manifest-path
plugins/restructuredtext/Cargo.toml -- --ignored --nocapture` reads each
`.rst` file in a folder, whole and cut off at every 97th character,
checking that none panics, and prints how long each takes to read.

The parser follows docutils' own, in Python, state by state, and names
what it follows, so each part can be checked against it:

- `src/lib.rs` — reads a reStructuredText file, and its frontmatter, into
  the parts of its page, with what to warn of; and its glue to little.
- `src/lines.rs` — splits a file into lines, with tabs expanded, and finds
  the blocks of indented lines in them.
- `src/body.rs` — the blocks of a run of lines: paragraphs, sections and
  their titles, literal, line, and doctest blocks, quotes, and
  definition lists.
- `src/lists.rs` — bullet, enumerated, field, and option lists.
- `src/explicit.rs` — explicit markup, `..`: footnotes, citations,
  targets, substitution definitions, comments, and directives' arguments,
  options, and content.
- `src/directives.rs` — the directives the plugin reads, like `note`,
  `image`, `code`, and `csv-table`, and the warnings for those it doesn't.
- `src/tables.rs` — grid and simple tables, and the tables of the
  `table`, `csv-table`, and `list-table` directives.
- `src/inline.rs` — inline markup, roles, and references, in the text of
  each block.
- `src/uri.rs` — standalone URLs and email addresses in text.
- `src/text.rs` — the characters inline markup may follow, names and the
  ids made from them, escapes, and Roman numerals.
- `src/data.rs` — tables of characters, URI schemes, and ids, generated
  from docutils 0.23.
- `src/tree.rs` — the tree the parser reads a file into, before its
  references are led to their targets.
- `src/state.rs` — what the parser knows of the document as it reads it:
  its targets and their ids, footnotes, substitutions, roles, and warnings.
- `src/resolve.rs` — leads each reference to its target, footnote, or
  substitution, and writes the tree as the library's blocks and inlines.
- `src/convert.rs` — the document's title, subtitle, description, and
  docinfo, as docutils' transforms find them.
- `src/tests.rs` — its tests, of what each construct is written as,
  rendered as little renders it, and what's warned of, on which line; and
  that the manual's page on it is read with no warning.
