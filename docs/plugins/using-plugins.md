---
title: Using plugins
description: Add to what little does as it builds a site, with plugins, each one .wasm file, run in a sandbox, in the order you name them.
---

# Using plugins

A plugin adds to what little does as it builds a site. Name each plugin
your site uses in `plugins`, with its `.wasm` file:

```yaml
# little.yml
plugins:
  links: ./plugins/links.wasm
  csv: https://example.com/little-csv-1.0.wasm
```

little calls each plugin's [hooks](../reference/plugin-hooks.md) as the
build reaches each step, in a [sandbox](#the-sandbox). Use
[little's own plugins](#littles-own-plugins), or
[write your own](writing-plugins.md), like one that opens links to other
sites in a new tab.

## Naming a plugin

A plugin's file is a path or an `https://` URL, as a
[theme](../themes/choosing-a-theme.md) is. little
[downloads](../themes/choosing-a-theme.md#downloading-a-theme) a URL in the
site's first build, into `.little/plugins/`, and never again, so to update
a plugin, change its URL.

A plugin's settings go beside its file, in `wasm`:

```yaml
# little.yml
plugins:
  python:
    wasm: ./plugins/little_python.wasm
    read: src
    style: google
```

little runs plugins in the order `plugins` names them, and gives each what
the one before it gave back. A theme can name plugins in its `little.yml`
too, which run first. Your site's `little.yml` has the last word, plugin by
plugin and setting by setting, so it can change a setting of its theme's
plugin, which keeps its place, or turn one off, like `python: false`. A
page can't change plugins or their settings in its frontmatter.

## little's own plugins

little's own plugins add:

- an API reference, from [Python's docstrings](pydoc.md),
  [Rust's doc comments](rustdoc.md), [JavaScript's](jsdoc.md), or
  [Go's](godoc.md)
- the reference of an [HTTP API](openapi.md), from its OpenAPI document
- pages from [Jupyter notebooks](jupyter.ipynb), [AsciiDoc](asciidoc.adoc),
  [Org mode](org-mode.org), or [reStructuredText](restructuredtext.rst)

Each one's page gives the URL of its `.wasm` file. Its releases, among
[little's](https://github.com/withlittle/little/releases), list what
changed in each version, and the version of little's
[hooks](../reference/plugin-hooks.md) it's built for. Each carries a
`THIRD-PARTY-NOTICES.md`, with the licences of the crates it's built from.
To build one yourself, see its README, in
[`plugins/`](https://github.com/withlittle/little/tree/main/plugins).

## Code blocks

A plugin can render fenced code blocks in a language of its own, like a
table from a ```` ```csv ```` block. A block goes to the plugin by its
[language](../writing/code.md#languages), and can pass it options after
that, like ```` ```csv headers=false ````. In a page in
[another format](#pages-in-other-formats), the code blocks in its Markdown
go to plugins too.

The plugin gives back Markdown, which little writes as if you had, so its
headings join the page's table of contents, its links are checked, and its
math, diagrams, and code are rendered. Or it gives back HTML, which needs
[`raw_html`](../writing/markdown.md#choosing-markdowns-extensions). A block
it can't render stays code, and is warned of:

```text
warning: plugin `csv` can't render the block on line 9 of ./docs/sizes.md: the table is empty
```

## Pages and files

A plugin can add pages, and other files, to a site, or leave files in the
source out, with its [`files`](../reference/plugin-hooks.md#files) hook. A
page it adds is built as if its file were in the source, at the path the
plugin chooses, like `mypackage/README.md` for `/mypackage/`, so it's in
the nav and search, its links are checked, and its code blocks go to
plugins. It has no file in your
[repository](../getting-started/configuring.md#choosing-the-sites-repository),
so it has no `page.source_url` or `page.edit_url`.

## Pages in other formats

A plugin can read files in a format of its own as pages, by their
extension, with its [`formats`](../reference/plugin-hooks.md#formats) hook:
[`jupyter`](jupyter.ipynb) reads `.ipynb` files, and
[`asciidoc`](asciidoc.adoc), [`orgmode`](org-mode.org), and
[`restructuredtext`](restructuredtext.rst) read `.adoc`, `.org`, and `.rst`
files. Each file is a page, as a Markdown file is, so `analysis.ipynb` is
the page at `/analysis`, `README.ipynb` is its folder's page, and a link to
`analysis.ipynb` leads to the page. Without its plugin, the file is copied
as it is.

`asciidoc`, `orgmode`, and `restructuredtext` write each page as Markdown,
with the [extensions](../writing/markdown.md#choosing-markdowns-extensions)
your site turns on, so it
[looks](../themes/styling-markdown.md#asciidoc-org-mode-and-restructuredtext)
like the rest of your site. What needs an extension that's off is written
in HTML, or plainer Markdown, like an alert as a quote, with a warning
where something is lost. With `raw_html` on, you can write HTML in a page,
and anchors and headings keep the ids their format gives them, beside the
`#fragment` little gives each heading. Text shows as you write it, even
where Markdown would read it as markup, like the `*` in 2 * 3, or
`:smile:`.

## When a plugin fails

A hook fails when it gives back an error, or its plugin crashes, as a Rust
plugin does when it panics. little starts a crashed plugin again for the
next hook.

- The build stops if a plugin can't start, say because its settings are
  wrong, if a hook that little calls once, like `files`, fails, or if a
  `formats` hook can't read a page.
- Any other hook, which little calls for each page or block, is warned of
  when it fails, and the page is built as if the hook hadn't run:

  ```text
  warning: plugin `links`'s `content` hook failed on ./docs/guide.md: …
  ```

A plugin can warn of what it needs to, like a file it can't read:

```text
warning: plugin `pydoc`: can't parse `mypackage/broken.py`: Expected a parameter or the end of the parameter list on line 3
```

[`little build --strict`](../reference/commands.md#little-build) fails on
each of these warnings. A plugin has no time limit, so one that never
returns holds the build up until you press Ctrl-C.

## The sandbox

A plugin reaches nothing little doesn't give it: not the network, your
environment variables, or your files. It can read only the folders its
`read` setting names, from the site's folder, like `src` or
`[src, examples]`, and can't change them. Only your site's `little.yml` can
set `read`, so a theme's plugin reads your files only when you say so.

What a plugin prints goes to little's stderr. Its clock reads the time the
build started, and its random numbers are the same in each build, so a
site builds the same each time.
