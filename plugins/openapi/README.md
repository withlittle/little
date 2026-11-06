# openapi

A [plugin](../../docs/plugins/using-plugins.md) for little that adds the
reference of an HTTP API to a site: a page for each OpenAPI 3 document, in
JSON or YAML, in the folders it reads, with how a client authenticates,
each operation, under its tags, and each schema, as Redoc and Swagger UI
show them, but in the site's theme, with no script. It reads JSON with
serde_json, and YAML with
[yaml-rust2](https://github.com/Ethiraric/yaml-rust2), as YAML 1.2, and
follows each `$ref` to another file, so it needs no Node, and a site
builds the same each time, offline. Why it works as it does is in
[ADR 0069](../../decisions/0069-a-plugin-for-openapi.md).

## Building it

In a clone of little's repository:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/openapi/Cargo.toml
```

That builds
`plugins/openapi/target/wasm32-wasip2/release/little_openapi.wasm`,
about 700 KB, which a site can name where it is, or copy.

## Using it

[HTTP API reference](../../docs/plugins/http-api-reference.md), in little's
manual, says how to name it in a site's `little.yml`, what pages it adds,
what they show, and its limits, for now.

## Developing it

Its tests run without WebAssembly, as only its glue to little's
[`wit/plugin.wit`](../../wit/plugin.wit), in `src/lib.rs`, needs it:

```sh
cargo test --manifest-path plugins/openapi/Cargo.toml
cargo clippy --manifest-path plugins/openapi/Cargo.toml --all-targets --target wasm32-wasip2
```

- `src/lib.rs` — its settings, a page for each document, and its glue to
  little.
- `src/files.rs` — finds the OpenAPI 3 documents in the folders it reads,
  and parses a file of JSON or YAML.
- `src/yaml.rs` — reads YAML as YAML 1.2, with yaml-rust2, into JSON's
  values, so a document reads the same in either.
- `src/refs.rs` — the files a document's `$ref`s lead to, and follows
  each, within a file and from one to another, by its JSON pointer.
- `src/page.rs` — writes a document's page: what its `info` says, its
  servers and security, its security schemes, each tag's operations, its
  webhooks, and its schemas.
- `src/schema.rs` — writes a schema as its type, in a few words, the notes
  on a value of it, like its default and limits, and the table of an
  object's fields.
- `src/markdown.rs` — escapes text, moves the headings in descriptions
  down, and works out the id comrak gives each heading, for the links to
  it.
- `src/tests.rs` — tests of the pages of whole documents.

It shares its glue to little, its settings, and the helpers for pages and
their tables with [`pydoc`](../pydoc/), [`rustdoc`](../rustdoc/),
[`jsdoc`](../jsdoc/), and [`godoc`](../godoc/), in
[`plugins/api_reference/`](../api_reference/), a library with its own
tests, but not its model of a page, which is of code.
