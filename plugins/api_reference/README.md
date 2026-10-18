# api_reference

A library, not a plugin, that little's plugins that build an API reference
share: [`pydoc`](../pydoc/), from Python's docstrings,
[`rustdoc`](../rustdoc/), from Rust's doc comments, and
[`jsdoc`](../jsdoc/), from JavaScript's and TypeScript's. Each reads its
language's code into what a page documents, in `src/model.rs`, and this
writes the page's Markdown, in `src/markdown.rs`, so their pages look alike.
Why they share it is in
[ADR 0031](../../docs/decisions/0031-a-plugin-for-pythons-docstrings.md), and
how it grew for Rust in
[ADR 0044](../../docs/decisions/0044-a-plugin-for-rusts-doc-comments.md), and
for JavaScript in
[ADR 0048](../../docs/decisions/0048-a-plugin-for-javascripts-doc-comments.md).

## Developing it

Its tests run without WebAssembly:

```sh
cargo test --manifest-path plugins/api_reference/Cargo.toml
cargo clippy --manifest-path plugins/api_reference/Cargo.toml --all-targets
```

A change to it is a change to each plugin's pages, so run their tests too.
