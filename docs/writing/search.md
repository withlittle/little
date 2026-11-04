---
title: Search
description: Search built in, with Pagefind, so readers can search a site in their browser.
---

# Search

The built-in theme has a search box in its header. It finds pages, and the
headings in them, as a reader types. Press ⌘K or Ctrl+K to open it. Try it
on this manual.

There's nothing to set up. Each build indexes every page with
[Pagefind](https://pagefind.app/), which is built into little. The index
goes in `dist/pagefind/`, and the search box searches it in the reader's
browser, so there's no server or service either. `little serve` indexes
each save.

To turn search off, set `search: false`. The built-in theme then leaves out
its search box too:

```yaml
# little.yml
search: false
```

## What's indexed

Each page's own content is indexed, under its title. So nothing the theme
puts around it, like its header or nav, fills the results.

- Words match by their stem, in the site's
  [`lang`](../getting-started/configuring.md#the-built-in-themes-settings).
  So "installing" finds "install".
- These aren't indexed: the 404 page, pages you
  [leave out](#leaving-pages-out), HTML files copied from `docs/`, and the
  words in [diagrams](diagrams.md).
- On a site with [versions](versions.md), each version has its own index.
  So a search finds pages in the version the reader is in.
- Chinese, Japanese, and Thai, which don't put spaces between words, search
  poorly.
- `pagefind/` is about 0.7 MB for a site of a few pages, and about 3 MB for
  one of 450. A reader downloads only the parts a search needs. Pagefind
  keeps each page's first image for its result, so the first image in a
  notebook is in `pagefind/` too, whole.
- A page or file where little writes one of the search's files, like
  `docs/pagefind/pagefind.js`, stops the build. A page at `/pagefind`, from
  `docs/pagefind.md`, is fine, and so is any other file in
  `docs/pagefind/`.

A theme without a search box leaves search off. To add one, see
[A search box](../themes/making-a-theme.md#a-search-box).

## Leaving pages out

To leave pages out of search, list their URLs in `search.exclude`. Each URL
leaves out its page, and every page under it:

```yaml
# little.yml
search:
  exclude:
    - /changelog   # the page from changelog.md
    - /archive/    # archive/README.md, and every page in archive/
```

A page left out is still built, and still in the nav. Search just doesn't
find it.

- Write each URL from the root of the site, as
  [`nav`](navigation.md#writing-the-nav-yourself) does.
- `/archive` and `/archive/` are the same. Both leave out `/archive/2019`,
  but not `/archived`.
- `search.exclude` turns search on, as `search: true` does.
- little warns of a URL that matches no page, as it does of a broken link.
  `little build --strict` fails on one.
- On a site with [versions](versions.md), each URL leaves out its pages in
  every version. little warns of it only if no version has a page there.

A page's own `search`, in its
[frontmatter](frontmatter.md#leaving-a-page-out-of-search), has the last
word. `search: false` leaves it out. `search: true` keeps it in, even under
a URL in `exclude`, but doesn't turn search on for a site that has it off.

To index only a few pages, exclude `/`, and give each of them
`search: true`:

```markdown
---
search: true
---
```

Don't leave out every page, or the search box shows an error. Set
`search: false` instead.
