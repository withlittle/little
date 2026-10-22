# jupyter

A [plugin](../../docs/plugins/using-plugins.md) for little that builds a
page from each [Jupyter](https://jupyter.org) notebook, an `.ipynb` file, in
a site's source, as it was saved, outputs and all, without Jupyter or
Python. Its Markdown cells are Markdown, its code cells are code blocks, in
the notebook's language, and their outputs are text, tables, and images.
It reads each notebook with little's
[`formats` hook](../../docs/reference/plugin-hooks.md#formats), and gives
little back the page's parts, which little writes as it writes Markdown.
Why notebooks' pages are as they are is in
[ADR 0024](../../docs/decisions/0024-jupyter-notebooks.md).

## Building it

In a clone of little's repository:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/jupyter/Cargo.toml
```

That builds
`plugins/jupyter/target/wasm32-wasip2/release/little_jupyter.wasm`,
about 460 KB, which a site can name where it is, or copy.

## Using it

[Notebooks](../../docs/writing/notebooks.ipynb), in little's manual, says
what a notebook's page shows, and its limits.

## Developing it

Its tests run without WebAssembly, as only its glue to little's
[`wit/plugin.wit`](../../wit/plugin.wit), at the end of `src/lib.rs`, needs
it:

```sh
cargo test --manifest-path plugins/jupyter/Cargo.toml
cargo clippy --manifest-path plugins/jupyter/Cargo.toml --all-targets --target wasm32-wasip2
```

- `src/lib.rs` — reads a notebook, in version 4 of Jupyter's format, with
  serde, into the parts of its page: its frontmatter, its cells, and their
  outputs, as a terminal or Jupyter shows them; and its glue to little.
- `src/tests.rs` — its tests, of the parts each kind of cell and output
  becomes.
