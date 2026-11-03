# 0005 — Previous and next pages

- Status: accepted
- Date: 2026-09-29

## Context
A reader who finishes a page has to go back to the sidebar to find the next
one. Docs sites usually end each page with links to the pages before and after
it, so a reader can page through the docs in order, but a theme had no way to
know which pages those are.

## Options considered
- **The order:** the sidebar's, from top to bottom and into each section
  (MkDocs, Docusaurus, VitePress, Sphinx, mdBook), or a sort of its own (Hugo:
  by weight, then date, then title). Hugo's docs warn that its order is
  independent of the lists a theme shows, which can surprise. Following the
  sidebar, the links always agree with it.
- **Across sections, or within one:** Hugo offers both, `.Next` and
  `.NextInSection`. The others go across sections, as a reader reading in
  order would.
- **The first and last pages:** the missing link set to null (`None` in
  MkDocs and Sphinx, `nil` in Hugo and Jekyll, `undefined` in Docusaurus and
  VitePress), set to an empty object, or left out. None of them use an empty
  object, which would let `{{ page.next.url }}` print an empty link on the
  last page instead of failing the build.
- **What a link holds:** the whole page, as in MkDocs and Hugo, or just
  `{ title, url }`, like an item in `site.nav`. A link needs no more.
- **Names:** `prev`, as in Sphinx, VitePress, and HTML's `rel="prev"`, or
  `previous`, as in MkDocs, Jekyll, and Docusaurus. `prev` is shorter.

## Decision
- Every page but the 404 page gets `page.prev` and `page.next`: `{ title, url }`
  for the pages before and after it in `site.nav`, read from top to bottom. A
  section's own page comes before what's in it, and a folder without a
  `README.md`, which is a label in the nav, is passed over.
- As in [0002](./0002-theme-structure.md), what a page doesn't have is left
  out: the first page has no `page.prev`, and the last has no `page.next`.
  Themes check with `{% if page.next %}`.
- The built-in theme shows them at the foot of each page.

## Consequences
- The links always agree with the sidebar, and will follow it when the nav
  gets explicit ordering.
- The home page is first in the nav, so it links to the next page. A theme
  that doesn't want that checks `page.type`.
- A template that uses `page.next.url` without checking fails the build on
  the last page, naming the template.
- A page can't opt out, or link somewhere else, as Docusaurus and VitePress
  allow in frontmatter.
