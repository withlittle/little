---
title: Template data
description: The data each of a theme's templates gets, site and page, and the filters little adds.
---

# Template data

Each of a theme's [templates](../themes/making-a-theme.md#templates) gets
the site as `site`, and the page it renders as `page`. A page leaves out
anything it doesn't have, rather than setting it to null, so
`{% if page.description %}` and `{{ page.description | default("…") }}`
both work.

## site

| Variable | Contents |
| -------- | -------- |
| `site` | The site's config, merged, with each of its [settings](settings.md), like `site.url`, and [your own](settings.md#your-own-settings) |
| `site.markdown` | The site's [`markdown` settings](../writing/markdown.md#choosing-markdowns-extensions), like `site.markdown.math`, so a theme can [load a script](../themes/styling-markdown.md#loading-a-script-a-site-needs) only for a site that needs it |
| `site.nav` | The [nav](../writing/navigation.md#the-nav), a list of `{ title, url, children }`, where a label has no `url` |
| `site.version` | On a site with [versions](../writing/versions.md), the `title` of the one being built, like `1.4`. A `version` in `little.yml` replaces it |
| `site.versions` | Each entry in `versions`, as it's written, with its home page's `url`, and `root`, for a [menu of versions](../themes/making-a-theme.md#a-menu-of-versions) |
| `site.year` | The year the site is built in, like `2026`, for a [copyright notice](../themes/making-a-theme.md#a-copyright-notice) |

## page

| Variable | Contents |
| -------- | -------- |
| `page.type` | The page's [type](../themes/making-a-theme.md#templates): `home`, `section`, `page`, or `404` |
| `page.url` | The page's URL, like `/getting-started/install`. Compare it with `item.url` to mark the current nav item |
| `page.title` | The page's [title](../writing/frontmatter.md#a-pages-title) |
| `page.content` | The page's content, as HTML, to write with [`safe`](../themes/making-a-theme.md#templates) |
| `page.toc` | The page's [table of contents](../writing/navigation.md#tables-of-contents), shaped like `site.nav`, with each heading's `#fragment` as its `url` |
| `page.children` | Home and section pages only: what else is in the page's folder, shaped like `site.nav` |
| `page.breadcrumbs` | The page's [breadcrumbs](../writing/navigation.md#breadcrumbs), from the top of `site.nav` down, each `{ title, url }`, where a label has no `url` |
| `page.prev`, `page.next` | The [pages before and after](../writing/navigation.md#previous-and-next-pages) this one in `site.nav`, each `{ title, url }` |
| `page.source_url` | Where to view the page's file in the site's [repository](../getting-started/configuring.md#choosing-the-sites-repository), on github.com or gitlab.com, like `https://github.com/withlittle/little/blob/main/docs/getting-started/install.md`; from a version's `ref`, at its commit |
| `page.edit_url` | Where to edit it there, like `https://github.com/withlittle/little/edit/main/docs/getting-started/install.md`; none from a version's `ref` |
| `page.versions` | Each entry in `site.versions`, with the `url` of this page in that version, else of its home page, for a [menu of versions](../themes/making-a-theme.md#a-menu-of-versions) |
| `page.<key>` | Every other key in the page's [frontmatter](../writing/frontmatter.md), as written, like `page.description` |

A 404 page has just `page.type` and `page.url`, `/404.html`. On a site with
versions, it has `page.versions` too, with each version's home page.

A page's frontmatter has the last word on `page`, as
[Frontmatter wins](../writing/frontmatter.md#frontmatter-wins) explains.

## Filters

little adds two filters to [MiniJinja's](https://docs.rs/minijinja), for
the URLs a theme writes itself, as
[URLs in a theme](../themes/making-a-theme.md#urls-in-a-theme) explains:

| Filter | Gives back | Like |
| ------ | ---------- | ---- |
| `url` | a URL from the root of the site, with the path of the site's `url` before it | `{{ '/assets/style.css' \| url }}` is `/little/assets/style.css` |
| `absolute_url` | a URL with the path, with the site's scheme and host before it | `{{ page.url \| absolute_url }}` is `https://withlittle.github.io/little/getting-started/install` |
