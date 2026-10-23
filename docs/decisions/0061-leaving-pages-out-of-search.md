# 0061 — Leaving pages out of search

- Status: accepted
- Date: 2026-10-23
- Amends: [0019](./0019-site-search.md),
  [0053](./0053-decisions-in-the-manual.md)

## Context
Since [0019](./0019-site-search.md), search indexes every page but the 404
page, and a page can't leave itself out. Some pages aren't ones a reader
searches for, and crowd out those they are: a changelog, which names every
feature; an archive of old posts; a folder of a hundred pages written for
another tool; or this manual's design decisions, 62 of its 110 pages, each
of which says what was decided when it was made, though a later one may
have changed it ([0053](./0053-decisions-in-the-manual.md)). Pagefind
leaves out what a theme marks, with attributes like `data-pagefind-ignore`,
or selectors in its config, but little indexes each page's own content, not
the theme's markup around it ([0019](./0019-site-search.md)), so a theme
has nothing to mark, and a selector nothing to match.

## Options considered
- **Pagefind's settings in `little.yml`,** like `root_selector` and
  `exclude_selectors`: they select within the theme's markup, which isn't
  indexed; they tie each site to Pagefind, which 0019 kept out of
  little's settings; Pagefind panics on a selector it can't parse; and in
  YAML, `- #pagination` is a comment.
- **Frontmatter alone:** `search: false` on each page, where a page's own
  settings go ([0006](./0006-frontmatter-and-config-win.md)), but a folder
  of a hundred pages needs a hundred, and a page a plugin adds has no file
  to write it in.
- **What `little.yml` lists:** files in the source, like `other/notes.md`,
  which a page a plugin adds hasn't, and a version from a git ref may have
  elsewhere; globs, for what a list of folders already covers; or URLs,
  from the root of the site, as `nav` writes them, which every page has,
  whatever made it, and which a reader sees.
- **A URL that matches no page:** ignored, so a typo, or a page that's
  moved, is quietly searched again; an error, so deleting a page breaks the
  build; or a warning, as a broken link is
  ([0016](./0016-checking-links.md)), which `--strict` fails on.
- **Which wins:** the site's list, or the page's own `search`, as
  frontmatter has the last word on a page
  ([0006](./0006-frontmatter-and-config-win.md)).

## Decision
- `search` is `true`, `false`, or its settings, which turn it on. Its one
  setting is `exclude`, a list of URLs from the root of the site, as `nav`
  writes them, without the path of its `url`:

  ```yaml
  search:
    exclude:
      - /changelog
      - /archive/
  ```

- Each URL leaves out its page, and every page under it: `/archive` and
  `/archive/` are the same, and leave out `/archive/2019`, but not
  `/archived`, and `/` leaves out every page. A page left out is still
  built, and in the nav.
- A page's own `search`, `true` or `false`, in its frontmatter, or from a
  plugin's `page` hook, has the last word on it: `false` leaves it out, and
  `true` keeps it in, under a URL in `exclude`, so `exclude: [/]` and
  `search: true` on a few pages indexes only those. It doesn't turn search
  on for a site that has it off, and anything else stops the build.
- A URL in `exclude` that matches no page, in any of the site's
  [versions](./0040-versions-from-git-refs.md), is warned of, and
  `--strict` fails on it. On a site with versions, it leaves out the pages
  at its URLs in each.
- `search` cascades as any setting does: a site's `exclude` replaces its
  theme's, and `true` or `false` replaces both. Settings left empty, like
  `{}`, stop the build, since a theme reads them as off; settings with
  `exclude` are truthy, so `{% if site.search %}` still shows the box.
- A URL in `exclude` that doesn't start with `/`, like `other/notes.md`, or
  a setting `search` hasn't, stops the build, saying what it should be.
- The manual leaves its records, and their list, out of search, with
  `exclude: [/decisions/]`, so a search finds the pages that say how little
  works now; the list is still in the nav, under Reference, and pages still
  link to the records.

## Consequences
- A site leaves out a folder of pages with a line in `little.yml`, and a
  page leaves itself out with a line of frontmatter, whatever made it, in
  every version, with nothing of Pagefind's in its config.
- `site.search` can be settings, not `true`, so a theme that tests
  `site.search == true` hides its box on a site that leaves pages out; one
  that tests `{% if site.search %}`, as the manual shows, doesn't. Nothing
  the manual documents breaks.
- Search in the manual no longer finds its records, amended or not, as
  [0053](./0053-decisions-in-the-manual.md) had it: a reader reaches them
  from their list, in the nav, or a page that links to one.
- Limits, for now:
  - `exclude` has no globs, and can't leave out part of a page;
  - a site that leaves every page out still writes `pagefind/`, with no
    index, and its search box shows an error, so `search: false` is the way
    to go without;
  - a URL in `exclude` whose pages all keep themselves in isn't warned of.
