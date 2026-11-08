---
title: Home
description: The little static documentation site generator for big things.
---

# little

The little static documentation site generator for big things. Put
Markdown in `docs/`, and get a site.

> [!WARNING]
> little is in early development, before 1.0, so expect breaking changes.
> This manual describes little as it is on its `main` branch.

## Start in a minute

```sh
cargo install --locked little-docs
little init my-docs
cd my-docs
little serve
```

[Your first site](getting-started/first-site.md) goes on from there, and
[Install](getting-started/install.md#install-without-rust) says how to
install little without Rust.

## Why little

- **Markdown that just works.** Write as you would on GitHub, with tables,
  callouts, footnotes, emoji, math, and Mermaid diagrams, and link to a
  page by its file. little highlights code, renders math, and draws
  diagrams as it builds, so pages need no JavaScript to show them.
- **One binary.** No runtime, no `node_modules`, and no version managers:
  search, link checking, Sass, math, and diagrams are built in.
- **Themes own the markup.** little gives a theme its pages as data, and
  writes no HTML of its own, so a theme can use Bootstrap, Tailwind, or
  your own CSS.
- **Config that cascades.** little has defaults, a theme sets its own, and
  your site sets only what it changes.
- **Plugins in a sandbox.** A plugin is one WebAssembly file, written in
  Rust, Python, or another language, that hooks into each step of a build.
  little's own plugins add API references for [Python](plugins/pydoc.md),
  [Rust](plugins/rustdoc.md), [JavaScript](plugins/jsdoc.md),
  [Go](plugins/godoc.md), and [OpenAPI](plugins/openapi.md), and read
  [Jupyter notebooks](plugins/jupyter.ipynb),
  [AsciiDoc](plugins/asciidoc.adoc), [Org mode](plugins/org-mode.org), and
  [reStructuredText](plugins/restructuredtext.rst).

## This manual

| Section | Read it to… |
| ------- | ----------- |
| [Getting started](getting-started/install.md) | install little, start a site, configure it, and publish it |
| [Writing](writing/pages-and-files.md) | write pages, with links, code, math, diagrams, and navigation, and build each release's docs |
| [Themes](themes/choosing-a-theme.md) | choose a theme, or make one |
| [Plugins](plugins/using-plugins.md) | use or write plugins, for other formats or an API reference |
| [Reference](reference/commands.md) | look up a command, a setting, a template's data, or a plugin's hooks |

> [!TIP]
> This manual is a little site, in the `docs/` folder of little's
> [repository](https://github.com/withlittle/little). Each page uses what it
> describes: the page on [math](writing/math.md) has math in it, and the
> page on [Jupyter notebooks](plugins/jupyter.ipynb) is a notebook.
