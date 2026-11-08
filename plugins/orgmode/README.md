# orgmode

A [plugin](../../docs/plugins/using-plugins.md) for little that builds a
page from each [Org mode](https://orgmode.org) file, an `.org` file, in a
site's source, without Emacs. It reads each with little's
[`formats` hook](../../docs/reference/plugin-hooks.md#formats), parses it
with [orgize](https://github.com/PoiScript/orgize), and gives little back
the page's parts, as Org's own HTML export shows it, which
[`plugins/markup/`](../markup/), the library little's plugins for other
formats share, writes as Markdown, with the extensions to it the site turns
on, so little writes them as it writes Markdown. Why a plugin reads it, not
little, is in
[ADR 0060](../../decisions/0060-other-formats-in-plugins.md), and how
its page is written in
[ADR 0062](../../decisions/0062-plugins-for-asciidoc-and-orgmode.md).

## Building it

In a clone of little's repository:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/orgmode/Cargo.toml
```

That builds
`plugins/orgmode/target/wasm32-wasip2/release/little_orgmode.wasm`,
about 0.6 MB, which a site can name where it is, or copy.

## Using it

[Org mode](../../docs/plugins/org-mode.org), in little's manual, says how a
site names it in its `plugins`, and what an Org file's page shows.

## Developing it

Its tests run without WebAssembly, as only its glue to little's
[`wit/plugin.wit`](../../wit/plugin.wit), at the end of `src/lib.rs`, needs
it:

```sh
cargo test --manifest-path plugins/orgmode/Cargo.toml
cargo clippy --manifest-path plugins/orgmode/Cargo.toml --all-targets --target wasm32-wasip2
```

- `src/lib.rs` — reads an Org file, and its frontmatter, into the parts of
  its page, with what to warn of; and its glue to little.
- `src/keywords.rs` — the keywords a file sets for itself, wherever they
  are in it: its title, description, and macros, the `#+OPTIONS:` and
  TODO keywords it's read with, and the header arguments its blocks of
  code inherit.
- `src/convert.rs` — Org's headlines and elements as the library's blocks,
  leaving out what the plugin leaves out, with their captions and names;
  the definitions of footnotes; and links to headlines.
- `src/inline.rs` — Org's objects, its text, markup, links, footnotes, and
  macros, as the library's inlines, with what orgize reads in a way Org
  doesn't, like `$…$` math and plain links, read as Org reads it.
- `src/links.rs` — what Org's links lead to, and the plain and angle links
  in text.
- `src/table.rs` — Org's tables as Markdown's, without their special rows
  and columns, with a header row, and each column aligned as Org's export
  aligns it.
- `src/tests.rs` — its tests, of what each construct is written as,
  rendered as little renders it, and what's warned of, on which line; and
  that the manual's page on it is read with no warning.
