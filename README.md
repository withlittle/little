# little

A little static site generator for documentation: drop Markdown, or Jupyter
notebooks, into `docs/` and get a site. One self-contained binary, a
three-level cascading config, and themes that own 100% of the markup.

> **Status:** early development (v0.1), versioned with
> [Semantic Versioning](https://semver.org/). Expect breaking changes. See
> the [roadmap](./VISION.md#roadmap-thin-vertical-slices).

## Install

Download little for your system from its
[latest release](https://github.com/withlittle/little/releases/latest), and
put it on your `PATH`, or install it with Cargo. It's `little-docs` on
crates.io, where `little` is another crate's name, but the command is still
`little`:

```sh
cargo binstall little-docs           # little's release, for your system
cargo install --locked little-docs   # or build it, with Rust 1.96 or later
```

[Install](./docs/getting-started/install.md), in the manual, says which
archive is for your system, how to check it, and how to build little from
source.

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
  code, math, diagrams, notebooks, frontmatter, navigation, search, and
  versions.
- [Themes](./docs/themes/choosing-a-theme.md): choose a theme, or make one.
- [Plugins](./docs/plugins/using-plugins.md): use plugins, add an API
  reference for Python or Rust, or write a plugin.
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
The [`pydoc`](./plugins/pydoc/), [`rustdoc`](./plugins/rustdoc/), and
[`jsdoc`](./plugins/jsdoc/) plugins are crates of their own, with their own
tests, which their READMEs say how to run, as is the
[library](./plugins/api_reference/) they share. The manual
documents little's own code with `rustdoc`, so
[build it](./plugins/rustdoc/README.md#building-it) before you build the
manual.

See [`AGENTS.md`](./AGENTS.md) for architecture and conventions,
[`VISION.md`](./VISION.md) for where little is headed,
[`docs/decisions/`](./docs/decisions/) for design records, and
[`RELEASING.md`](./RELEASING.md) for how little and its plugins are
released.

## License

[MIT](./LICENSE)
