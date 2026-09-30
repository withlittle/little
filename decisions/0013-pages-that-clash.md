# 0013 — Two files can't be the same page

- Status: accepted
- Date: 2026-10-01

## Context
Each page is built into a folder named for its URL: `guide.md`, at `/guide`,
into `guide/index.html`, and `guide/README.md`, at `/guide/`, into the same
file. A README's name can be in any case, as on GitHub, so `README.md` and
`readme.md` in one folder are the same page too. When a source had two such
files, the one written last silently replaced the other (for `README.md` and
`readme.md`, whichever the disk happened to list last), and nothing told the
author a page was missing.

## Options considered
- **What to do about a clash:** carry on, as before; warn and build one of
  the two, as MkDocs does, and as Hugo does if asked (`--printPathWarnings`);
  or stop the build with an error, as Eleventy and Zola do. A warning scrolls
  by, and the page is still missing from the site.
- **What clashes:** files that would be written to the same file in the
  destination; or URLs that differ only in case too, like `API.md` and
  `api/README.md`, at `/API` and `/api/`, which are one folder on a disk that
  ignores case, as macOS's and Windows' do by default, but two on Linux's.
- **When to check:** once every page is read, before any is rendered or the
  destination touched; or as each page is written.

## Decision
- Two files in the source that would be the same page stop the build, with an
  error that names both:
  `docs/guide.md and docs/guide/README.md would both be the page at /guide/: rename or move one`.
- It's checked before any page is rendered, so the build fails fast, and
  leaves the last one in place, as any failed build does.
- URLs that differ only in case aren't checked.

## Consequences
- A page can't go missing without a word: the author is told which two files
  clash, and renames or moves one.
- A site that built before, with a page quietly lost, doesn't build until one
  of the two is renamed or moved.
- A page beside a folder of the same name, like `guide.md` beside
  `guide/install.md`, is fine; only a `guide/README.md` would clash with it.
- On macOS and Windows, pages whose URLs differ only in case can still
  replace each other, as can a page and a theme's static file whose paths
  differ only in case.
- With `little serve` running, moving a page by saving it in its new place
  before deleting the old one fails the rebuild in between. `little serve`
  keeps serving the last build, and the next change rebuilds it.
