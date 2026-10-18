# 0037 — Wrapping long names in the built-in theme

- Status: accepted
- Date: 2026-10-11
- Amends: [0033](./0033-the-built-in-themes-design.md)

## Context
The built-in theme ([0033](./0033-the-built-in-themes-design.md)) writes
each page's title as it is, in its sidebar, its tables of contents, its
breadcrumbs, and its previous and next pages. A browser wraps a line at a
space or a hyphen, but not at `_` or `.`, so a title that's a name, like a
Python module's `enable_iterative_imputer`, or a method's
`ThreadPoolExecutor.submit`, is one long word, which doesn't wrap.

On a site with 138 pages of API reference, from the docstrings plugin
([0031](./0031-a-plugin-for-pythons-docstrings.md)), with modules four
folders deep:
- names ran out of their rows in the sidebar, past the current page's
  highlight, and under the fold buttons of the folders beside them, and the
  sidebar scrolled sideways;
- across the reference, 360 links in the tables of contents beside its
  pages, which are 13rem wide, ran out of them;
- and, on a phone, the breadcrumbs, and the previous and next pages, ran off
  the screen.

The room beside a folder's link for its fold button
([0036](./0036-folding-the-built-in-themes-nav.md)) had never been there:
the nav's rule for its links and labels, `.nav :is(a, .nav-label)`, takes
the specificity of the most specific selector in `:is()`, a class, so its
padding beat that of `.nav a:has(+ details)`, and a long title ran under
the button.

This manual's titles are words, with spaces, which wrap.

## Options considered
- **Where a long name breaks:**
  - nowhere, cut short with an ellipsis: it keeps to one line, but hides
    the end of a name, which is where names that start alike differ, like
    `solver_cpu` and `solver_gpu`, and a tooltip, from `title`, isn't there
    on a touch screen;
  - at a hyphen the browser adds, with `hyphens: auto`: it hyphenates only
    words in its dictionary for the page's language, and a hyphen in a name
    reads as part of it;
  - anywhere, with `overflow-wrap: anywhere`, as Furo has: a name always
    fits, but breaks mid-word, as `enable_iterative_imp` and `uter`;
  - or after each `_` and `.`, with `<wbr>`, as rustdoc does: a name breaks
    between its words, as `enable_iterative_` and `imputer`, and no text is
    added to it.
- **Who marks where it breaks:**
  - little, in each title in `site.nav`: every theme would get markup in
    its data, and the theme owns all the markup
    ([0002](./0002-theme-structure.md));
  - or the theme, as it writes each title.
- **Which titles:**
  - every title: a browser breaks a line at the last place that fits, so a
    title like "Version 2.10.0" could break after "2.";
  - or only names, titles with no spaces: a title with spaces wraps at
    them.
- **More room**, from a wider sidebar, or less indent for each level: it
  fits only names that are a little too long, takes room from the page, and
  does nothing for the table of contents, or for a phone.

## Decision
- The built-in theme writes each title in its sidebar, its tables of
  contents, its breadcrumbs, and its previous and next pages through a
  macro, `breakable`, which puts a `<wbr>` after each `_` and `.` in a title
  with no spaces. The title is still escaped, and one with spaces is
  written as it was.
- Each of them has `overflow-wrap: anywhere`, so a name that's too long,
  even between its `_`s and `.`s, breaks where it has to, rather than run
  out of its box, as does a long word in a title with spaces.
- A page's `<title>`, and a fold button's hidden name, are written as they
  were: they aren't drawn in a box.
- The nav's rule for its links and labels is a list, `.nav a, .nav
  .nav-label`, so a link with a fold button leaves room for it.

## Consequences
- On the site with 138 pages of API reference, no title runs out of its
  box, at 320px, 375px, or 1280px wide, and the sidebar doesn't scroll
  sideways. A name deep in the nav can take two lines, as each level is
  indented as before.
- A `<wbr>` adds no text: a screen reader reads a name, and a reader copies
  it, as it is, and the template still finds `aria-current="page"` in a
  folder's items.
- This manual looks as it did: none of its titles is a name, so its HTML is
  the same, and its pages are, to the pixel, at 1280px and 375px wide.
- A page's `#` heading is written by little, from its Markdown, not by the
  theme, so it has no `<wbr>`, and a long name in it still breaks
  mid-word.
- Both `<wbr>` and `overflow-wrap: anywhere` are in every browser 0033
  supports.
- A theme that builds on the built-in one, with its own `default.html`,
  writes its titles as it did: with the built-in stylesheet, a long name in
  them breaks mid-word, unless it copies `breakable`.
