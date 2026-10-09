---
title: Using plugins
description: Render code blocks in languages of their own, or add pages, with plugins, each one .wasm file run in a sandbox.
---

# Using plugins

A plugin renders code blocks in a language of its own, as little builds each
page, like a table from a ```` ```csv ```` block, or a diagram from one in a
language little doesn't draw, or adds pages to the site, like an [API
reference](python-api-reference.md). It's one `.wasm` file, which runs
wherever little does, in a sandbox.

## Naming a plugin

`plugins` names each plugin a site uses, and its file, by a path or a URL:

```yaml
# little.yml
plugins:
  csv: ./plugins/little_csv.wasm
  bob: https://example.com/svgbob-1.0.wasm
```

A plugin's settings, which it reads itself, go beside its file, in `wasm`:

```yaml
# little.yml
plugins:
  python:
    wasm: ./plugins/little_python.wasm
    read: src
    style: google
```

- A path is relative to the folder of the `little.yml` that sets it, as for
  a [theme](../themes/choosing-a-theme.md). A URL must start with
  `https://`: the first build downloads it into the site's
  `.little/plugins/`, and little never downloads that URL again, so later
  builds work offline. To use another version, change the URL.
- A theme can name plugins in its `little.yml`, for its sites to use, and
  the site's `little.yml` has the last word, plugin by plugin and setting by
  setting, so a site can change a setting of its theme's plugin, or turn one
  off, like `python: false`. A page can't change them in its frontmatter.

## Code blocks

A block is a plugin's by its language, the first word after its fence, up to
a comma, in any case, as for [highlighting](../writing/code.md#languages).
The plugin says which languages it renders, and two plugins that render the
same one stop the build. A block can pass the plugin options after its
language, like ```` ```csv headers=false ````, for it to read.

- A plugin gives back Markdown, which little writes as if you had: its
  headings join the page's table of contents, its links are checked, its
  math is rendered, its diagrams drawn, and its code highlighted. Or it gives
  back HTML, which little writes as it is, so it needs
  [`raw_html`](../writing/markdown.md#choosing-markdowns-extensions): with
  `raw_html: false`, the build stops.
- In a [notebook](../writing/notebooks.ipynb), the blocks in Markdown cells,
  and in Markdown a cell showed, go to plugins too, and are warned of without
  a line. Its code cells don't.
- A block a plugin can't render, as it gives back an error, or crashes, is
  left as it was, as code, and warned of, with the file and line it's on:

  ```text
  warning: plugin `csv` can't render the block on line 9 of ./docs/sizes.md: the table is empty
  ```

  `little build --strict` fails on one, and `little serve` warns of each
  after each build. After a crash, little starts the plugin again for the
  next block. A plugin that can't start, as its settings are wrong, say,
  stops the build.

## Pages

A plugin can add pages, each built as if it were a Markdown file in the
source, at a path the plugin chooses, like `reference/mypackage/README.md`,
for `/reference/mypackage/`: it's in the nav and search, its links are
checked, and its code blocks go to plugins.

- A page that would be the same page as a file in the source, or as another
  plugin's page, stops the build, naming both.
- It has no file in the site's
  [repository](../getting-started/configuring.md#choosing-the-sites-repository),
  so no `page.source_url` or `page.edit_url`.
- What a plugin warns of as it adds its pages, like a file it can't read, is
  warned of, and `little build --strict` fails on it:

  ```text
  warning: plugin `docstrings`: can't parse `mypackage/broken.py`: Expected a parameter or the end of the parameter list on line 3
  ```

- A plugin that can't add its pages stops the build. `little serve` asks for
  them again in each build, which a change in a folder the plugin reads
  starts.

## The sandbox

A plugin reaches nothing little doesn't give it: not the network, your
environment variables, or your files, but the folders `read` names, from the
site's folder, like `src` or `[src, examples]`, which it can read, but not
change. Only the site's `little.yml` can set `read`, so a theme's plugin
reads your files only when you say so.

What a plugin prints goes to little's stderr. Its clock reads the time the
build started, and its random numbers are the same in each build, so a site
builds the same each time.

## Limits, for now

- A plugin's Markdown can't use the page's link references, or have
  footnotes.
- A plugin's pages can't be notebooks, and are all added again in each
  build, even for a change to one.
- A plugin has no time limit, so one that never returns holds the build up
  until you press Ctrl-C.
- A download isn't checked against a hash, so little trusts the site it's
  on.
- Each `little build` compiles each plugin again, which takes a moment for a
  small plugin, and about a second for one in Python. `little serve`
  compiles each once, and again when it changes.

To write your own, see [Writing a plugin](writing-a-plugin.md).
