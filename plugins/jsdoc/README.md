# jsdoc

A [plugin](../../docs/plugins/using-plugins.md) for little that adds an API
reference to a site: a page for a JavaScript or TypeScript package and each
of its modules, built from their doc comments, `/** ... */`, as
[JSDoc](https://jsdoc.app) and [TypeDoc](https://typedoc.org) build them.
It's named for JSDoc, but doesn't run it, nor TypeDoc, nor Node: it reads
the package's code with [oxc](https://oxc.rs)'s parser, without running or
type-checking it, so it needs neither npm nor the package's dependencies,
and a site builds the same each time, offline. Why it works as it does is in
[ADR 0048](../../docs/decisions/0048-a-plugin-for-javascripts-doc-comments.md),
and why it's named so, in
[ADR 0047](../../docs/decisions/0047-naming-api-reference-plugins.md).

## Building it

In a clone of little's repository:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/jsdoc/Cargo.toml
```

That builds
`plugins/jsdoc/target/wasm32-wasip2/release/little_jsdoc.wasm`,
about 1.2 MB, which a site can name where it is, or copy.

## Using it

[JavaScript API reference](../../docs/plugins/javascript-api-reference.md),
in little's manual, says how to name it in a site's `little.yml`, what pages
it adds, what they show, and its limits, for now.

## Developing it

Its tests run without WebAssembly, as only its glue to little's
[`wit/plugin.wit`](../../wit/plugin.wit), in `src/lib.rs`, needs it:

```sh
cargo test --manifest-path plugins/jsdoc/Cargo.toml
cargo clippy --manifest-path plugins/jsdoc/Cargo.toml --all-targets --target wasm32-wasip2
```

- `src/lib.rs` — its settings, the package's pages, the links between
  them, and its glue to little.
- `src/files.rs` — finds the package's files, the module each relative
  import leads to, and the page that shows each module.
- `src/reader.rs` — reads each module, with oxc's parser: what it declares,
  imports, and exports, as ES modules and CommonJS write them.
- `src/objects.rs` — reads what a page shows from each declaration: its
  kind, labels, signature, and members.
- `src/comment.rs` — reads doc comments, as JSDoc and TSDoc write them:
  their description, their block tags, like `@param`, and the sections of a
  page they make.
- `src/links.rs` — leads each `{@link}` to what it names.
- `src/print.rs` — writes types and values as a signature shows them, with
  oxc's codegen.
- `src/tests.rs` — tests of the pages of whole packages.

What a page documents, in no language in particular, and the Markdown it
writes from it, are in [`plugins/api_reference/`](../api_reference/), a
library it shares with [`pydoc`](../pydoc/) and [`rustdoc`](../rustdoc/),
with its own tests.
