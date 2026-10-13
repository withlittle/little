---
title: Template data
description: The data each of a theme's templates gets, site and page, and the filters little adds.
---

# Template data

Each of a theme's [templates](../themes/making-a-theme.md#templates) gets
the site as `site`, and the page it renders as `page`. A page leaves out
anything it doesn't have, rather than setting it to null, so
`{% if page.description %}` and `{{ page.description | default("…") }}` both
work.

## site

| Variable | Contents |
| -------- | -------- |
| `site` | The site's config, merged: little's, the theme's, and the site's, like `site.name`, `site.url`, `site.repo`, and `site.search`, and any keys you add, like `site.announcement`. [Settings](settings.md) lists them |
| `site.markdown` | The [extensions to Markdown](../writing/markdown.md#choosing-markdowns-extensions) the site's pages are written with, like `site.markdown.math`, so a theme can load a script only for a site that needs it |
| `site.nav` | The [nav](../writing/navigation.md#the-nav) tree: a list of `{ title, url, children }`; a folder without a `README.md` has no `url`. A `nav` in `little.yml` replaces it |
| `site.version` | On a site with [versions](../writing/versions.md), the `title` of the one being built, like `1.4`. A `version` in `little.yml` replaces it |
| `site.versions` | On a site with [versions](../writing/versions.md), each entry in `versions`, as it's written, like `{ title, url, ref }`, but with the `url` of its version's home page, with the path of the site's `url`, and `root`, true for the version at `/` and false for the others |
| `site.year` | The year the site is built in, in UTC, like `2026`, for a [copyright notice](../themes/making-a-theme.md#a-copyright-notice). A `year` in `little.yml` replaces it |

## page

| Variable | Contents |
| -------- | -------- |
| `page.type` | The page's type: `home`, `section`, `page`, or `404` |
| `page.url` | The page's URL, like `/getting-started/install`; compare it with `item.url` to mark the current nav item |
| `page.title` | The page's [title](../writing/frontmatter.md#a-pages-title) |
| `page.content` | The rendered Markdown as HTML; output it with the `safe` filter |
| `page.toc` | The page's [table of contents](../writing/navigation.md#tables-of-contents) (its headings from `toc_min_depth` to `toc_max_depth`), shaped like `site.nav`: a list of `{ title, url, children }`, where `url` is the heading's `#fragment` and each heading is a child of the nearest one above it with fewer `#`s, like a `###` of the `##` above it |
| `page.children` | Home and section pages only: what else is in the page's folder, shaped like `site.nav` |
| `page.breadcrumbs` | What's above this page in `site.nav`, from the top down: a list of `{ title, url }`, where a folder without a `README.md` has no `url`. It starts with the home page, if `site.nav` lists it at its top, and leaves out this page. The home page, and a page that isn't in `site.nav`, have an empty list. See [Breadcrumbs](../writing/navigation.md#breadcrumbs) |
| `page.prev` | The page before this one in `site.nav`, read from top to bottom: `{ title, url }`. The first page has none. See [Previous and next pages](../writing/navigation.md#previous-and-next-pages) |
| `page.next` | The page after this one in `site.nav`: `{ title, url }`. The last page has none |
| `page.source_url` | Where to view the page's file in the site's [repository](../getting-started/configuring.md#choosing-the-sites-repository), if it's on GitHub or GitLab, like `https://github.com/withlittle/little/blob/main/docs/getting-started/install.md`. On a page from a [version's](../writing/versions.md#whats-in-a-version) `ref`, it's at the ref's commit |
| `page.edit_url` | Where to edit the page's file there, like `https://github.com/withlittle/little/edit/main/docs/getting-started/install.md`. A page from a version's `ref` has none, as a tag can't be edited |
| `page.versions` | On a site with [versions](../writing/versions.md), each entry in `versions`, like `site.versions`, but with the `url` of this page in that version, if it has a page at the same URL from its root, or else of its home page, for a [menu of versions](../themes/making-a-theme.md#a-menu-of-versions) |
| `page.<key>` | Every other key in the page's frontmatter, as written, like `page.description` |

A 404 page has just `page.type` and `page.url` (`/404.html`), and, on a site
with versions, `page.versions`, with each version's home page.

A page's frontmatter has the last word on `page`, even over what little
works out, so `next: false` removes the link to the next page, and
`toc: false` the table of contents. It changes only what the theme sees:
`url` doesn't move the page, and `type` doesn't choose its template. See
[Frontmatter](../writing/frontmatter.md#frontmatter-wins).

## Filters

Templates are [MiniJinja](https://docs.rs/minijinja), with its filters, and
two of little's, for the URLs a theme writes itself, as [URLs in a
theme](../themes/making-a-theme.md#urls-in-a-theme) explains:

| Filter | Gives back | Like |
| ------ | ---------- | ---- |
| `url` | a URL from the root of the site, with the path of the site's `url` before it, and, on a site with [versions](../writing/versions.md#whats-in-a-version), the version's, but for the theme's static files, which every version shares | `{{ '/assets/style.css' \| url }}` is `/little/assets/style.css` |
| `absolute_url` | a URL with the path, with the site's scheme and host before it | `{{ page.url \| absolute_url }}` is `https://withlittle.github.io/little/getting-started/install` |
