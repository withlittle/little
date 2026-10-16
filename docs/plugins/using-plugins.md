---
title: Using plugins
description: Add to what little does as it builds a site, with plugins, each one .wasm file, run in a sandbox, in the order you name them.
---

# Using plugins

A plugin adds to what little does as it builds a site: an API reference,
from [Python's docstrings](python-api-reference.md),
[Rust's doc comments](rust-api-reference.md), or
[JavaScript's](javascript-api-reference.md), say, links to other sites that
open in a new tab, or a table from a ```` ```csv ```` block. It's one
`.wasm` file, which runs wherever little does, in a sandbox, and little
calls its [hooks](../reference/plugin-hooks.md) as the build reaches each
step: as it reads the site's config, its theme, its files, and each page,
and as it writes them.

## Naming a plugin

`plugins` names each plugin a site uses, and its file, by a path or a URL:

```yaml
# little.yml
plugins:
  links: ./plugins/links.wasm
  csv: https://example.com/little-csv-1.0.wasm
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
  builds work offline. To use another version, change the URL. A download
  stops, as a [theme's](../themes/choosing-a-theme.md#downloading-a-theme)
  does, if it takes over 5 minutes, or is over 200 MB.
- A theme can name plugins in its `little.yml`, for its sites to use, and
  the site's `little.yml` has the last word, plugin by plugin and setting by
  setting, so a site can change a setting of its theme's plugin, or turn one
  off, like `python: false`. A page can't change them in its frontmatter.

## The order they run in

little calls each hook of each plugin that has it in the order `plugins`
names them, a theme's first, then the site's, so each is given what the
one before it gave back. A site that changes a setting of its theme's
plugin leaves it in its place.

## Code blocks

A plugin can render fenced code blocks in languages of its own. A block is
a plugin's by its language, the first word after its fence, up to a comma,
in any case, as for [highlighting](../writing/code.md#languages). Two
plugins that render the same language stop the build. A block can pass the
plugin options after its language, like ```` ```csv headers=false ````, for
it to read.

- A plugin gives back Markdown, which little writes as if you had: its
  headings join the page's table of contents, its links are checked, its
  math is rendered, its diagrams drawn, and its code highlighted. Or it gives
  back HTML, which little writes as it is, so it needs
  [`raw_html`](../writing/markdown.md#choosing-markdowns-extensions): with
  `raw_html: false`, the build stops.
- In a [notebook](../writing/notebooks.ipynb), the blocks in Markdown cells,
  and in Markdown a cell showed, go to plugins too, and are warned of without
  a line. Its code cells don't.
- A block a plugin can't render is left as it was, as code, and warned of,
  with the file and line it's on:

  ```text
  warning: plugin `csv` can't render the block on line 9 of ./docs/sizes.md: the table is empty
  ```

## Pages and files

A plugin can add pages, and other files, to a site, or leave files in its
source out, with its [`files`](../reference/plugin-hooks.md#files) hook. A
page it adds is built as if its file were in the source, at the path the
plugin chooses, like `reference/mypackage/README.md`, for
`/reference/mypackage/`: it's in the nav and search, its links are checked,
and its code blocks go to plugins.

- A page that would be the same page as a file in the source, or as another
  plugin's page, stops the build, naming both.
- It has no file in the site's
  [repository](../getting-started/configuring.md#choosing-the-sites-repository),
  so no `page.source_url` or `page.edit_url`.
- `little serve` adds them again in each build, which a change in a folder
  the plugin reads starts.

## When a plugin fails

A hook fails when it gives back an error, or its plugin crashes, as a Rust
plugin does when it panics. After a crash, little starts the plugin again,
for the next hook.

- A plugin that can't start, as its settings are wrong, say, stops the
  build, as does a hook that little calls once, like `files`, that fails.
- A hook that little calls for each page, or block, that fails is warned
  of, and the page is built as if the hook hadn't run:

  ```text
  warning: plugin `links`'s `content` hook failed on ./docs/guide.md: …
  ```

- A plugin can warn of what it needs to, like a file it can't read:

  ```text
  warning: plugin `pydoc`: can't parse `mypackage/broken.py`: Expected a parameter or the end of the parameter list on line 3
  ```

`little build --strict` fails on any of these warnings, and `little serve`
shows each after each build.

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
- A plugin's pages are all added again in each build, even for a change to
  one.
- A plugin has no time limit, so one that never returns holds the build up
  until you press Ctrl-C.
- A download isn't checked against a hash, so little trusts the site it's
  on.
- Each `little build` compiles each plugin again, which takes a moment for a
  small plugin, and about a second for one in Python. `little serve`
  compiles each once, and again when it changes.
- `little config` prints the site's config before plugins change it.

[Plugin hooks](../reference/plugin-hooks.md#limits-for-now) lists the
hooks' own limits. To write a plugin, see
[Writing a plugin](writing-a-plugin.md).
