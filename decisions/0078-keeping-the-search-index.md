# 0078 — Keeping the search index

- Status: accepted
- Date: 2026-11-13
- Amends: [0076](./0076-rebuilding-only-what-differs.md)

## Context
A rebuild in `little serve` indexes every page for search again
([0076](./0076-rebuilding-only-what-differs.md)), with Pagefind
([0019](./0019-site-search.md)). On a site of 2,000 pages, as
`scripts/bench.sh` writes it, that was about 1.1 of the 2.3 seconds a
save took to reach the browser, on an Apple silicon Mac. Most of it is
Pagefind reading each page: its words, and their stems. Building the
index from what it's read is quick.

Pagefind keeps what it's read of each page. Given a page again, at the
same URL, it replaces what it read, where it was, and it can build its
index any number of times. It can't forget a page, and reads a new one
after the rest.

## Options considered
- **Index after the browser reloads,** as the links are checked. The
  search box would find the last build's pages for a second, and the work
  would be the same.
- **Keep Pagefind's index, whatever the pages.** A page that's gone
  would still be found, and a new one would be read after the rest, so
  the index's files would differ from a build's.
- **Keep it while the pages are the same.** Only the pages that differ
  are read again, and the files are a build's.

## Decision
- A build keeps what Pagefind read of each version's pages, with a hash
  of each page's title, content, and language. The build after it, in
  `little serve`, takes it.
- If the version indexes the same pages, by their URLs, in the same
  order, Pagefind is given only those whose hash differs, and builds its
  index again.
- For any other pages, one more, one fewer, or one newly left out of
  search, Pagefind starts again, and reads them all.
- A build that fails leaves the next nothing to take, so that starts
  again too. `little build` has no build before it.

## Consequences
- On the same site, a save to a page's text reloads in 1.1 seconds, not
  2.3, with either nav. What's left is parsing the pages.
- The index's files are what a build writes, byte for byte, which the
  tests hold.
- `little serve` holds what Pagefind read of every page, from build to
  build.
- Adding or removing a page indexes them all, as before.
