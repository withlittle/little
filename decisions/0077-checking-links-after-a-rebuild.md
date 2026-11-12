# 0077 — Checking links after a rebuild

- Status: accepted
- Date: 2026-11-13
- Amends: [0076](./0076-rebuilding-only-what-differs.md)

## Context
`little serve` checks the links within the site after each build, once
the browser has reloaded. A rebuild writes only the files that differ
([0076](./0076-rebuilding-only-what-differs.md)), but the check read
every page again: lychee for its links, and little for its anchors
([0075](./0075-finding-anchors-in-the-sites-pages.md)). On a site of
2,000 pages, as `scripts/bench.sh` writes it, that took 7 seconds with
the whole nav on every page, and 0.6 with `nav_prune`, on an Apple
silicon Mac, and the next save waited for it.

Nearly all of that is reading. Checking a link within the site, once it's
known, is finding a file.

## Options considered
- **Check only the links an edit could break:** those in the files it
  wrote, and those that lead into them. little would have to keep which
  links lead where, and a link it missed would be a warning it missed.
- **Check on another thread, and drop a check a new save overtakes.** A
  save wouldn't wait, but the warnings would come as late, and every page
  would still be read each time.
- **Keep what's read from each file, and check every link.** Only the
  files a build wrote are read again. A link from a page that's as it was
  to one that's gone is still found, as it's still checked.

## Decision
- `little serve` keeps a checker from build to build. `little build`
  checks from nothing, as it did.
- A build notes each file it wrote. The checker keeps the links lychee
  finds in each HTML file, and each file's anchors. After a rebuild it
  forgets the files that are gone, reads those the build wrote, and
  leaves the rest.
- Every link is then checked, as before: that its file is there, and that
  the file has its anchor.
- A check that fails, or a destination that moves, starts it from
  nothing.
- A link is kept once, and each file holds a number for each of its
  links, so a page with the whole nav of 2,000 pages holds 8 KB.

## Consequences
- On the same site, the check after a save to one page takes 0.11
  seconds with the whole nav on every page, and 0.07 with `nav_prune`.
- The warnings are the same, which a test holds, build after build.
- Holding numbers, not each link's file by name, also makes the check of
  a whole build faster: `little build` checks that site's links in 4
  seconds, not 6.5, with the whole nav.
- A change that rewrites every page, like a page's title in the whole
  nav, has every page read again, as does a site with a plugin that has
  an `html` hook.
- A file edited by hand in the destination isn't read again until a
  build writes it, or `little serve` starts again.
