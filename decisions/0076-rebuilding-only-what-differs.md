# 0076 — Rebuilding only what differs

- Status: accepted
- Date: 2026-11-12

## Context
`little serve` builds the whole site again on each save: it reads and
parses every page, renders each in the theme, indexes them for search,
clears the destination, and writes every file. On a site of 2,000 pages,
as `scripts/bench.sh` writes it, a save to one page took this long to
reach the browser, on an Apple silicon Mac:

| | A save reloads in |
| --- | --- |
| The whole nav on every page | 8.2 s |
| With `nav_prune` | 3.5 s |

Most of it is the theme rendering pages that come out as they were, and
writing them again. Reading and parsing the pages takes about half a
second, and indexing them about one.

## Options considered
- **Build only the files that changed,** from what the watcher says was
  saved. A page's HTML depends on more than its file: the nav lists every
  page's title, a plugin can add pages from any folder it reads, and the
  config and theme reach every page. little would have to know what each
  page depends on, and a mistake would show a stale page.
- **Keep each page's HTML in memory,** and write those that differ. It
  still renders every page, which is most of the time, and holds the
  site's size in memory, 670 MB for that site.
- **Hash what each page is rendered from.** A page whose hash is the last
  build's is as that build wrote it, so needn't be rendered, or written.
  Nothing has to know why a page changed.

## Decision
- `little serve` gives each build after its first the one before it.
  `little build` never has one, so builds as it did.
- Every file is still read, and every page parsed, with the plugins'
  hooks, as that's cheap, and it's where a change shows.
- A page is rendered again unless what the theme renders it from hashes
  as it did: the theme's templates, the URL the site is under, `site`,
  with the page's own nav if it's pruned, and `page`, as the plugins'
  `page` hooks leave it. A build keeps the hash, and the language the
  theme gave the page, for search, not the HTML.
- A site with a plugin that has an `html` hook renders every page each
  time, as the hook can make anything of a page.
- The destination isn't cleared. What the last build wrote, and this one
  doesn't, is deleted, with each folder that leaves empty, before
  anything's written; then a page that was rendered is written, and any
  other file whose contents hash differently, search's among them.
- A file that's gone from the destination is written again. If the
  destination has lost little's marker, or the config has moved it, or
  the last build failed, the build is a whole one.
- Search is indexed in full each time.

## Consequences
- On the same site, a save to a page's text reloads in 2.4 seconds with
  the whole nav on every page, and in 2.3 with `nav_prune`. What's left
  is indexing the site for search, and reading its files.
- A change that reaches every page, like a page's title in the whole nav,
  or a template, renders every page, as before.
- A rebuild leaves the destination as a build would, byte for byte, which
  a test holds, for each kind of nav.
- A file edited by hand in the destination stays as it's edited until its
  page changes, or `little serve` starts again.
- The hashes are 64 bits, from Rust's `DefaultHasher`, and are only
  compared within one run of `little serve`.
