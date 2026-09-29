# 0010 — Breadcrumbs

- Status: accepted
- Date: 2026-09-30

## Context
A reader who lands on a page from a search or a link can see where it is only
in the sidebar, and nothing on the page leads up to the section it's in. Docs
sites usually show a trail of links above a page, its breadcrumbs, from the
home page down to the section the page is in. A theme had no way to know
which pages those are, short of searching `site.nav` in its templates.
[0008](./0008-page-toc.md) named them `page.breadcrumbs`, for what readers
see.

## Options considered
- **What they follow:** the nav, as in MkDocs (`page.ancestors`), Sphinx
  (`parents`, from the toctree), Docusaurus (the sidebar), Antora, and
  Eleventy's navigation plugin, or the folders, as in Hugo (`.Ancestors`) and
  Zola (`page.ancestors`). little builds its nav from the folders, so the two
  agree until a site writes its own `nav`. Following the nav, the trail
  always agrees with the sidebar, and with `page.prev` and `page.next`
  ([0005](./0005-previous-and-next-pages.md)).
- **The home page:** in the list, as in Hugo and Zola, or left to the theme,
  as in MkDocs, Sphinx, Docusaurus, and Antora, whose themes add it.
  `site.nav` lists the home page beside the items at its top, as MkDocs does,
  so a trail that only followed it would never start with the home page.
  Every theme would have to find it, and leave it off the home page's own
  trail.
- **The page itself:** left out, as in MkDocs, Hugo, Zola, Sphinx, and
  Eleventy (unless asked for), or last, as in Docusaurus and Antora. A theme
  already has it, as `page.title` and `page.url`, and usually shows it
  differently: not linked, and marked as the current page.
- **The order:** from the top down, the order a theme shows them in, as in
  Zola, Sphinx, Docusaurus, Antora, and Eleventy, or nearest first, as in
  MkDocs and Hugo, whose themes reverse it.
- **What an item holds:** the whole page, as in Hugo and Zola, or
  `{ title, url }`, like an item in `site.nav`. A crumb needs no more. A
  folder without a `README.md` is a label in the nav, with no page to link
  to: it could be passed over, as `page.prev` and `page.next` pass it over,
  or kept, with no `url`, as `site.nav` keeps it.

## Decision
- Every page but the 404 page gets `page.breadcrumbs`: a list of
  `{ title, url }` for the items above it in `site.nav`, from the top down,
  titled as the nav titles them. A folder without a `README.md` is kept, as a
  label with no `url`.
- The home page, an item at the top of `site.nav` whose `url` is `/`, is
  above the other items at the top, so their trails start with it. Its own
  trail is empty.
- The page itself isn't in the list.
- A page that isn't in `site.nav` has an empty list, as it has no
  `page.prev` or `page.next`. A page listed twice gets the trail to the last
  place it's listed, where its `page.prev` and `page.next` come from too.
- As with everything in `page`, the page's frontmatter has the last word
  ([0006](./0006-frontmatter-and-config-win.md)): `breadcrumbs: false` turns
  them off, and a list of its own replaces them.

## Consequences
- The trail always agrees with the sidebar and with `page.prev` and
  `page.next`, even in a nav written by hand. `page.children` still follows
  the folders, so there, a section's children and the trails into it can
  differ.
- A theme checks `{% if page.breadcrumbs %}` before listing them, as it does
  `page.toc`: the home page has none, and a page can turn them off. A theme
  that lists them without checking fails the build on a page with
  `breadcrumbs: false`, naming the page and the template's line.
- A label, or a link to another site, is in a trail when the nav puts pages
  under it. A theme shows a label as plain text, as its sidebar does.
- A trail starts with the home page only when `site.nav` lists it at its
  top: not when the source has no `README.md`, or when a site's `nav` puts
  the home page under another item, or leaves it out.
