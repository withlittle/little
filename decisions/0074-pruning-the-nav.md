# 0074 — Pruning the nav

- Status: accepted
- Date: 2026-11-12
- Amends: [0036](./0036-folding-the-built-in-themes-nav.md)

## Context
Every page gets the whole of `site.nav`, and the built-in theme writes all
of it into each page's sidebar: it folds the folders the page isn't in
([0036](./0036-folding-the-built-in-themes-nav.md)), but still writes
what's in them, which makes a site's HTML grow with the square of its
pages. Two sites of pages of 4 KB of Markdown, in folders of 50 at the top
of the source, built with the built-in theme, on an Apple M2:

| Pages | Built in | Links checked in | Size |
| ----- | -------- | ---------------- | ---- |
| 1,021 | 3.3 s | 3.4 s | 184 MB |
| 5,101 | 55 s | 101 s | 4.1 GB |

A page of the larger is 865 KB, and all but 13 KB of it is its nav.
With a theme that writes no nav, a site like it is built and checked in 11
seconds, into 80 MB: little keeps up with the pages, but not with their
navs. The nav also stands in the way of building only what's changed
([`VISION.md`](../VISION.md)): while each page lists every other, a new
page, or a new title, changes them all.

Material for MkDocs has `navigation.prune`, since 9.2: it writes only the
items a reader sees, and replaces each section that would expand with a
link to the first page in it, or to its index page.

## Options considered
- **Who prunes:** each theme, in its templates, or little, in the nav it
  gives them. A template can leave out what's in a folder unless
  `page.breadcrumbs` has the folder: a few lines in the built-in theme's,
  which every other theme would write again, and which can't tell a
  folder's page from a link to another site. 0036 turned down a nav for
  each page, as "a field in every theme's data for what one theme shows";
  a pruned nav needs no field, and a template that lists `site.nav` lists
  it as it is, since a folder without `children` is already a link.
- **Where a page's nav goes:** in `site.nav`, which themes list already; or
  in a `page.nav` beside it, which each theme would have to list instead,
  and which was the table of contents' name until
  [0008](./0008-page-toc.md).
- **Which folders are pruned:**
  - those the built-in theme folds, each within a top-level one (0036), so
    a sidebar looks as it did, but for the folds: a site of 1,156 pages,
    in folders three deep, goes from 288 MB to 31 MB, but the one of 1,021
    above, whose 20 folders are all at the top, stays 184 MB;
  - or every folder a page isn't in, the top-level ones too, which a
    reader then sees as links: 27 MB and 32 MB.
- **A label,** a folder with no page of its own:
  - kept whole, as it has no page to link to: a site whose folders have no
    `README.md`, as folders of numbered records often haven't, is then
    hardly pruned at all;
  - or a link to the first page in it, as in Material.
- **A folder whose link leads nowhere in the site,** like one to another
  site, in a `nav` in `little.yml`: pruned, so nothing leads to what's in
  it; or kept whole.
- **Its name:** `navigation.prune`, as Material has it, though `nav` is
  taken, by a list, which `prune` can't go in, and `navigation` beside it
  is easy to mix up; or `nav_prune`, flat, like `toc_max_depth`
  ([0009](./0009-toc-depth-in-config.md)).
- **Whether it's on:** for every site, though on a small one a reader
  loses unfolding a folder where they are, for nothing; from some number
  of pages, so a site changes as it grows past it; or off, for a site to
  turn on.

## Decision
- `nav_prune` is `true` or `false`, and anything else stops the build.
  little sets `false`, and a theme or a site can set `true`. It's the
  site's, for each of its versions
  ([0042](./0042-versions-share-the-sites-config-and-theme-files.md)).
- With `nav_prune: true`, each page gets a `site.nav` of its own: the whole
  nav, as the config sets it or little builds it, and as plugins' `site`
  hooks leave it ([0046](./0046-plugin-hooks.md)), but a folder has its
  `children` only for a page that's in it, or that's the folder's own. For
  any other page it's a link, without `children`: to its own `url`, or,
  for a label, to the `url` of the first page in it, in the nav's order.
  Its other keys stay.
- The top of the nav is always whole, and it's all that a page that isn't
  in the nav gets, like the 404 page.
- A folder keeps its `children` on every page if its link leads nowhere in
  the site: its `url` isn't one of the site's pages', or it's a label with
  no page in it. The folders in it are still pruned.
- A page listed twice is in the folder it's last listed in, where its
  breadcrumbs lead ([0010](./0010-breadcrumbs.md)).
- `page.breadcrumbs`, `page.prev`, `page.next`, and `page.children` follow
  the whole nav, as before.
- The built-in theme writes a pruned nav with the markup it has: a folder
  the page isn't in is a link like a page's, with no button to fold it.

## Consequences
- The same two sites, with `nav_prune: true`:

  | Pages | Built in | Links checked in | Size |
  | ----- | -------- | ---------------- | ---- |
  | 1,021 | 2.1 s | 0.5 s | 32 MB |
  | 5,101 | 12 s | 4 s | 219 MB |

  A page of the larger is 37 KB, with 151 links in its nav: the home page,
  the 100 folders at the top, and the 50 pages of its own folder.
- A reader can't unfold a folder they're not in: its link takes them into
  it, where the nav lists what's in it.
- With `nav_prune`, `site.nav` isn't the same on every page, as 0036 had
  it, and a label isn't always one: on a page outside it, it has a `url`.
  A theme that lists the whole site from `site.nav`, as a site map would,
  gets only each page's part of it; the home page's `page.children` still
  holds every page.
- A new page changes the nav only of the pages that list its folder, so
  fewer pages change when one does.
- Limits, for now:
  - a theme can't tell a folder that's a link from a page, to mark it;
  - the pages at the top of a nav, and those in a folder that's kept
    whole, are on every page, so a site with thousands of either is
    hardly pruned;
  - a label's link is to the first page in it even on that page, when the
    page is listed again in another folder.
