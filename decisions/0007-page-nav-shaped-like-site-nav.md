# 0007 — A page's table of contents is shaped like the nav

- Status: accepted
- Date: 2026-09-29

## Context
`page.nav`, a page's table of contents, was a flat list of its `##` and `###`
headings, each `{ title, anchor, depth }`. A theme had to indent each heading
by its `depth`, and couldn't list them as nested lists, the way it lists
`site.nav` and `page.children`, without working out where each `##` ends. The
two navs were shaped differently, so a theme couldn't list them with one loop.

## Options considered
- **Flat or nested:** flat, with each heading's level, as little had it and
  as Docusaurus's Markdown loader gives it (its theme nests it), or nested,
  as in MkDocs, Zola, and Hugo's `.Fragments.Headings`.
- **What an item holds:** a shape of its own, like MkDocs'
  `{ title, url, level, children }` or Zola's
  `{ title, id, permalink, level, children }`, or exactly the shape of
  `site.nav`: `{ title, url, children }`. With a level, a table of contents
  isn't shaped like the nav, and a theme can count its depth as it goes.
- **The link:** the heading's `#fragment`, as MkDocs' `url` is, or the page's
  URL with it, as Zola's `permalink` is. A fragment works however the page
  was reached, with or without a trailing slash, and never equals `page.url`,
  so a loop that marks the current page never marks a heading.
- **A `###` with no `##` above it:** at the top, beside the `##`s (MkDocs,
  Zola, Docusaurus), or under an empty heading (Hugo), which a theme lists as
  an empty item.

## Decision
- `page.nav` is a list of `{ title, url, children }`, like `site.nav` and
  `page.children`: `url` is the heading's `#fragment`, and each `###` is in
  the `children` of the `##` above it. As in `site.nav`, a heading without
  `children` leaves them out. It still lists `##` and `###` headings.
- A heading with nothing above it, like a `###` before the first `##`, goes
  at the top.
- The built-in theme lists it with a recursive loop, as it does its sidebar.

## Consequences
- `site.nav`, `page.children`, and `page.nav` share a shape, so one loop or
  macro can list all three.
- A theme that used `anchor` or `depth` must use `url`, and the nesting, or
  `loop.depth`, instead. Only the built-in theme did.
- A heading's level isn't in `page.nav`, so a `###` at the top is listed like
  a `##`, not indented as it was.
- A `nav` in a page's frontmatter still replaces `page.nav` as written
  ([0006](./0006-frontmatter-and-config-win.md)): little doesn't check its
  shape, as it does a `nav` in `little.yml`.
