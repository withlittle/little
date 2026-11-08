# little

The little static documentation site generator for big things. Put
Markdown in `docs/`, and get a site.

```sh
cargo install --locked little-docs
little init my-docs
cd my-docs
little serve
```

The [manual](https://withlittle.github.io/little) goes on from there. It
says [why little](./docs/README.md#why-little), and how to
[install it without Rust](./docs/getting-started/install.md#install-without-rust).

> **Status:** early development, versioned with
> [Semantic Versioning](https://semver.org/). Expect breaking changes until
> 1.0. See the [roadmap](./VISION.md#roadmap-thin-vertical-slices).

## Documentation

The manual is a little site, in [`docs/`](./docs/README.md), published from
`main`. To read your changes to it as a site, run `cargo run -- serve` in a
clone of this repository, and open <http://127.0.0.1:8080/little/>.

## Development

```sh
cargo test                     # run the tests
cargo clippy --all-targets     # lint (CI fails on warnings)
cargo fmt                      # format
cargo run -- build --strict    # build the manual, and check its links
```

The tests build a [plugin](./docs/plugins/writing-plugins.md),
[`tests/plugin/`](./tests/plugin/), with Rust's `wasm32-wasip2` target, which
[`rust-toolchain.toml`](./rust-toolchain.toml) has rustup install.
The [`pydoc`](./plugins/pydoc/), [`rustdoc`](./plugins/rustdoc/),
[`jsdoc`](./plugins/jsdoc/), [`godoc`](./plugins/godoc/),
[`openapi`](./plugins/openapi/), [`jupyter`](./plugins/jupyter/),
[`asciidoc`](./plugins/asciidoc/), [`orgmode`](./plugins/orgmode/), and
[`restructuredtext`](./plugins/restructuredtext/) plugins are crates of
their own, with their own tests, which their READMEs say how to run, as
are the [library](./plugins/api_reference/) the first five share, and the
[one](./plugins/markup/) the last three share. The manual has a page in the
format each of the last four reads, so build
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
