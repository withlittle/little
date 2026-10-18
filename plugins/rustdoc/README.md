# rustdoc

A [plugin](../../docs/plugins/using-plugins.md) for little that adds an API
reference to a site: a page for a Rust crate and each of its modules, built
from their doc comments, as Rust's own
[rustdoc](https://doc.rust-lang.org/rustdoc/) builds them. It's named for
rustdoc, but doesn't run it: it reads the crate's code with
[syn](https://github.com/dtolnay/syn)'s parser, without compiling it, so it
needs neither Cargo nor the crate's dependencies, and a site builds the same
each time, offline. little's manual documents little's own code with it. Why
it works as it does is in
[ADR 0044](../../docs/decisions/0044-a-plugin-for-rusts-doc-comments.md), and
why it's named so, in
[ADR 0047](../../docs/decisions/0047-naming-api-reference-plugins.md).

## Building it

In a clone of little's repository:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/rustdoc/Cargo.toml
```

That builds
`plugins/rustdoc/target/wasm32-wasip2/release/little_rustdoc.wasm`,
about 1.2 MB, which a site can name where it is, or copy. little's manual
names it where it is, so build it before `cargo run -- build` or `serve`.

## Using it

[Rust API reference](../../docs/plugins/rust-api-reference.md), in little's
manual, says how to name it in a site's `little.yml`, what pages it adds,
what they show, and its limits, for now.

## Developing it

Its tests run without WebAssembly, as only its glue to little's
[`wit/plugin.wit`](../../wit/plugin.wit), in `src/lib.rs`, needs it:

```sh
cargo test --manifest-path plugins/rustdoc/Cargo.toml
cargo clippy --manifest-path plugins/rustdoc/Cargo.toml --all-targets --target wasm32-wasip2
```

- `src/lib.rs` — its settings, the crate's pages, the links between them,
  and its glue to little. Its tests include one that documents little's own
  `src/`.
- `src/modules.rs` — reads the crate's modules, from its files, as rustc
  finds them, and what the names in each lead to.
- `src/rust.rs` — reads each module's items, with syn, into what it
  documents, and writes their signatures, with prettyplease.
- `src/docs.rs` — reads doc comments' Markdown, as rustdoc does: their code
  blocks, their headings, and their links to paths.

What a page documents, in no language in particular, and the Markdown it
writes from it, are in [`plugins/api_reference/`](../api_reference/), a
library it shares with [`pydoc`](../pydoc/) and [`jsdoc`](../jsdoc/), with
its own tests.
