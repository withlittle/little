# 0008 — A page's table of contents is `page.toc`

- Status: accepted
- Date: 2026-09-30

## Context
A page's table of contents was `page.nav`, named for `site.nav`, whose shape
it shares ([0007](./0007-page-nav-shaped-like-site-nav.md)). But a page has
more than one kind of navigation: its table of contents, `page.prev` and
`page.next`, `page.children`, and soon breadcrumbs. Only `page.nav` didn't say
which it was, and a theme author could as well take it for the site's nav as
the page sees it, which is what Antora's `page.navigation` is. little's own
docs had to call it "the table of contents" wherever it came up.

Authors write these names too: every key in a page's frontmatter reaches
`page` ([0006](./0006-frontmatter-and-config-win.md)). So `nav: false` turned
off a page's table of contents, where an author would expect it to change the
sidebar, as `nav_order` and `nav_exclude` do in Just the Docs.

## Options considered
- **`page.nav`**, as it was, to match `site.nav`. But `page.children` shares
  the nav's shape under a name of its own: a shape doesn't need a name.
- **`page.toc`**, as in MkDocs, Zola, Sphinx, Docusaurus, and Starlight.
  MkDocs, like little, calls the site's navigation `nav`. Book tools, like
  mdBook, call a whole book's contents its table of contents, but on `page`,
  `toc` can only be the page's.
- **`page.outline`**, as in VitePress: plain, but fewer tools use it.
- **`page.headings`**, as in Astro: it names the data, not what it's for, and
  sounds like every heading, where little lists only `##` and `###`.
- **A namespace**, like `page.nav.toc` beside `page.nav.prev`: it lengthens
  every template, and every override in frontmatter (`nav: { next: false }`),
  and none of these tools groups them all.

## Decision
- A page's table of contents is `page.toc`, shaped as
  [0007](./0007-page-nav-shaped-like-site-nav.md) says.
- A page's navigation is named for what readers see, since authors write the
  same names in frontmatter: `page.toc`, `page.prev`, `page.next`, and, when
  they come, `page.breadcrumbs`. The site's stays `site.nav`, like the `nav`
  in `little.yml`.

## Consequences
- `toc: false` in a page's frontmatter turns off its table of contents in a
  theme that checks `{% if page.toc %}`, as the built-in theme does. A theme
  that lists it without checking fails the build on that page.
- A table of contents is on unless a page turns it off, so there's no
  `toc: true`. Some Jekyll themes and Quarto use it to turn one on: here it
  replaces the list, and a theme that lists it fails the build, naming the
  page and the template's line.
- A theme that uses `page.nav` gets no table of contents, and no error. Only
  the built-in theme did. little keeps no alias, which would keep `nav` taken
  in frontmatter.
- A `nav` in a page's frontmatter no longer replaces its table of contents:
  it reaches the theme as `page.nav`, like any other key.
