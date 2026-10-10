---
title: Navigation
description: The sidebar, previous and next pages, breadcrumbs, and tables of contents, and writing the nav yourself.
---

# Navigation

little works out a site's navigation, for its theme to show: the nav, which
the built-in theme shows in its sidebar, or, on a phone or a tablet, in a
menu opened from its header, each page's previous and next pages, its
breadcrumbs, and its table of contents.

## The nav

The nav is built from the folder structure: the home page first, then pages
and folders sorted by title. A folder with a `README.md` is a linked
section, and one without is a plain label, with its pages under it.

- Titles are sorted whatever their case, and the numbers in them by their
  value, so `2. Use Postgres` comes before `10. Use Redis`, and `Step 9`
  before `Step 10`. A title that starts with a number comes before one that
  starts with a letter.
- To put pages in an order of your own, number their titles, or [write the
  nav yourself](#writing-the-nav-yourself).

The built-in theme shows each top-level folder as a group, and folds up each
folder within one, but those the current page is in. A reader opens a folder
from its label, or, for a folder with a `README.md`, from the button beside
its link. The sidebar scrolls to the current page, if it's out of sight.

## Writing the nav yourself

To write the nav yourself, set `nav` in `little.yml` to a list of
`{ title, url, children }`. It replaces little's. Use each page's URL as
little builds it (`/guide/`, not `/guide`), so the theme can mark the
current page:

```yaml
# little.yml
nav:
  - { title: Home, url: / }
  - title: Guide
    url: /guide/
    children:
      - { title: Install, url: /guide/install }
```

- An item without a `url` is a label, as a folder without a `README.md` is.
- Write each URL from the root of the site, without the path of the site's
  [`url`](../getting-started/publishing.md#choosing-the-sites-url): little
  adds it.
- [Breadcrumbs](#breadcrumbs), and [previous and next
  pages](#previous-and-next-pages), follow it instead, so a page that isn't
  in it has none. It's still built, and search still finds it, but it isn't
  in the nav.
- This manual writes its own, to put its pages in the order a reader needs
  them, rather than by title, in its
  [`little.yml`](https://github.com/withlittle/little/blob/main/little.yml).

## Previous and next pages

Each page's previous and next pages follow the nav, from top to bottom, as
a reader reads it: a section's own page, then the pages in it, then on to
the next. A label, without a page, is passed over. The first page has no
previous page, and the last has no next. The built-in theme links to them
below each page's content.

## Breadcrumbs

A page's breadcrumbs are what's above it in the nav, from the top down: the
home page, if it's at the nav's top, then the sections the page is in, but
not the page itself. The home page has none. The built-in theme shows them
above each page's title.

## Tables of contents

Each page's headings are its table of contents, which the built-in theme
shows as "On this page", with each `###` under the `##` above it: to the
right of the page on a wide screen, marking the section being read, and
above it, folded up, on a narrower one, folding up again once a heading in
it is picked. It lists headings from one level to another, 2 for `##` to 6
for `######`:

| Key | Default | Lists headings… |
| --- | ------- | --------------- |
| `toc_min_depth` | `2`, for `##` | from this level |
| `toc_max_depth` | `6`, for `######`; `3`, for `###`, in the built-in theme | down to this level |

To list the `####` headings too, say, set it in `little.yml`:

```yaml
# little.yml
toc_max_depth: 4
```

- A page's `#` heading is its title, so tables of contents start at `##`.
  With `toc_min_depth: 1`, they list it too, and the other headings under it.
- A theme can set them in its `little.yml`, as the built-in theme does, and
  the site's `little.yml` has the last word. They're the same for every page:
  a page can only turn its table of contents off, with `toc: false` in its
  [frontmatter](frontmatter.md#frontmatter-wins).
- Each must be a level from 1 to 6, and `toc_min_depth` can't be more than
  `toc_max_depth`: otherwise, the build stops and says why.
