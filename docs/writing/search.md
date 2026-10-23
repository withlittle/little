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
  nav, fills the results. The 404 page isn't indexed, nor is a page you
  [leave out](#leaving-pages-out), an HTML file copied from `docs/`, or the
  words in a [diagram](diagrams.md).
- Words are matched by their stem in the site's
  [`lang`](../getting-started/configuring.md#the-built-in-themes-settings),
  so "installing" finds "install".
- On a site with [versions](versions.md), each has its own index, in its
  folder, so a search finds the pages of the version the reader's in.
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
- Chinese, Japanese, and Thai, which don't put spaces between words, search
  poorly.

A theme without a search box leaves search off: see [A search
box](../themes/making-a-theme.md#a-search-box) to add one.

## Leaving pages out

To leave pages out of search, list their URLs in `search.exclude`, from the
root of the site, as [`nav`](navigation.md#writing-the-nav-yourself) writes
them. Each leaves out its page, and every page under it:

```yaml
# little.yml
search:
  exclude:
    - /changelog   # the page from changelog.md
    - /archive/    # archive/README.md, and every page in archive/
```

- A page left out is still built, and in the nav: search just doesn't find
  it.
- `/archive` and `/archive/` are the same, and leave out `/archive/2019`,
  but not `/archived`.
- Settings turn search on, as `search: true` does.
- A URL that matches no page is warned of, as a broken link is, so a typo
  doesn't go unnoticed, and `little build --strict` fails on it.
- On a site with [versions](versions.md), it leaves out the pages at its
  URLs in each one, and is warned of only if none has a page there.

A page's own `search`, in its [frontmatter](frontmatter.md#leaving-a-page-out-of-search),
has the last word: `search: false` leaves it out, and `search: true` keeps
it in, even under a URL in `exclude`, though it doesn't turn search on for a
site that has it off. To index only a few pages, exclude `/`, and give each
of them `search: true`:

```markdown
---
search: true
---
```

A site that leaves every page out still has its search box, which shows an
error: set `search: false` instead.
