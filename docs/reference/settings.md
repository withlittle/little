---
title: Settings
description: Every setting little has, its default, and where it's documented.
---

# Settings

A `little.yml` holds two groups, and nothing else: `site`, the site's
settings, and `page`, [settings for every page](#every-pages-settings).

```yaml
# little.yml
site:
  name: My Project
page:
  author: The team
```

little's own `little.yml` is compiled into it, the theme's goes over it,
and the site's over both. Each merges over the one before it, key by key,
so `markdown: { math: tex }` changes only `math`, and keeps the rest of
`markdown`. Any other value, like a list, replaces the one beneath it.
[`little config`](commands.md#little-config) prints them, merged.

## little's settings

little sets these in its own
[`little.yml`](https://github.com/withlittle/little/blob/main/src/little.yml),
or works them out. They go under `site`, and a theme or a site can change
any of them:

| Key | Default | Chooses… |
| --- | ------- | -------- |
| `source.directory` | `./docs` | the folder a site is [built from](../getting-started/configuring.md#choosing-folders) |
| `source.exclude` | none | the paths of the files there it [leaves out](../writing/pages-and-files.md#leaving-files-out), each with those under it |
| `destination` | `./dist` | the folder a site is [built into](../getting-started/configuring.md#choosing-folders) |
| `theme.source` | none, for the built-in theme | the [theme](../themes/choosing-a-theme.md) a site is built with, by its folder, or its archive's URL, or the one a theme [builds on](../themes/choosing-a-theme.md#building-on-another-theme) |
| `theme.enabled` | `true`, for a site; none, for a theme | in a theme, `true` [builds on](../themes/choosing-a-theme.md#building-on-another-theme) the built-in theme |
| `theme.mermaid` | none | how [diagrams look](../writing/diagrams.md#choosing-how-diagrams-look), in Mermaid's own config |
| `url` | none | where the site's [published](../getting-started/publishing.md#choosing-the-sites-url), like `https://withlittle.github.io/little` |
| `repository.url` | none | the [repository](../getting-started/configuring.md#choosing-the-sites-repository) its source is in |
| `repository.branch` | `main` | the branch its pages' files are [linked to](../getting-started/configuring.md#choosing-the-sites-repository) on |
| `repository.owner`, `repository.name` | from `repository.url` | the repository's owner and name, like `withlittle` and `little`, for a theme to show |
| `navigation.items` | the source's pages and folders, [sorted by title](../writing/navigation.md#the-nav) | the site's [nav](../writing/navigation.md#writing-the-nav-yourself) |
| `navigation.prune` | `false` | whether each page's nav lists [only the folders the page is in](../writing/navigation.md#pruning-the-nav) |

| `search.enabled` | `false`; `true` in the built-in theme | whether the site's [indexed for search](../writing/search.md) |
| `search.exclude` | none | the URLs of the pages [left out of search](../writing/search.md#leaving-pages-out), each with those under it |
| `markdown` | GitHub's extensions | the [extensions to Markdown](../writing/markdown.md#choosing-markdowns-extensions) pages are written with |
| `markdown.highlight` | `pygments` | the classes [code is highlighted](../writing/code.md#choosing-how-code-is-highlighted) with |
| `markdown.math` | `mathml` | how [math is written](../writing/math.md#choosing-how-math-is-written) |
| `markdown.mermaid` | `svg` | how [diagrams are drawn](../writing/diagrams.md#choosing-how-diagrams-are-drawn) |
| `plugins` | none | the [plugins](../plugins/using-plugins.md) a site uses, each with its `wasm` file and its settings |
| `versions` | none | the site's [versions](../writing/versions.md), each with its `title`, `url`, and git `ref` |
| `redirects` | none | the site's [redirects](../writing/pages-and-files.md#redirects), each with the URL it leads `to`, and the URLs it's `from`, or a folder for each, like `/tools/*` |
| `serve.host` | `127.0.0.1` | the host `little serve` [serves at](commands.md#choosing-where-to-serve) |
| `serve.port` | `8080` | the port it serves on |
| `error.title`, `error.description` | `Page not found`, and a line that says so | what the [404 page](../getting-started/publishing.md#a-404-page) says |
| `built` | when the build starts | when a theme says the site was [built](template-data.md#dates) |
| `modified` | when the site's newest commit was made | when a theme says the site [last changed](template-data.md#dates) |

## The built-in theme's settings

The built-in theme's
[`little.yml`](https://github.com/withlittle/little/blob/main/theme/little.yml)
goes over little's, with settings of its own, like `name` and `language`,
which
[Configuring](../getting-started/configuring.md#the-built-in-themes-settings)
lists.

## Your own settings

Any other key under `site` reaches the theme as `site.<key>`, for a theme
that shows it, like `announcement: "2.1 is out!"` as `site.announcement`.

## Every page's settings

A key under `page` reaches the theme as `page.<key>`, on every page, as if
each page's [frontmatter](../writing/frontmatter.md#for-every-page) set it.
A page's own frontmatter goes over it, and changes only that page.

little sets these under `page`, and a theme, a site, or a page can change
any of them:

| Key | Default | Chooses… |
| --- | ------- | -------- |
| `outline.enabled` | `true` | whether a page has a [table of contents](../writing/navigation.md#tables-of-contents) |
| `outline.depth.min` | `2`, for `##` | the level of heading it lists from |
| `outline.depth.max` | `6`, for `######`; `3`, for `###`, in the built-in theme | the level it lists down to |
| `breadcrumb.enabled` | `true` | whether it has [breadcrumbs](../writing/navigation.md#breadcrumbs) |
| `previous.enabled`, `next.enabled` | `true` | whether it links to the [pages before and after it](../writing/navigation.md#previous-and-next-pages) |
| `repository.view.enabled`, `repository.edit.enabled` | `true` | whether it links to where [its file](../getting-started/configuring.md#choosing-the-sites-repository) is viewed, and edited, in the site's repository |
| `source.enabled` | `true` | whether [its file](../writing/pages-and-files.md#a-pages-file) is copied beside it |
| `search.enabled` | `false` under a URL in `site.search.exclude`, else `true` | whether [search](../writing/search.md#leaving-pages-out) indexes it |
