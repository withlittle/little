---
title: Choosing a theme
description: Build a site with little's built-in theme, a theme in a folder, or one downloaded, and build a theme on another.
---

# Choosing a theme

A theme owns all of a site's HTML, and its look. little builds your site
with its [built-in theme](#the-built-in-theme), unless you set `theme` in
`little.yml`:

```yaml
# little.yml
theme: ../shared/my-theme
```

| `theme` | The site is built with… |
| ------- | ----------------------- |
| `true` (the default) | little's [built-in theme](#the-built-in-theme) |
| a path, like `../shared/my-theme` | the theme in that folder |
| a URL, like `https://example.com/my-theme-1.0.tar.gz` | the theme in that archive, [downloaded](#downloading-a-theme) |

A path can be absolute, or relative to the folder of the `little.yml` that
sets it, not to where you run `little`.

## The built-in theme

The built-in theme is compiled into little, so your site needs nothing else
to look like this manual. It has a sidebar of the site's
[nav](../writing/navigation.md#the-nav), a table of contents, breadcrumbs,
previous and next pages, a search box, links to your
[repository](../getting-started/configuring.md#choosing-the-sites-repository),
and, on a site with versions, a
[menu of them](../writing/versions.md#the-menu-of-versions).
[Configuring](../getting-started/configuring.md#the-built-in-themes-settings)
lists its settings.

It's light or dark, as the reader's system is, and a button in its header
lets them pick one, which their browser remembers. On a phone or a tablet,
the nav is a menu, opened from the header, and the table of contents folds
up above the page. The sidebar shows each top-level folder as a group, and
folds up each folder within a group, except those the current page is in,
so a large site's nav stays short.

It's built to be accessible, with a link to skip to the content, a visible
focus ring, text that meets WCAG's AA contrast, underlined links, and no
sliding or smooth scrolling for a reader who prefers reduced motion. It
works in forced colours, like Windows' contrast themes, and prints each
page without its header and nav.

Everything but search works without JavaScript. The theme's script only
adds to it, like the buttons to choose light or dark and to
[copy code](../writing/code.md#copying-code).

To restyle it, [build a theme on it](#building-on-another-theme), and link
a stylesheet of your own from a copy of its
[`default.html`](https://github.com/withlittle/little/blob/main/theme/_templates/default.html).
Its [CSS](https://github.com/withlittle/little/blob/main/theme/_static/assets/style.css)
is in cascade layers, so CSS outside a layer wins over it, however specific
its selectors. Yours can't override the few declarations it marks
`!important`: those for Pagefind's search button on a narrow screen, a
diagram's background, which `--diagram-bg` sets, and what print hides. Its
colours and sizes are custom properties, like `--accent`, so setting one
changes it everywhere, and each colour is a pair for light and dark, like
`light-dark(#7c3aed, #a78bfa)`.

## Downloading a theme

Set `theme` to the URL of a theme's `.zip` or `.tar.gz` archive, and the
first build downloads it:

```yaml
# little.yml
theme: https://example.com/my-theme-1.0.tar.gz
```

The archive must hold the theme in a single folder, like the archive a git
host makes of a tag, and be at an `https://` URL that needs no sign-in.
little keeps the theme in the site's `.little/` folder, which git ignores,
and never downloads that URL again, so later builds work offline. To use
another version, change the URL. To download the same URL again, like a
branch's archive, which changes, delete `.little/`.

A download stops if it takes over 5 minutes, or is over 200 MB. On a slow
connection, download and unpack the archive yourself, and set `theme` to
the theme's folder.

## Building on another theme

A theme's own `little.yml` can set `theme` too, to build on another theme,
like `theme: true` for the built-in one; a theme that doesn't set it builds
on none. Its settings go over that theme's, and its files replace that
theme's, so it needs only what it changes. A company
theme can be just a `little.yml` that builds on a downloaded one:

```yaml
# company-theme/little.yml
theme: https://example.com/my-theme-1.0.tar.gz
font: "Inter, sans-serif"
colors:
  accent: navy
```

A site that uses it cascades **little → my-theme → company-theme → site**.
In a theme's `little.yml`, a path is relative to the theme's folder, and a
URL is downloaded into the site's `.little/`.

A file in `_templates/` or `_sass/` replaces the one at the same path in the
theme it builds on, and a file in `_static/` the one published at the same
path, so a `style.scss` replaces a `style.css`, and the other way round. A
template can `{% extends %}` or `{% include %}` that theme's templates, but
not the one it replaces, which would extend itself: to change part of a
template, replace a smaller one it `{% include %}`s instead.

To make a theme of your own, see [Making a theme](making-a-theme.md).
