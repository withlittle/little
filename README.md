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
`main`. To read your changes to it as a site, in a clone of this repository,
build the plugins it uses, as [`AGENTS.md`](./AGENTS.md#commands) says, then
run `cargo run -- serve` and open <http://127.0.0.1:8080/little/>.

## Development

[`AGENTS.md`](./AGENTS.md) says how to work on little: the commands that
build, test, and lint it and its plugins, where things are, and the
conventions to follow. See [`VISION.md`](./VISION.md) for where little is
headed, [`decisions/`](./decisions/) for design records, and
[`RELEASING.md`](./RELEASING.md) for how little and its plugins are
released.

## License

[MIT](./LICENSE)
