---
title: Configuring
description: Name your site, and choose its colours, fonts, folders, and repository, in little.yml.
---

# Configuring

A site's settings go under `site` in `little.yml`, next to its `docs/`
folder:

```yaml
# little.yml
site:
  name: My Project
  description: Docs that just work
```

Set only what you want to change. Everything else has a default, from
little or the theme, so the file can be empty, or missing. `little config`
prints every setting your site ends up with, and
[Settings](../reference/settings.md) lists them all, with how they combine.

## The built-in theme's settings

Each goes under `site`:

| Key | Default | Used for |
| --- | ------- | -------- |
| `name` | `Documentation` | the site's title, in the header and the browser's tab |
| `description` | none | the meta description of each page without [its own](../writing/frontmatter.md) |
| `language` | `en` | each page's language, which [search](../writing/search.md) matches words in |
| `theme.colors.primary` | `"#7c3aed"` | links, markers, the tint of the greys, and the Important [callout](../writing/markdown.md#callouts) |
| `theme.colors.info`, `success`, `warning`, `danger` | a blue, a green, an amber, and a red | the Note, Tip, Warning, and Caution callouts |
| `theme.fonts.body` | the system's | the text's `font-family` |
| `theme.fonts.heading` | the body's | the headings', and the site's name's |
| `theme.fonts.code` | the system's monospace | code's |
| `theme.radius` | `0.5rem` | how round corners are |
| `search.enabled` | `true` | the [search](../writing/search.md) box in the header, and its index |
| `repository.url` | none | links to the [repository](#choosing-the-sites-repository), and to each page's file in it |

## Choosing colours and fonts

The built-in theme takes its colours, its fonts, and how round its corners
are from `theme`:

```yaml
# little.yml
site:
  theme:
    colors:
      primary: "#0d9488"
    fonts:
      heading: Georgia, serif
    radius: 0.25rem
```

Put a colour in quotes, as YAML reads what follows a `#` as a comment. The
theme takes each colour's hue and strength, and chooses how light it is
itself, so your `primary` reads as a link, and tints the greys, in light
and dark alike, whatever colour it is.

Each font is a CSS `font-family`, and `radius` a CSS length, or `0` for
square corners.

## Choosing folders

little builds a site from `docs/` into `dist/`. To use other folders, set
`source.directory` and `destination`, relative to the folder with
`little.yml`, or absolute:

```yaml
# little.yml
site:
  source:
    directory: ./content
  destination: ./public
```

Each build replaces everything in the destination, so keep only builds
there. To keep some of the source's files out of your site, see
[Leaving files out](../writing/pages-and-files.md#leaving-files-out).

## Choosing the site's repository

Set `repository.url` to your repository's address, as your browser shows
it:

```yaml
# little.yml
site:
  repository:
    url: https://github.com/withlittle/little
```

The built-in theme links to it in its header. On github.com or gitlab.com,
it also links to view and edit each page's file, on the `main` branch,
unless you set another beside the `url`, like `branch: trunk`.

A page's [frontmatter](../writing/frontmatter.md#frontmatter-wins) can
change these links, or remove them, with `repository: { enabled: false }`,
on a page that's generated from something else.
