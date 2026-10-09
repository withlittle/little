---
title: Writing a plugin
description: Write a plugin, in Rust or Python, that renders code blocks or adds pages, as a WebAssembly component.
---

# Writing a plugin

A [plugin](using-plugins.md) is a
[WebAssembly component](https://component-model.bytecodealliance.org/),
built for WASI 0.2, that exports the interface in little's
[`wit/plugin.wit`](https://github.com/withlittle/little/blob/main/wit/plugin.wit).
Rust's `wasm32-wasip2` target builds one, and so can Python, Go, JavaScript,
and C.

## In Rust

This plugin renders a ```` ```csv ```` block as a table, in Markdown:

```toml
# Cargo.toml
[package]
name = "little-csv"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = "0.62"
```

```rust
// src/lib.rs
wit_bindgen::generate!({ path: "wit/plugin.wit", world: "plugin" });

use exports::little::plugin::code_blocks::{Block, Guest, Output};

struct Csv;

impl Guest for Csv {
    fn setup(_settings: String) -> Result<Vec<String>, String> {
        Ok(vec!["csv".to_string()])
    }

    fn render(block: Block) -> Result<Output, String> {
        let mut rows = block.text.lines().map(|line| line.split(',').collect::<Vec<_>>());
        let header = rows.next().ok_or("the table is empty")?;
        let mut table = format!("| {} |\n|{}\n", header.join(" | "), " --- |".repeat(header.len()));
        for row in rows {
            table += &format!("| {} |\n", row.join(" | "));
        }
        Ok(Output::Markdown(table))
    }
}

export!(Csv);
```

Copy little's `wit/plugin.wit` into the crate's `wit/` folder, and build it:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2
```

Then name `target/wasm32-wasip2/release/little_csv.wasm` in a site's
[`plugins`](using-plugins.md#naming-a-plugin).

## What little calls

- `setup` is called as each build starts, before any page is built, with the
  plugin's settings from `little.yml` as a JSON object, without `wasm`, like
  `{"read":["src"],"style":"google"}`, where `read` is always a list. It
  gives back the languages the plugin renders, each one word, which little
  matches in any case, or an error, which stops the build.
- `render` is given each block in those languages, with:
  - `language`, its language, in lowercase, like `csv`;
  - `info`, all that's after its fence, like `csv headers=false`;
  - `text`, its text, which ends in a newline unless the block is empty;
  - `page`, its page's file, from the site's folder, like `docs/sizes.md`.

  It gives back `Output::Markdown` or `Output::Html`, or an error, which
  little warns of, leaving the block as it was.
- little starts each plugin once a build, so a plugin keeps its state from
  block to block, and page to page. If it crashes, as a Rust plugin does when
  it panics, little starts it again, and calls `setup` again, before the next
  block.
- A plugin reads the folders `read` names at their paths from the site's
  folder, like `std::fs::read_to_string("src/app.py")`. It can't change
  them, or reach anything else.
- A plugin keeps working as little changes, until the interface's version,
  `little:plugin@0.1.0`, does.

## In Python

[componentize-py](https://github.com/bytecodealliance/componentize-py)
builds the same plugin from `app.py`, with
`componentize-py -d plugin.wit -w plugin componentize app -o csv.wasm`:

```python
import csv
from componentize_py_types import Err
from wit_world import exports
from wit_world.exports.code_blocks import Block, Output, Output_Markdown

class CodeBlocks(exports.CodeBlocks):
    def setup(self, settings: str) -> list[str]:
        return ["csv"]

    def render(self, block: Block) -> Output:
        rows = list(csv.reader(block.text.splitlines()))
        if not rows:
            raise Err("the table is empty")
        table = ["| " + " | ".join(rows[0]) + " |", "|" + " --- |" * len(rows[0])]
        table += ["| " + " | ".join(row) + " |" for row in rows[1:]]
        return Output_Markdown("\n".join(table))
```

It carries Python with it, so it's about 18 MB, rather than Rust's 77 KB, and
adds about a second to `little build`, rather than 10 ms.

## Adding pages

A plugin can [add pages](using-plugins.md#pages), too, as
[`docstrings`](python-api-reference.md) does, when it's built for
`world plugin-with-pages`, which exports `pages` beside `setup` and
`render`. For the plugin above to add a page that shows the site's
`data/sizes.csv`, in a ```` ```csv ```` block its own `render` renders,
change the world it's built for, and add `pages`:

````rust
// src/lib.rs
wit_bindgen::generate!({ path: "wit/plugin.wit", world: "plugin-with-pages" });

use exports::little::plugin::pages::{self, Added, Page};

impl pages::Guest for Csv {
    fn pages() -> Result<Added, String> {
        let text = std::fs::read_to_string("data/sizes.csv").map_err(|error| error.to_string())?;
        let markdown = format!("# Sizes\n\n```csv\n{}\n```\n", text.trim_end());
        let page = Page { path: "data/sizes.md".to_string(), markdown };
        Ok(Added { pages: vec![page], warnings: Vec::new() })
    }
}
````

Then name the folder it reads, `read: data`, beside its `wasm` in the site's
`little.yml`, and the site has a page at `/data/sizes`, with the table on
it.

- `pages` is called once each build, after `setup`, before any page is
  built. It gives back the pages the plugin adds, and what little should
  warn of, each one line, like a file it can't read, which
  `little build --strict` fails on; or an error, which stops the build, as a
  crash does.
- A page's `path` is where its file would be, from the source's folder, with
  `/` between its parts, like `reference/README.md`: no part is empty, `.`,
  or `..`, and it ends in `.md`, or the build stops.
- The page is built as a Markdown file at that path would be: its
  frontmatter is read, its relative links lead from its folder, and its code
  blocks go to plugins, with `page` the file it would be, like
  `docs/data/sizes.md`. A page that would be the same page as a file in the
  source, or as another plugin's page, stops the build.
- A plugin that only adds pages gives back no languages from `setup`, so its
  `render` is never called.
