# markup

A library, not a plugin, that little's plugins for other formats share:
[`asciidoc`](../asciidoc/), [`orgmode`](../orgmode/), and
[`restructuredtext`](../restructuredtext/). Each reads its format into a
document, in `src/model.rs`, and this writes the document as Markdown, with
only what the site's `markdown` settings turn on, in `src/settings.rs`, so
little builds each page as it builds one written in Markdown. Text is
escaped, so it shows as it's written; what Markdown can't hold is written
as HTML, where `raw_html` is on, or as near as Markdown comes; and each
heading's `#fragment` is worked out as little's, so links to it work.
Why they share it is in
[ADR 0062](../../docs/decisions/0062-plugins-for-asciidoc-and-orgmode.md).

## Developing it

Its tests run without WebAssembly, and render what it writes with comrak,
as little does:

```sh
cargo test --manifest-path plugins/markup/Cargo.toml
cargo clippy --manifest-path plugins/markup/Cargo.toml --all-targets
```

A change to it is a change to each plugin's pages, so run their tests too.
