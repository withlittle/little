# asciidoc

A [plugin](../../docs/plugins/using-plugins.md) for little that builds a
page from each [AsciiDoc](https://asciidoc.org) file, an `.adoc` or
`.asciidoc` file, in a site's source, without Asciidoctor or Ruby. It reads
each with little's
[`formats` hook](../../docs/reference/plugin-hooks.md#formats), parses it
with [asciidoc-parser](https://github.com/asciidoc-rs/asciidoc-parser), and
gives little back the page's parts, which
[`plugins/markup/`](../markup/), the library little's plugins for other
formats share, writes as Markdown, with the extensions to it the site turns
on, so little writes them as it writes Markdown. Why a plugin reads it, not
little, is in
[ADR 0060](../../docs/decisions/0060-other-formats-in-plugins.md), and how
its page is written in
[ADR 0062](../../docs/decisions/0062-plugins-for-asciidoc-and-orgmode.md).

## Building it

In a clone of little's repository:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/asciidoc/Cargo.toml
```

That builds
`plugins/asciidoc/target/wasm32-wasip2/release/little_asciidoc.wasm`,
about 2.1 MB, which a site can name where it is, or copy.

## Using it

[AsciiDoc](../../docs/plugins/asciidoc.adoc), in little's manual, says how
a site names it in its `plugins`, what an AsciiDoc file's page shows, and
its limits.

## Developing it

Its tests run without WebAssembly, as only its glue to little's
[`wit/plugin.wit`](../../wit/plugin.wit), at the end of `src/lib.rs`, needs
it:

```sh
cargo test --manifest-path plugins/asciidoc/Cargo.toml
cargo clippy --manifest-path plugins/asciidoc/Cargo.toml --all-targets --target wasm32-wasip2
```

- `src/lib.rs` — reads an AsciiDoc file, and its frontmatter, into the
  parts of its page, with what to warn of; and its glue to little.
- `src/reader.rs` — parses the file, and reads its header, the
  attributes it follows, like `:imagesdir:`, and what asciidoc-parser
  warns of; finds each include, which it leaves out; and gathers the
  page's footnotes, and what to warn of, on which line.
- `src/blocks.rs` — AsciiDoc's sections, paragraphs, lists, and delimited
  blocks, as the library's blocks, with their titles and ids.
- `src/inlines.rs` — AsciiDoc's text and its markup, links,
  cross-references, footnotes, and macros, as the library's inlines.
- `src/tables.rs` — AsciiDoc's tables as Markdown's, with a header row,
  and merged cells in their first places.
- `src/text.rs` — HTML's escapes, which asciidoc-parser writes some text
  in; images' paths and default alt text; links to other pages; and the
  macros the plugin doesn't read, which it keeps as text.
- `src/tests.rs` — its tests, of what each construct is written as,
  rendered as little renders it, and what's warned of, on which line; and
  that the manual's page on it is read with no warning.
