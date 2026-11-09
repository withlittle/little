---
title: Search
description: Search built in, so readers can search a site in their browser.
---

# Search

The built-in theme has a search box in its header, which finds pages, and
the headings in them, as a reader types. Press ⌘K or Ctrl+K to open it,
and try it on this manual.

There's nothing to set up. Each build indexes every page into
`dist/pagefind/`, and the search box searches it in the reader's browser,
so you need no server or service for it. `little serve` indexes each save.

To turn search off, set `search: false`, and the built-in theme leaves out
its search box too:

```yaml
# little.yml
search: false
```

A theme without a search box leaves search off. To add one to yours, see
[A search box](../themes/making-a-theme.md#a-search-box).

## What's indexed

Each page's own content is indexed, under its title, so nothing the theme
puts around it, like its header or nav, fills the results. Words match by
their stem, in each page's language, which the built-in theme takes from
[`lang`](../getting-started/configuring.md#the-built-in-themes-settings),
so "installing" finds "install".

The 404 page, pages you [leave out](#leaving-pages-out), HTML files copied
from `docs/`, and the words in [diagrams](diagrams.md) aren't indexed. On a
site with [versions](versions.md), each version has its own index, so a
search finds pages in the version the reader is in.

## Leaving pages out

To leave pages out of search, list their URLs in `search.exclude`, from the
root of the site. Each leaves out its page, and every page under it:

```yaml
# little.yml
search:
  exclude:
    - /changelog   # the page from changelog.md
    - /archive/    # archive/README.md, and every page in archive/
```

`/archive` and `/archive/` are the same: both leave out `/archive/2019`,
but not `/archived`. A page left out is still built, and still in the nav,
but search doesn't find it.

Setting `search.exclude` turns search on. little warns of a URL in it that
matches no page. On a site with versions, each URL leaves out its pages in
every version, and little warns of one only if no version has a page
there.

A page's own `search`, in its [frontmatter](frontmatter.md), has the last
word: `search: false` leaves it out, and `search: true` keeps it in, even
under a URL in `exclude`, though it doesn't turn search on for a site that
has it off. To index only a few pages, exclude `/`, and give each of them
`search: true`:

```markdown
---
search: true
---
```

Don't leave out every page, or the search box shows an error. Set
`search: false` instead.
