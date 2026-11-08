---
title: Versions
description: Docs for each release, built from its git tag or branch, with a menu to switch between them.
---

# Versions

List your releases in `versions`, and little builds each one's docs from
its git tag, branch, or commit:

```yaml
# little.yml
versions:
  - title: "2.0"   # no ref: built from your files as they are
    url: /
  - title: "1.4"
    url: /1.4
    ref: v1.4
```

1.4's `docs/guide/install.md` is then at `/1.4/guide/install`, and the
built-in theme adds a [menu](#the-menu-of-versions) for switching versions.

## Listing versions

List versions in the order the menu should show them. Each has:

| Key | What it is |
| --- | ---------- |
| `title` | what the menu shows, like `1.4` or `2.0 (Alpha)` |
| `url` | where it's published, from the root of the site, like `/1.4` |
| `ref` | the git tag, branch, or commit it's built from, like `v1.4` |
| `source` | the folder its pages were in at `ref`, if they've moved since |

- One version must be at `/`. It needn't be the latest.
- Leave out `ref` on one version to build it from your files as they are,
  so `little serve` shows your edits.

## The menu of versions

With two or more versions, the built-in theme's header has a menu of them.
Switching keeps the reader on the same page, if that version has it. On
every version but the one at `/`, a note above each page names the
version, and links to the one at `/`.

For a menu in your own theme, see
[A menu of versions](../themes/making-a-theme.md#a-menu-of-versions).

## What's in a version

Every version is built by the little you run, with the site's theme,
plugins, and settings, so an old release gets today's look and fixes. Only
these come from its `ref`, which little reads with git, leaving your files
alone:

- Its pages and their files, from the `source` its entry sets, else the
  one in the `little.yml` at the ref, else the site's.
- Its nav, from the `little.yml` at the ref, else built from its pages,
  unless you [set its own](#a-versions-own-nav-and-plugins).
- The folders its plugins read, so an old release's
  [API reference](../plugins/pydoc.md) documents its own code.

A tag from before you used little works too.

## A version's own nav and plugins

For an old release that doesn't fit today's settings, a version from a
`ref` can set its own `nav` and plugins' settings:

```yaml
# little.yml
plugins:
  pydoc:
    wasm: ./little_pydoc.wasm
    read: ./src
versions:
  - title: "2.0"
    url: /
  - title: "1.4"
    url: /1.4
    ref: v1.4
    nav:
      - { title: Home, url: / }
      - { title: Install, url: /guide/install }
    plugins:
      pydoc:
        read: ./lib   # where its code was, at v1.4
```

- Write its `nav` like the
  [site's](navigation.md#writing-the-nav-yourself), with URLs from the
  version's home.
- Its `plugins` override the site's, setting by setting. Here, 1.4's
  `pydoc` reads `./lib` at `v1.4`, not `./src`. `pydoc: false` would turn
  it off for that version.
