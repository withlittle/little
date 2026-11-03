# api_reference

A library, not a plugin, that little's plugins that build an API reference
share: [`pydoc`](../pydoc/), from Python's docstrings,
[`rustdoc`](../rustdoc/), from Rust's doc comments,
[`jsdoc`](../jsdoc/), from JavaScript's and TypeScript's, and
[`godoc`](../godoc/), from Go's. Each reads its
language's code into what a page documents, in `src/model.rs`, and this
writes the page's Markdown, in `src/markdown.rs`, so their pages look alike.
It also reads the settings they have alike, like `title` and `url`, in
`src/settings.rs`, and holds the pages each adds, with their paths and the
links between them, in `src/pages.rs`.
Why they share it is in
[ADR 0031](../../decisions/0031-a-plugin-for-pythons-docstrings.md), and
how it grew for Rust in
[ADR 0044](../../decisions/0044-a-plugin-for-rusts-doc-comments.md), and
for JavaScript in
[ADR 0048](../../decisions/0048-a-plugin-for-javascripts-doc-comments.md),
and for Go in
[ADR 0064](../../decisions/0064-a-plugin-for-gos-doc-comments.md).

## Developing it

Its tests run without WebAssembly:

```sh
cargo test --manifest-path plugins/api_reference/Cargo.toml
cargo clippy --manifest-path plugins/api_reference/Cargo.toml --all-targets
```

A change to it is a change to each plugin's pages, so run their tests too.
