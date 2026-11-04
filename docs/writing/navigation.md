---
title: Navigation
description: The sidebar, previous and next pages, breadcrumbs, and tables of contents, and writing the nav yourself.
---

# Navigation

little works out a site's navigation from its pages, for the theme to
show:

- the nav, which the built-in theme shows in its sidebar, or, on a phone
  or a tablet, in a menu opened from its header
- each page's previous and next pages
- each page's breadcrumbs
- each page's table of contents

## The nav

little builds the nav from your folders: the home page first, then pages
and folders, sorted by title. A folder with a `README.md` is a section that
links to that page. A folder without one is a plain label. Either way, its
pages go under it.

Titles sort whatever their case, and numbers in them sort by their value.
So `2. Use Postgres` comes before `10. Use Redis`, and `Step 9` before
`Step 10`. A title that starts with a number comes before one that starts
with a letter.

To put pages in an order of your own, number their titles, or
[write the nav yourself](#writing-the-nav-yourself).

The built-in theme shows each top-level folder as a group. It folds up each
folder within a group, except the ones the current page is in. A reader
opens a folder from its label, or, for a folder with a `README.md`, from
the button beside its link. The sidebar scrolls to the current page, if
it's out of sight.

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

Use each page's URL as little builds it, like `/guide/`, not `/guide`, so
the theme can mark the current page.

- An item without a `url` is a label, like a folder without a `README.md`.
- Write each URL from the root of the site, without the path of the site's
  [`url`](../getting-started/publishing.md#choosing-the-sites-url). little
  adds it.
- [Breadcrumbs](#breadcrumbs), and
  [previous and next pages](#previous-and-next-pages), follow your nav. So
  a page that isn't in it has none. It's still built, and search still
  finds it, but it isn't in the nav.
- This manual writes its own nav, in its
  [`little.yml`](https://github.com/withlittle/little/blob/main/little.yml),
  to put its pages in the order a reader needs them.

## Previous and next pages

A page's previous and next pages follow the nav from top to bottom, as a
reader reads it: a section's own page, then the pages in it, then on to
the next. Labels are skipped. The first page has no previous page, and the
last has no next. The built-in theme links to them below each page.

## Breadcrumbs

A page's breadcrumbs are the items above it in the nav, from the top down:
the home page, if it's at the top of the nav, then the sections the page
is in, but not the page itself. The home page has none. The built-in theme
shows them above each page's title.

## Tables of contents

A page's headings make its table of contents. The built-in theme shows it
as "On this page", with each `###` under the `##` above it. On a wide
screen, it's to the right of the page, and marks the section being read.
On a narrower one, it's folded up above the page, and folds up again once a
heading in it is picked.

The built-in theme lists `##` and `###` headings. To list `####` headings
too, set `toc_max_depth`:

```yaml
# little.yml
toc_max_depth: 4
```

| Key | Default | Lists headings… |
| --- | ------- | --------------- |
| `toc_min_depth` | `2`, for `##` | from this level |
| `toc_max_depth` | `6`, for `######`; `3`, for `###`, in the built-in theme | down to this level |

- A page's `#` heading is its title, so tables of contents start at `##`.
  With `toc_min_depth: 1`, they list the `#` heading too, and the other
  headings under it.
- They're the same for every page. A page can only turn its table of
  contents off, with `toc: false` in its
  [frontmatter](frontmatter.md#frontmatter-wins).
- A theme can set them in its `little.yml`, as the built-in theme does.
  The site's `little.yml` has the last word.
- Each must be a level from 1 to 6, and `toc_min_depth` can't be more than
  `toc_max_depth`. Otherwise, the build stops, and says why.
- A page with no headings to list, or with `toc: false`, has no table of
  contents. On a wide screen, the built-in theme leaves the room to the
  right of the page empty, so the text doesn't move as a reader goes from
  page to page.
