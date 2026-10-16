---
title: Home
description: little turns a folder of Markdown, or Jupyter notebooks, into a documentation site, with one binary and no config.
---

# little

A little static site generator for documentation: drop Markdown, or Jupyter
notebooks, into `docs/` and get a site. One self-contained binary, a
three-level cascading config, and themes that own 100% of the markup.

> [!WARNING]
> little is in early development, at v0.1, so expect breaking changes. This
> manual describes little as it is on its `main` branch.

## Start in a minute

With little [installed](getting-started/install.md), make a folder with a
page in it, and serve it:

```sh
mkdir -p my-docs/docs && cd my-docs
echo "# Hello, little" > docs/README.md
little serve
```

Open <http://127.0.0.1:8080>, edit `docs/README.md`, and the page reloads
itself. [Your first site](getting-started/first-site.md) goes on from there.

## Why little

- **Markdown that just works.** Write as you would on GitHub, with tables,
  callouts, footnotes, emoji, math, and Mermaid diagrams, and link to a page
  by its file. Code is highlighted, math rendered, and diagrams drawn as each
  page is built, so readers' browsers show them without JavaScript.
- **One binary.** No runtime, no `node_modules`, and no version managers:
  search, link checking, Sass, math, and diagrams are built in.
- **Themes own the markup.** little gives a theme its pages as data, and
  writes no HTML of its own, so a theme can be anything, with Bootstrap,
  Tailwind, or CSS of your own.
- **Config that cascades.** little's defaults, then the theme's, then the
  site's: a site sets only what it changes.
- **Plugins in a sandbox.** A plugin is one WebAssembly file, written in Rust,
  Python, or another language, that hooks into each step of a build, to add
  an API reference from [Python's docstrings](plugins/python-api-reference.md),
  [Rust's doc comments](plugins/rust-api-reference.md), or
  [JavaScript's](plugins/javascript-api-reference.md), say, or change each
  page as it's written.

## This manual

| Section | Read it to… |
| ------- | ----------- |
| [Getting started](getting-started/install.md) | install little, start a site, configure it, and publish it |
| [Writing](writing/pages-and-files.md) | write pages, with links, code, math, diagrams, notebooks, and navigation, and keep a version for each release |
| [Themes](themes/choosing-a-theme.md) | choose a theme for a site, or make one |
| [Plugins](plugins/using-plugins.md) | use plugins, add an API reference, or write a plugin |
| [Reference](reference/commands.md) | look up a command, a setting, what a theme's templates get, or a plugin's hooks |

> [!TIP]
> This manual is a little site, in the `docs/` folder of little's
> [repository](https://github.com/withlittle/little), and each page uses what
> it describes: the page on [math](writing/math.md) has math in it, and the
> page on [notebooks](writing/notebooks.ipynb) is a notebook. To read it
> offline, run `little serve` in a clone of the repository.
