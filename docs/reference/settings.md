---
title: Settings
description: Every setting little has, its default, and where it's documented.
---

# Settings

Every setting goes in a `little.yml`: little's own, compiled into it, then
the theme's, then the site's, each over the last, as [Configuring](../getting-started/configuring.md)
explains. `little config` prints them, merged.

## little's settings

little sets these in its own
[`little.yml`](https://github.com/withlittle/little/blob/main/src/little.yml),
or works them out, and a theme or a site can change any of them:

| Key | Default | Chooses… |
| --- | ------- | -------- |
| `source` | `./docs` | the folder a site is [built from](../getting-started/configuring.md#choosing-folders) |
| `destination` | `./dist` | the folder a site is [built into](../getting-started/configuring.md#choosing-folders) |
| `theme` | `true`, the built-in theme | the [theme](../themes/choosing-a-theme.md) a site is built with, or, in a theme's `little.yml`, the one it [builds on](../themes/choosing-a-theme.md#building-on-another-theme) |
| `url` | none | where the site's [published](../getting-started/publishing.md#choosing-the-sites-url), like `https://withlittle.github.io/little` |
| `repo` | none | the [repository](../getting-started/configuring.md#choosing-the-sites-repository) its source is in |
| `branch` | `main` | the branch its pages' files are [linked to](../getting-started/configuring.md#choosing-the-sites-repository) on |
| `nav` | the source's pages and folders, sorted by title | the site's [nav](../writing/navigation.md#writing-the-nav-yourself) |
| `toc_min_depth` | `2`, for `##` | the level of heading a page's [table of contents](../writing/navigation.md#tables-of-contents) lists from |
| `toc_max_depth` | `6`, for `######` | the level it lists down to |
| `search` | `false` | whether the site's [indexed for search](../writing/search.md) |
| `markdown` | GitHub's extensions | the [extensions to Markdown](../writing/markdown.md#choosing-markdowns-extensions) pages are written with |
| `markdown.highlight` | `pygments` | the classes [code is highlighted](../writing/code.md#choosing-how-code-is-highlighted) with |
| `markdown.math` | `mathml` | how [math is written](../writing/math.md#choosing-how-math-is-written) |
| `markdown.mermaid` | `svg` | how [diagrams are drawn](../writing/diagrams.md#choosing-how-diagrams-are-drawn) |
| `mermaid` | none | how [diagrams look](../writing/diagrams.md#choosing-how-diagrams-look), in Mermaid's own config |
| `plugins` | none | the [plugins](../plugins/using-plugins.md) a site uses, their files, and their settings |
| `host` | `127.0.0.1` | the host `little serve` [serves at](commands.md#choosing-where-to-serve) |
| `port` | `8080` | the port it serves on |
| `year` | the year the build starts in | the year a theme's [copyright notice](../themes/making-a-theme.md#a-copyright-notice) shows |

- A path in `theme` or `plugins` is relative to the folder of the
  `little.yml` that sets it, but `source` and `destination` are always
  relative to the site's folder.
- In `plugins`, only the site can set a plugin's `read`, the folders it can
  read.
- little works out `nav` and `year` as it builds, so `little config`
  doesn't print them, unless a `little.yml` sets them, which replaces what
  little works out.

## The built-in theme's settings

The built-in theme's
[`little.yml`](https://github.com/withlittle/little/blob/main/theme/little.yml)
goes over little's, and the site's goes over it:

| Key | Default | Chooses… |
| --- | ------- | -------- |
| `name` | `Documentation` | the site's title, in the header and the browser's tab |
| `description` | none | the text beside the site's title |
| `lang` | `en` | each page's language, which [search](../writing/search.md) matches words in |
| `font` | a system UI font stack | the text's `font-family` |
| `toc_max_depth` | `3`, for `###` | the level a page's table of contents lists down to |
| `search` | `true` | whether the site's indexed, for the search box in the header |

It uses `repo` too, to link to the repository, and to each page's file in
it. [Configuring](../getting-started/configuring.md#the-built-in-themes-settings)
says more.

## Your own settings

Any other key reaches the theme as `site.<key>`, like `site.version` for
`version: "2.1"`, for a theme that shows it. A key in a page's frontmatter
reaches the theme as `page.<key>` instead, and changes only that page: see
[Frontmatter](../writing/frontmatter.md#frontmatter-wins).
