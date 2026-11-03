# godoc

A [plugin](../../docs/plugins/using-plugins.md) for little that adds an API
reference to a site: a page for each package of a Go module, built from
their doc comments, as `go doc` and [pkg.go.dev](https://pkg.go.dev) show
them. It's named for godoc, Go's documentation server, but doesn't run it,
nor `go doc`, nor Go: it reads the module's code with
[gosyn](https://github.com/chikaku/gosyn)'s parser, without compiling it,
so it needs neither Go nor the module's dependencies, and a site builds the
same each time, offline. Why it works as it does is in
[ADR 0064](../../decisions/0064-a-plugin-for-gos-doc-comments.md),
and why it's named so, in
[ADR 0047](../../decisions/0047-naming-api-reference-plugins.md).

## Building it

In a clone of little's repository:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/godoc/Cargo.toml
```

That builds
`plugins/godoc/target/wasm32-wasip2/release/little_godoc.wasm`,
about 520 KB, which a site can name where it is, or copy.

## Using it

[Go API reference](../../docs/plugins/go-api-reference.md), in little's
manual, says how to name it in a site's `little.yml`, what pages it adds,
what they show, and its limits, for now.

## Developing it

Its tests run without WebAssembly, as only its glue to little's
[`wit/plugin.wit`](../../wit/plugin.wit), in `src/lib.rs`, needs it:

```sh
cargo test --manifest-path plugins/godoc/Cargo.toml
cargo clippy --manifest-path plugins/godoc/Cargo.toml --all-targets --target wasm32-wasip2
```

- `src/lib.rs` — its settings, the module's path, from its `go.mod`, the
  module's pages, the links between them, and its glue to little.
- `src/files.rs` — finds the module's folders that hold its packages, as
  the go command finds them, and the page that shows each.
- `src/package.rs` — puts each package together from what its files
  declare, as go/doc does, and reads what its page shows.
- `src/reader.rs` — reads what each file declares, with gosyn's parser: its
  package, what it imports, and what it exports, each with its doc comment
  and its code.
- `src/source.rs` — reads a file's tokens as Go's parser does: the
  semicolons its scanner inserts at the ends of lines, and the doc comment
  and line comment of each declaration and field.
- `src/comment.rs` — reads doc comments, as go/doc/comment does, into
  paragraphs, headings, code, and lists, and writes them as Markdown, with
  each doc link led to what it names.
- `src/tests.rs` — tests of the pages of whole modules.

What a page documents, in no language in particular, and the Markdown it
writes from it, are in [`plugins/api_reference/`](../api_reference/), a
library it shares with [`pydoc`](../pydoc/), [`rustdoc`](../rustdoc/), and
[`jsdoc`](../jsdoc/), with its own tests.
