# sitemap

A [plugin](../../docs/plugins/using-plugins.md) for little that writes a
site's sitemap, `sitemap.xml`, for search engines: the address of each of
its pages, and when it last changed. It reads where the site is published
with little's [`site` hook](../../docs/reference/plugin-hooks.md#site),
notes each page with its
[`page` hook](../../docs/reference/plugin-hooks.md#page), and writes the
file with its
[`destination` hook](../../docs/reference/plugin-hooks.md#destination).
Why it's a plugin, and why its sitemap is as it is, is in
[ADR 0090](../../decisions/0090-a-plugin-for-sitemaps.md).

## Building it

In a clone of little's repository:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/sitemap/Cargo.toml
```

That builds
`plugins/sitemap/target/wasm32-wasip2/release/little_sitemap.wasm`,
about 420 KB, which a site can name where it is, or copy.

## Using it

[Sitemap](../../docs/plugins/sitemap.md), in little's manual, says how a
site names it in its `plugins`, and what its sitemap lists.

## Developing it

Its tests run without WebAssembly, as only its glue to little's
[`wit/plugin.wit`](../../wit/plugin.wit), at the end of `src/lib.rs`, needs
it:

```sh
cargo test --manifest-path plugins/sitemap/Cargo.toml
cargo clippy --manifest-path plugins/sitemap/Cargo.toml --all-targets --target wasm32-wasip2
```

- `src/lib.rs` — the sitemap a build gathers, from the site's `url`, and
  each page's URL and date, and the XML it's written as; and its glue to
  little.
- `src/tests.rs` — its tests, of which pages a sitemap lists, and how each
  is written.
