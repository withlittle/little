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
| `site` | The site's settings, merged: each of [little's](settings.md#littles-settings), like `site.url`, and [your own](settings.md#your-own-settings) |
| `site.markdown` | The site's [`markdown` settings](../writing/markdown.md#choosing-markdowns-extensions), like `site.markdown.math`, so a theme can [load a script](../themes/styling-markdown.md#loading-a-script-a-site-needs) only for a site that needs it |
| `site.navigation.items` | The [nav](../writing/navigation.md#the-nav), a list of `{ title, url, children }`, where a label has no `url`. With [`navigation.prune`](../writing/navigation.md#pruning-the-nav), each page gets its own |
| `site.repository` | The site's [repository](../getting-started/configuring.md#choosing-the-sites-repository): its `url` and `branch`, and its `owner` and `name`, like `withlittle` and `little`, which little reads from its `url` |
| `site.search.enabled` | Whether the site's [indexed for search](../writing/search.md), so a theme shows its search box |
| `site.version` | On a site with [versions](../writing/versions.md), the `title` of the one being built, like `1.4`. A `version` under `site` in `little.yml` replaces it |
| `site.versions` | Each entry in `versions`, as it's written, with its home page's `url`, and `root`, for a [menu of versions](../themes/making-a-theme.md#a-menu-of-versions) |
| `site.built` | When the site was [built](#dates), like `2026-11-19T02:37:42Z`, for a [copyright notice](../themes/making-a-theme.md#a-copyright-notice) |
| `site.modified` | When the site [last changed](#dates), as git has it |

## page

| Variable | Contents |
| -------- | -------- |
| `page.type` | The page's [type](../themes/making-a-theme.md#templates): `home`, `section`, `page`, `404`, or `redirect` |
| `page.url` | The page's URL, like `/getting-started/install`. Compare it with `item.url` to mark the current nav item |
| `page.title` | The page's [title](../writing/frontmatter.md#a-pages-title) |
| `page.content` | The page's content, as HTML, to write with [`safe`](../themes/making-a-theme.md#templates) |
| `page.created`, `page.modified` | When the page was [created, and when it last changed](#dates), as git has them, for a theme to [show](../themes/making-a-theme.md#when-a-page-last-changed) |
| `page.children` | Home and section pages only: what else is in the page's folder, shaped like the nav |
| `page.outline.items` | The page's [table of contents](../writing/navigation.md#tables-of-contents), shaped like the nav, with each heading's `#fragment` as its `url` |
| `page.outline.depth` | The levels of heading it lists, from `min` to `max` |
| `page.breadcrumb.items` | The page's [breadcrumbs](../writing/navigation.md#breadcrumbs), from the top of the nav down, each `{ title, url }`, where a label has no `url` |
| `page.previous`, `page.next` | The [pages before and after](../writing/navigation.md#previous-and-next-pages) this one in the nav, each `{ title, url }`. The first page has no `page.previous`, and the last no `page.next` |
| `page.repository.view.url` | Where to view the page's file in the site's [repository](../getting-started/configuring.md#choosing-the-sites-repository), on github.com or gitlab.com, like `https://github.com/withlittle/little/blob/main/docs/getting-started/install.md`; from a version's `ref`, at its commit |
| `page.repository.edit.url` | Where to edit it there, like `https://github.com/withlittle/little/edit/main/docs/getting-started/install.md`; none from a version's `ref` |
| `page.source.url` | Where [the page's file](../writing/pages-and-files.md#a-pages-file) is copied to, like `/guide/install.md`, for a theme to [link to](../themes/making-a-theme.md#a-pages-file); none for a page a plugin adds, unless its plugin names one |
| `page.search.enabled` | Whether [search](../writing/search.md#leaving-pages-out) indexes the page, on a site with search on: `false` for one that's left out |
| `page.versions` | Each entry in `site.versions`, with the `url` of this page in that version, else of its home page, for a [menu of versions](../themes/making-a-theme.md#a-menu-of-versions) |
| `page.redirect.url`, `page.redirect.title` | [Redirects](../themes/making-a-theme.md#redirects) only: where it leads, and the title of the page there, if it's one of the site's |
| `page.<key>` | Every other key in the page's [frontmatter](../writing/frontmatter.md), or under [`page`](../writing/frontmatter.md#for-every-page) in a `little.yml`, as written, like `page.description` |

`page.outline`, `page.breadcrumb`, `page.repository`'s `view` and `edit`,
`page.source`, and `page.search` are on every page, so
`{% if page.outline.items %}` works anywhere. A page that
[turns one off](../writing/frontmatter.md#frontmatter-wins), with its
`enabled`, has no `items` in it, or no URL, and no `page.previous` or
`page.next`, so a theme shows what it's given.

A 404 page has `page.type`, `page.url`, `/404.html`, and what's under `page`
in a `little.yml`, with nothing in its `items`. Its `page.title` and
`page.description` are
[the site's to set](../getting-started/publishing.md#a-404-page). On a site
with versions, it has `page.versions` too, with each version's home page.

A [redirect](../themes/making-a-theme.md#redirects) has what a 404 page
has, but for a title and a description: its `page.url` is the URL it's
from, and its `page.redirect` is where it leads.

A page's frontmatter has the last word on `page`, as
[Frontmatter wins](../writing/frontmatter.md#frontmatter-wins) explains.

## Dates

Each date is a time in UTC, written like `2026-11-19T02:37:42Z`, which a
`<time>`'s `datetime` takes as it is, and the
[`dateformat` filter](#filters) writes as you choose.

| Variable | When… |
| -------- | ----- |
| `site.built` | the build started, or `little serve` did |
| `site.modified` | the commit the site is built from was made |
| `page.created` | the commit that added the page's file was made |
| `page.modified` | the newest commit to change the page's file was made |

`site.built` is always there. With
[`SOURCE_DATE_EPOCH`](https://reproducible-builds.org/specs/source-date-epoch/)
set, for a reproducible build, it's that time instead.

The other three come from git, so every build of a commit has the same
dates, and changes that aren't committed move none of them. A file that's
renamed keeps the date it was created, where git finds it's the same file,
and a version from a [`ref`](../writing/versions.md) has its pages' dates
from its ref. little leaves out a date git can't give:

- all three, for a site that isn't in a git repository;
- a page's two, until its file is committed, and for a page a plugin adds;
- every page's two, in a shallow clone, as CI often makes: fetch the
  repository's whole history for them.

A `created` or `modified` in a page's
[frontmatter](../writing/frontmatter.md#a-pages-dates) replaces the page's,
and a `built` or `modified` under `site` in `little.yml` the site's.

## Filters

little adds three filters to [MiniJinja's](https://docs.rs/minijinja): two
for the URLs a theme writes itself, as
[URLs in a theme](../themes/making-a-theme.md#urls-in-a-theme) explains,
and one for [dates](#dates):

| Filter | Gives back | Like |
| ------ | ---------- | ---- |
| `url` | a URL from the root of the site, with the path of the site's `url` before it | `{{ '/assets/style.css' \| url }}` is `/little/assets/style.css` |
| `absolute_url` | a URL with the path, with the site's scheme and host before it | `{{ page.url \| absolute_url }}` is `https://withlittle.github.io/little/getting-started/install` |
| `dateformat` | a date, written as its `format` says, in [`strftime`'s codes](https://man7.org/linux/man-pages/man3/strftime.3.html) | `{{ page.modified \| dateformat(format="%-d %B %Y") }}` is `19 November 2026` |

`dateformat` reads a date alone too, like `2026-11-19`, as a page's
frontmatter might set one, and takes it as midnight, UTC. It names months
and days in English, and writes nothing for a date that isn't there.
