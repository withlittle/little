# pydoc

A [plugin](../../docs/plugins/using-plugins.md) for little that adds an API
reference to a site: a page for each module and package of a Python
package, built from its docstrings, in NumPy's, Google's, or Sphinx's
style, as [mkdocstrings](https://mkdocstrings.github.io/) builds them. It's
named for Python's own [pydoc](https://docs.python.org/3/library/pydoc.html),
but doesn't run it: it reads the package's code with
[Ruff](https://github.com/astral-sh/ruff)'s parser, without running it, so it
needs neither Python nor the package's dependencies, and a site builds the
same each time, offline. Why it works as it does is in
[ADR 0031](../../decisions/0031-a-plugin-for-pythons-docstrings.md), and why
it's named so, in
[ADR 0047](../../decisions/0047-naming-api-reference-plugins.md).

## Building it

In a clone of little's repository:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/pydoc/Cargo.toml
```

That builds
`plugins/pydoc/target/wasm32-wasip2/release/little_pydoc.wasm`,
about 1.6 MB, which a site can name where it is, or copy.

## Using it

[Python API reference](../../docs/plugins/python-api-reference.md), in
little's manual, says how to name it in a site's `little.yml`, what pages
it adds, what they show, and its limits, for now.

## Developing it

Its tests run without WebAssembly, as only its glue to little's
[`wit/plugin.wit`](../../wit/plugin.wit), in `src/lib.rs`, needs it:

```sh
cargo test --manifest-path plugins/pydoc/Cargo.toml
cargo clippy --manifest-path plugins/pydoc/Cargo.toml --all-targets --target wasm32-wasip2
```

- `src/lib.rs` — its settings, the packages and modules in the folders it
  reads, their pages, and its glue to little.
- `src/python.rs` — reads a module, with Ruff's parser, into what it
  documents.
- `src/docstring.rs` — reads a docstring's sections, in NumPy's, Google's,
  or Sphinx's style.

What a page documents, in no language in particular, and the Markdown it
writes from it, are in [`plugins/api_reference/`](../api_reference/), a
library it shares, with its own tests.
