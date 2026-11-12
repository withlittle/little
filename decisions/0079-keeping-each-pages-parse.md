# 0079 — Keeping each page's parse

- Status: accepted
- Date: 2026-11-13
- Amends: [0076](./0076-rebuilding-only-what-differs.md)

## Context
A rebuild in `little serve` reads and parses every page again
([0076](./0076-rebuilding-only-what-differs.md)), as that's where a
change shows. Once it indexes only the pages that differ for search
([0078](./0078-keeping-the-search-index.md)), parsing is most of what's
left: about 0.7 of the 1.1 seconds a save took to reach the browser, on
a site of 2,000 pages, as `scripts/bench.sh` writes it, on an Apple
silicon Mac. Most of that is highlighting the pages' code.

A Markdown page's parse depends on its text, the URL of the folder it's
in, which its links lead from, and the site's settings: its `markdown`,
the levels its table of contents lists, the URL it's under, and the
extensions its plugins read, which a link to a file can lead to a page
by. It doesn't depend on the site's other pages. Two things reach
further: a Mermaid diagram is drawn as of now, and a plugin that renders
code blocks can read the folders it's given.

## Options considered
- **Parse only the files the watcher saw change.** What a file's parse
  depends on besides the file would have to be watched too, and each
  platform reports changes its own way.
- **Keep each file's parse by when it was last changed.** A file's time
  says nothing of the settings, and a version from a git ref is checked
  out anew each build.
- **Keep each parse by a hash of all it's parsed from.** A page whose
  text and settings are the same is the same page, wherever it's from.

## Decision
- A build keeps each Markdown page's parse, by a hash of the page's
  text, as its plugins' `source` hooks leave it, its folder's URL, and
  the settings above. The build after it, in `little serve`, parses a
  page only if it has none by that hash.
- The plugins' `source` and `content` hooks still run on every page, as
  they come before and after the parse.
- A page that names Mermaid is parsed each time, and so is every page of
  a site with a plugin that renders code blocks, and any page a plugin
  reads, like a notebook.
- `little build` has no build before it, so parses every page.

## Consequences
- On the same site, a save to a page's text reloads in 0.5 seconds, not
  1.1, with either nav. A save to the same page took 8.2 seconds, or 3.5
  with `nav_prune`, before 0076.
- A version from a git ref isn't parsed again while its pages and
  settings are the same.
- `little serve` holds each page's parse from build to build: its HTML,
  its outline, and its links, about as much as the site's Markdown twice
  over.
- The hash is 64 bits, as 0076's are, and two pages with one hash would
  show one page's content for the other's, until `little serve` starts
  again.
