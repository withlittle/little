# api_reference

A library, not a plugin, that little's plugins that build an API reference
share: [`docstrings`](../docstrings/), from Python's docstrings, and
[`doc_comments`](../doc_comments/), from Rust's doc comments. Each reads its
language's code into what a page documents, in `src/model.rs`, and this
writes the page's Markdown, in `src/markdown.rs`, so their pages look alike.
Why they share it is in
[ADR 0031](../../decisions/0031-a-plugin-for-pythons-docstrings.md), and
how it grew for Rust in
[ADR 0044](../../decisions/0044-a-plugin-for-rusts-doc-comments.md).

## Developing it

Its tests run without WebAssembly:

```sh
cargo test --manifest-path plugins/api_reference/Cargo.toml
cargo clippy --manifest-path plugins/api_reference/Cargo.toml --all-targets
```

A change to it is a change to each plugin's pages, so run their tests too.
