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
