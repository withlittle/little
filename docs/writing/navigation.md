---
title: Navigation
description: The sidebar, previous and next pages, breadcrumbs, and tables of contents, and writing the nav yourself.
---

# Navigation

little works out your site's navigation from its pages, for the theme to
show: the nav, which the built-in theme shows in its sidebar, and each
page's previous and next pages, breadcrumbs, and table of contents.

## The nav

little builds the nav from your folders: the home page first, then pages
and folders, sorted by title. A folder with a `README.md` is a section that
links to that page, and a folder without one is a plain label. Either way,
its pages go under it.

Titles sort whatever their case, with numbers by their value, so
`2. Use Postgres` comes before `10. Use Redis`, and a title that starts
with a number comes before one that starts with a letter. To put pages in
an order of your own, number their titles, or
[write the nav yourself](#writing-the-nav-yourself).

## Writing the nav yourself

Set `nav` in `little.yml` to a list of `{ title, url, children }`. It
replaces the nav little works out:

```yaml
# little.yml
nav:
  - { title: Home, url: / }
  - title: Guide
    url: /guide/
    children:
      - { title: Install, url: /guide/install }
```

Write each URL as little builds the page, from the root of the site, like
`/guide/`, not `/guide`, so the theme can mark the current page. little
adds the path of the site's
[`url`](../getting-started/publishing.md#choosing-the-sites-url). An item
without a `url` is a label.

A page you leave out of your nav is still built, and search finds it, but
it has no breadcrumbs, or previous and next pages.

## Previous and next pages

A page's previous and next pages follow the nav from top to bottom: a
section's own page, then the pages in it, then on to the next, skipping
labels. The built-in theme links to them below each page.

## Breadcrumbs

A page's breadcrumbs are the items above it in the nav, from the top down:
the home page, if it's at the top of the nav, then the sections and labels
the page is in, but not the page itself. The home page has none. The
built-in theme shows them above each page's title.

## Tables of contents

A page's headings make its table of contents, with each `###` under the
`##` above it. The built-in theme shows it as "On this page", and lists
`##` and `###` headings. To list `####` headings too, set `toc_max_depth`:

```yaml
# little.yml
toc_max_depth: 4
```

| Key | Default | Lists headings… |
| --- | ------- | --------------- |
| `toc_min_depth` | `2`, for `##` | from this level |
| `toc_max_depth` | `6`, for `######`; `3`, for `###`, in the built-in theme | down to this level |

A page's `#` heading is its title, so tables of contents start at `##`.
With `toc_min_depth: 1`, they list it too, with the other headings under
it. The levels are the same for every page, but a page can turn its table
of contents off, with `toc: false` in its
[frontmatter](frontmatter.md#frontmatter-wins).
