---
title: Your first site
description: Start a site, add pages to it, and build it.
---

# Your first site

```sh
little init my-docs
cd my-docs
little serve
```

This starts a site in `my-docs/`, and serves it at
<http://127.0.0.1:8080>. Open it, edit `docs/README.md`, the home page, and
save: the page reloads itself.

To add docs to a project you already have, run `little init` in its folder.
[`little init`](../reference/commands.md#little-init) says what it writes,
and what it keeps.

## Add pages

Each `.md` file in `docs/` is a page. Add `docs/guide/install.md`:

```markdown
# Install

## Options
```

It's at `/guide/install`, and the sidebar lists it under Guide, titled
Install, after its `#` heading. [Pages and files](../writing/pages-and-files.md)
says how files become pages, and [Markdown](../writing/markdown.md) what
you can write in them.

## Link them

Link to a page by its file, relative to the page you're writing, like this
in `docs/README.md`:

```markdown
See [Install](guide/install.md), and its [options](guide/install.md#options).
```

The link works in the site, and wherever else your Markdown files are
read, like on GitHub. [Links and images](../writing/links-and-images.md)
says more.

## Build it

```sh
little build
```

This builds the site into `dist/`, and checks every link in it, warning of
each broken one, like a misspelt `#options`:

```text
built 2 page(s) -> ./dist
warning: broken link guide/install.md#optoins in ./docs/README.md: Cannot find fragment
```

`dist/` is ready to [publish](publishing.md).
