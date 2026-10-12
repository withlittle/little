---
title: Choosing a theme
description: Build a site with little's built-in theme, a theme in a folder, or one downloaded, and build a theme on another.
---

# Choosing a theme

A theme owns all of a site's HTML, and its look. `theme` chooses the theme a
site is built with:

| `theme` | The site is built with… |
| ------- | ----------------------- |
| `true` (the default) | little's [built-in theme](#the-built-in-theme) |
| `false` | no theme, which `little build` can't do yet |
| a path, like `../shared/my-theme` | the theme in that folder |
| a URL, like `https://example.com/my-theme.tar.gz` | the theme in that archive, [downloaded](#downloading-a-theme) |

```yaml
# little.yml
theme: ../shared/my-theme
```

- Relative paths start from the folder holding the `little.yml` that sets
  them, not from where you run `little`.
- Absolute paths work too, like `/Users/you/my-theme` on macOS or
  `C:\Users\you\my-theme` on Windows. YAML reads `\` as an escape inside
  double quotes, so leave Windows paths unquoted or use single quotes.
- Only unquoted `true` and `false` choose the built-in theme or none:
  `"true"`, `yes`, and `no` are all folder names.

## The built-in theme

little's built-in theme is compiled into little, so a site needs nothing
else to look like this manual: a sidebar of its nav, a table of contents,
breadcrumbs, previous and next pages, a search box, and links to each page's
file in its repository. Its
[settings](../getting-started/configuring.md#the-built-in-themes-settings)
name the site, describe it, and choose its font and language. Its source is
the [`theme/`](https://github.com/withlittle/little/tree/main/theme) folder in
little's repository, and another theme can [build on
it](#building-on-another-theme), file by file.

- It's light or dark, as the reader's system is, code blocks too, but a
  button in its header lets the reader choose, and their browser remembers
  it. Choosing what their system shows follows the system again.
- It fits any screen: on a phone or a tablet, the nav is a menu, opened from
  the header, and the table of contents folds up above the page. See
  [Navigation](../writing/navigation.md). A site name too long for the
  header wraps onto a second line, and ends in … if it's too long for two,
  so the buttons beside it stay on the screen.
- Its sidebar shows each top-level folder as a group, and folds up each
  folder within one, but those the current page is in, so a large site's
  nav stays short. See [The nav](../writing/navigation.md#the-nav).
- On a site with [versions](../writing/versions.md#the-menu-of-versions), a
  menu in its header, beside the site's name, leads to the page you're on
  in each version, and on each version but the one at `/`, a note above the
  page leads to that one.
- A title that's a long name, like a Python module's
  `enable_iterative_imputer`, wraps after each `_` and `.` in the sidebar,
  the tables of contents, the breadcrumbs, and the previous and next pages,
  rather than running out of them. A title with spaces wraps at its spaces.
- It's built to be accessible: a link to skip to the content, a visible
  focus ring for the keyboard, text that meets WCAG's AA contrast, underlined
  links, and no sliding or smooth scrolling for a reader who prefers reduced
  motion. It works in forced colours, like Windows' contrast themes, and
  prints each page without its header and nav.
- It works without JavaScript, but for search. Its script only adds to it:
  the button to choose light or dark, closing the menu of versions on a
  click elsewhere, scrolling the sidebar to the current page, tables as
  wide as the page, a button to [copy each code
  block](../writing/code.md#copying-code), and, in the table of contents
  beside the page, a mark on the section being read.
- It needs a browser from 2024 or later.
- It's plain CSS, in cascade layers, so CSS that isn't in a layer, like a
  stylesheet that a theme building on it links from its own `default.html`,
  wins over it, however specific its selectors. Only a few of its
  declarations are `!important`, which CSS outside a layer can't override:
  those for Pagefind's search button on a narrow screen, a diagram's
  background, which `--diagram-bg` sets, and what print hides. Its colours
  and sizes are custom properties, like `--accent`, each colour a pair for
  light and dark, like `light-dark(#7c3aed, #a78bfa)`, so setting one
  changes it everywhere.

## Downloading a theme

Set `theme` to the URL of a theme's `.zip` or `.tar.gz` archive, and the first
build downloads it. On GitHub, a tag's archive is at
`https://github.com/OWNER/REPO/archive/refs/tags/TAG.tar.gz`, so to build a
site with [theme-bootstrap](https://github.com/withlittle/theme-bootstrap):

```yaml
# little.yml
theme: https://github.com/withlittle/theme-bootstrap/archive/refs/tags/0.0.1.tar.gz
```

- The archive must hold the theme in a single folder, as GitHub's do.
- little keeps the theme in the site's `.little/` folder, which git ignores,
  and never downloads that URL again, so later builds work offline. To use
  another version, change the URL; to download the same URL again (say, a
  branch's archive, which changes), delete `.little/`.
- Downloads are over HTTPS only, and little can't sign in, so a private
  repository's archive answers 404. For safety, an archive holding symlinks,
  hard links, or paths that lead out of it is an error.
- `little serve` doesn't watch a downloaded theme, as it never changes.

## Building on another theme

A theme's own `little.yml` can set `theme` too, to build on another theme.
Its settings go over that theme's, and its files go over that theme's, file
by file, so it needs only what it changes. A company theme can be just a
`little.yml` that builds on a downloaded theme:

```yaml
# company-theme/little.yml
theme: https://github.com/withlittle/theme-bootstrap/archive/refs/tags/0.0.1.tar.gz
font: "Inter, sans-serif"
colors:
  accent: navy
```

A site that uses it cascades **little → theme-bootstrap → company-theme →
site**. Themes can build on themes that build on others, but not in a loop.
In a theme's `little.yml`, a path is relative to the theme's folder, and a
URL is downloaded into the site's `.little/`.

- A template in `_templates/` replaces the base theme's template at the same
  path, and so does a file in `_sass/`. A file in `_static/` replaces the
  base theme's file published at the same path, so a `style.scss` replaces a
  `style.css`, and the other way round. Templates can extend and include the
  base theme's too.
- A template can't `{% extends %}` the one it replaces: it would extend
  itself. To change part of a base theme's template, replace a smaller one it
  `{% include %}`s instead.

To make a theme of your own, see [Making a theme](making-a-theme.md).
