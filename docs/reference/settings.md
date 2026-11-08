---
title: Settings
description: Every setting little has, its default, and where it's documented.
---

# Settings

Every setting goes in a `little.yml`: little's own, compiled into it, then
the theme's, then the site's. Each merges over the one before it, key by
key, so `markdown: { math: tex }` changes only `math`, and keeps the rest of
`markdown`. Any other value, like a list, replaces the one beneath it.
[`little config`](commands.md#little-config) prints them, merged.

## little's settings

little sets these in its own
[`little.yml`](https://github.com/withlittle/little/blob/main/src/little.yml),
or works them out. A theme or a site can change any of them:

| Key | Default | Chooses… |
| --- | ------- | -------- |
| `source` | `./docs` | the folder a site is [built from](../getting-started/configuring.md#choosing-folders) |
| `destination` | `./dist` | the folder a site is [built into](../getting-started/configuring.md#choosing-folders) |
| `theme` | `true`, the built-in theme, for a site; none for a theme | the [theme](../themes/choosing-a-theme.md) a site is built with, or the one a theme [builds on](../themes/choosing-a-theme.md#building-on-another-theme) |
| `url` | none | where the site's [published](../getting-started/publishing.md#choosing-the-sites-url), like `https://withlittle.github.io/little` |
| `repo` | none | the [repository](../getting-started/configuring.md#choosing-the-sites-repository) its source is in |
| `branch` | `main` | the branch its pages' files are [linked to](../getting-started/configuring.md#choosing-the-sites-repository) on |
| `nav` | the source's pages and folders, [sorted by title](../writing/navigation.md#the-nav) | the site's [nav](../writing/navigation.md#writing-the-nav-yourself) |
| `toc_min_depth` | `2`, for `##` | the level of heading a page's [table of contents](../writing/navigation.md#tables-of-contents) lists from |
| `toc_max_depth` | `6`, for `######`; `3`, for `###`, in the built-in theme | the level it lists down to |
| `search` | `false`; `true` in the built-in theme | whether the site's [indexed for search](../writing/search.md): `true`, `false`, or its settings, which turn it on |
| `search.exclude` | none | the URLs of the pages [left out of search](../writing/search.md#leaving-pages-out), each with those under it |
| `markdown` | GitHub's extensions | the [extensions to Markdown](../writing/markdown.md#choosing-markdowns-extensions) pages are written with |
| `markdown.highlight` | `pygments` | the classes [code is highlighted](../writing/code.md#choosing-how-code-is-highlighted) with |
| `markdown.math` | `mathml` | how [math is written](../writing/math.md#choosing-how-math-is-written) |
| `markdown.mermaid` | `svg` | how [diagrams are drawn](../writing/diagrams.md#choosing-how-diagrams-are-drawn) |
| `mermaid` | none | how [diagrams look](../writing/diagrams.md#choosing-how-diagrams-look), in Mermaid's own config |
| `plugins` | none | the [plugins](../plugins/using-plugins.md) a site uses, their files, and their settings |
| `versions` | none | the site's [versions](../writing/versions.md), each with its `title`, `url`, and git `ref` |
| `host` | `127.0.0.1` | the host `little serve` [serves at](commands.md#choosing-where-to-serve) |
| `port` | `8080` | the port it serves on |
| `year` | the year the build starts in | the year a theme's [copyright notice](../themes/making-a-theme.md#a-copyright-notice) shows |

## The built-in theme's settings

The built-in theme's
[`little.yml`](https://github.com/withlittle/little/blob/main/theme/little.yml)
goes over little's, with settings of its own, like `name` and `lang`, which
[Configuring](../getting-started/configuring.md#the-built-in-themes-settings)
lists.

## Your own settings

Any other key reaches the theme as `site.<key>`, for a theme that shows it,
like `announcement: "2.1 is out!"` as `site.announcement`. A key in a
page's [frontmatter](../writing/frontmatter.md#frontmatter-wins) reaches it
as `page.<key>` instead, and changes only that page.
