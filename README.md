# little

A little static site generator for documentation: drop Markdown into
`docs/` and get a site. One self-contained binary, a three-level cascading
config, themes that own 100% of the markup, and plugins for the rest, like
Jupyter notebooks.

> **Status:** early development, versioned with
> [Semantic Versioning](https://semver.org/). Expect breaking changes until
> 1.0. See the [roadmap](./VISION.md#roadmap-thin-vertical-slices).

## Install

```sh
cargo install --locked little-docs
```

This needs [Rust](https://rustup.rs) 1.96 or later. The package is called
`little-docs`, but the command is `little`. To install little without Rust,
see [Install](./docs/getting-started/install.md#install-without-rust), in
the manual.

## Quick start

```sh
little init my-docs
cd my-docs
little serve
```

Open <http://127.0.0.1:8080>, edit `docs/README.md`, and the page reloads
itself.

## Documentation

little's manual is a little site, in [`docs/`](./docs/README.md), published
at <https://withlittle.github.io/little> from `main`. To read your changes
to it as a site, run `little serve` (or `cargo run -- serve`) in a clone of
this repository, and open <http://127.0.0.1:8080/little/>.

- [Getting started](./docs/getting-started/install.md): install little,
  start a site, configure it, and publish it.
- [Writing](./docs/writing/pages-and-files.md): pages, Markdown, links,
  code, math, diagrams, frontmatter, navigation, search, and versions.
- [Themes](./docs/themes/choosing-a-theme.md): choose a theme, or make one.
- [Plugins](./docs/plugins/using-plugins.md): use plugins, build pages
  from Jupyter notebooks, AsciiDoc, Org mode, or reStructuredText, add an
  API reference for Python, Rust, JavaScript, or Go, or write a plugin.
- [Reference](./docs/reference/commands.md): commands, settings, and the
  data a theme's templates get.

## Development

```sh
cargo test                     # run the tests
cargo clippy --all-targets     # lint (CI fails on warnings)
cargo fmt                      # format
cargo run -- build --strict    # build the manual, and check its links
```

The tests build a [plugin](./docs/plugins/writing-a-plugin.md),
[`tests/plugin/`](./tests/plugin/), with Rust's `wasm32-wasip2` target, which
[`rust-toolchain.toml`](./rust-toolchain.toml) has rustup install.
The [`pydoc`](./plugins/pydoc/), [`rustdoc`](./plugins/rustdoc/),
[`jsdoc`](./plugins/jsdoc/), [`godoc`](./plugins/godoc/),
[`jupyter`](./plugins/jupyter/), [`asciidoc`](./plugins/asciidoc/),
[`orgmode`](./plugins/orgmode/), and
[`restructuredtext`](./plugins/restructuredtext/) plugins are crates of
their own, with their own tests, which their READMEs say how to run, as
are the [library](./plugins/api_reference/) the first four share, and the
[one](./plugins/markup/) the last three share. The manual documents
little's own code with `rustdoc`, and has a page in the format each of the
last four reads, so build
[`rustdoc`](./plugins/rustdoc/README.md#building-it),
[`jupyter`](./plugins/jupyter/README.md#building-it),
[`asciidoc`](./plugins/asciidoc/README.md#building-it),
[`orgmode`](./plugins/orgmode/README.md#building-it), and
[`restructuredtext`](./plugins/restructuredtext/README.md#building-it)
before you build the manual.

See [`AGENTS.md`](./AGENTS.md) for architecture and conventions,
[`VISION.md`](./VISION.md) for where little is headed,
[`decisions/`](./decisions/) for design records, and
[`RELEASING.md`](./RELEASING.md) for how little and its plugins are
released.

## License

[MIT](./LICENSE)
