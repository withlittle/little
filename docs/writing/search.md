---
title: Search
description: Search built in, with Pagefind, which indexes each page as it's built, for a search box in readers' browsers.
---

# Search

A site built with the built-in theme is searchable out of the box: the search
box in its header, which ⌘K or Ctrl+K opens too, finds pages, and the
headings within them, as a reader types. Try it on this manual.

- Each build indexes every page into `pagefind/` in `dist/`, and the box
  searches it in the reader's browser, so there's no server or service to set
  up. `little serve` indexes each save. little indexes with
  [Pagefind](https://pagefind.app/), which is built in, so there's nothing
  else to install.
- What's indexed is each page's own content, from its Markdown or notebook,
  under its title, so nothing the theme puts around it, like its header or
  nav, fills the results. The 404 page isn't indexed, nor is an HTML file
  copied from `docs/`, or the words in a [diagram](diagrams.md).
- Words are matched by their stem in the site's
  [`lang`](../getting-started/configuring.md#the-built-in-themes-settings),
  so "installing" finds "install".
- To go without, set `search: false`, and the built-in theme leaves out the
  box too:

  ```yaml
  # little.yml
  search: false
  ```

- A page or file where little writes one of the search's files, like
  `docs/pagefind/pagefind.js`, stops the build. A page at `/pagefind`, from
  `docs/pagefind.md`, is fine, and so is any other file in `docs/pagefind/`.
- `pagefind/` is about 0.7 MB for a site of a few pages, and about 3 MB for
  one of 450. A reader downloads only the parts of it a search needs.
  Pagefind keeps each page's first image for its result, so the first image
  in a notebook is in `pagefind/` too, whole.
- A page can't leave itself out of search yet. Chinese, Japanese, and Thai,
  which don't put spaces between words, search poorly.

A theme without a search box leaves search off: see [A search
box](../themes/making-a-theme.md#a-search-box) to add one.
