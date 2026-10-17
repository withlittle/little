---
title: Writing a plugin
description: Write your first plugin, which opens links to other sites in a new tab, in Rust or Python, step by step.
---

# Writing a plugin

A [plugin](using-plugins.md) is a
[WebAssembly component](https://component-model.bytecodealliance.org/),
built for WASI 0.2, that exports the [hooks](../reference/plugin-hooks.md)
it needs, from little's
[`wit/plugin.wit`](https://github.com/withlittle/little/blob/main/wit/plugin.wit),
which little calls as the build reaches each step. This page writes one, in
Rust, that opens each link to another site in a new tab, with one hook,
`content`, which little calls for each page once its Markdown is written as
HTML. It turns

```html
<a href="https://google.com">Google</a>
```

into

```html
<a href="https://google.com" target="_blank" rel="noopener noreferrer">Google</a>
```

## Make a crate

You'll need [Rust](https://rustup.rs), with its `wasm32-wasip2` target,
which builds WebAssembly components:

```sh
rustup target add wasm32-wasip2
cargo new --lib links
```

Make the crate a `cdylib`, and add
[wit-bindgen](https://crates.io/crates/wit-bindgen), which writes the Rust
for the hooks, and [lol_html](https://crates.io/crates/lol_html), which
rewrites HTML:

```toml
# links/Cargo.toml
[package]
name = "links"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
lol_html = "3.0.1"
wit-bindgen = "0.62"
```

Then copy little's
[`wit/plugin.wit`](https://github.com/withlittle/little/blob/main/wit/plugin.wit)
into the crate, as `links/wit/plugin.wit`.

## Write the hook

```rust
// links/src/lib.rs
wit_bindgen::generate!({
    path: "wit/plugin.wit",
    inline: "
        package my:links;

        world links {
            export little:plugin/content@0.2.0;
        }
    ",
});

use exports::little::plugin::content::Guest;
use lol_html::{RewriteStrSettings, element, rewrite_str};

struct Links;

impl Guest for Links {
    fn content(_page: String, html: String) -> Result<String, String> {
        let settings = RewriteStrSettings::new().append_element_content_handler(element!(
            "a[href^='http']",
            |link| {
                link.set_attribute("target", "_blank")?;
                link.set_attribute("rel", "noopener noreferrer")?;
                Ok(())
            }
        ));
        rewrite_str(&html, settings).map_err(|error| error.to_string())
    }
}

export!(Links);
```

- The plugin's world, `links`, names the hooks it exports, from
  `little:plugin`, at version 0.2.0: here, only `content`. `generate!`
  writes a trait, `Guest`, for each, and `export!` exports the struct that
  implements them.
- [`content`](../reference/plugin-hooks.md#content) is given each page's
  file, like `docs/README.md`, and its HTML, and gives back the HTML the
  build goes on with: here, with a `target` and a `rel` on each link whose
  `href` starts with `http`. A link within the site, like `/guide`, is left
  as it is.
- An error, from HTML lol_html can't read, is warned of, naming the page,
  and the page's HTML is kept as it was.

## Build it

```sh
cd links
cargo build --release --target wasm32-wasip2
```

That builds the plugin, `target/wasm32-wasip2/release/links.wasm`, one file,
which runs wherever little does.

## Use it

Name it in a site's [`plugins`](using-plugins.md#naming-a-plugin), by its
path from the site's `little.yml`:

```yaml
# little.yml
plugins:
  links: ../links/target/wasm32-wasip2/release/links.wasm
```

Then build the site, or [serve](../reference/commands.md#little-serve) it,
which builds it again each time you build the plugin. A page with

```markdown
Search with [Google](https://google.com), or read the [guide](guide.md).
```

now has

```html
<p>Search with <a href="https://google.com" target="_blank" rel="noopener noreferrer">Google</a>, or read the <a href="/guide">guide</a>.</p>
```

## The theme's links, too

`content` changes only a page's own content, which the theme gets as
`page.content`, not the links the theme writes around it, like the one to
the site's repository in the built-in theme's header. For those too, export
the [`html`](../reference/plugin-hooks.md#html) hook instead, which is given
each page as it's written, theme and all: in `src/lib.rs`, export
`little:plugin/html@0.2.0`, `use exports::little::plugin::html::Guest`,
and name the function `html`. Its first argument is the page's URL, like
`/guide`, rather than its file.

## Settings, warnings, and files

- A plugin's settings go beside its `wasm` in `little.yml`, as
  [Using plugins](using-plugins.md#naming-a-plugin) shows, and its
  [`setup`](../reference/plugin-hooks.md#setup) hook is given them, as a
  JSON object, before any other hook.
- To warn of something, like a link it can't rewrite, a plugin imports
  `warnings`, and calls `warn`, which `little build` shows, naming the
  plugin, and `--strict` fails on. In Rust, a world that imports needs
  `generate_all`:

  ```rust
  wit_bindgen::generate!({
      path: "wit/plugin.wit",
      inline: "
          package my:links;

          world links {
              import little:plugin/warnings@0.2.0;
              export little:plugin/content@0.2.0;
          }
      ",
      generate_all,
  });

  use little::plugin::warnings::warn;
  ```

- A plugin can read the folders its `read` setting names, at their paths
  from the site's folder, like `std::fs::read_to_string("src/app.py")`, but
  can't change them, or reach anything else.

## In Python

[componentize-py](https://github.com/bytecodealliance/componentize-py)
builds the same plugin from Python. Copy `plugin.wit` into a folder, beside
`links.wit`, which names the plugin's world, and `app.py`, which exports its
hook:

```wit
// links.wit
package my:links;

world links {
    export little:plugin/content@0.2.0;
}
```

```python
# app.py
import re

from wit_world import exports

LINK = re.compile(r'<a ([^>]*href="https?://[^>]*)>')


class Content(exports.Content):
    def content(self, page: str, html: str) -> str:
        return LINK.sub(r'<a \1 target="_blank" rel="noopener noreferrer">', html)
```

Then build it:

```sh
pip install componentize-py
componentize-py -d plugin.wit -d links.wit -w my:links/links componentize app -o links.wasm
```

- A hook raises `componentize_py_types.Err` for an error, and a plugin that
  imports `warnings` calls `wit_world.imports.warnings.warn`.
- Its regular expression finds links as Markdown writes them, but not every
  `<a>`, as lol_html does.
- It carries Python with it, so it's about 18 MB, rather than Rust's 770 KB,
  and adds about a second to `little build`, rather than 60 ms.

## What's next

- [Plugin hooks](../reference/plugin-hooks.md) lists every hook, in the
  order little calls them, with what each is given, and gives back.
- little's own `pydoc`, `rustdoc`, and `jsdoc` plugins, in its
  [`plugins/`](https://github.com/withlittle/little/tree/main/plugins), add
  an API reference with the [`files`](../reference/plugin-hooks.md#files)
  hook.
- The version of little's hooks, `little:plugin@0.2.0`, is numbered as
  little's [versions](../getting-started/install.md#update-it) are. A hook
  little adds makes it 0.2.1, and a plugin built for 0.2.0 keeps working;
  a change that would break plugins makes it 0.3.0, and each must be built
  again, for 0.3.0, to load.
