---
title: Configuring
description: Name your site, and choose its folders and repository, in little.yml.
---

# Configuring

A site's settings go in `little.yml`, next to its `docs/` folder:

```yaml
# little.yml
name: My Project
description: Docs that just work
```

Set only what you want to change. Everything else has a default, from
little or the theme, so the file can be empty, or missing. `little config`
prints every setting your site ends up with, and
[Settings](../reference/settings.md) lists them all, with how they combine.

## The built-in theme's settings

| Key | Default | Used for |
| --- | ------- | -------- |
| `name` | `Documentation` | the site's title, in the header and the browser's tab |
| `description` | none | the meta description of each page without [its own](../writing/frontmatter.md) |
| `lang` | `en` | each page's language, which [search](../writing/search.md) matches words in |
| `font` | a system UI font stack | the text's `font-family` |
| `search` | `true` | the [search](../writing/search.md) box in the header, and its index |
| `repo` | none | links to the [repository](#choosing-the-sites-repository), and to each page's file in it |

## Choosing folders

little builds a site from `docs/` into `dist/`. To use other folders, set
`source` and `destination`, relative to the folder with `little.yml`, or
absolute:

```yaml
# little.yml
source: ./content
destination: ./public
```

Each build replaces everything in the destination, so keep only builds
there.

## Choosing the site's repository

Set `repo` to your repository's address, as your browser shows it:

```yaml
# little.yml
repo: https://github.com/withlittle/little
```

The built-in theme links to it in its header. On github.com or gitlab.com,
it also links to view and edit each page's file, on the `main` branch,
unless you set another, like `branch: trunk`.

A page's [frontmatter](../writing/frontmatter.md#frontmatter-wins) can
change these links, or remove one, like `edit_url: false` on a page that's
generated from something else.
