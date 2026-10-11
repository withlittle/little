# 0038 — The built-in theme's page and tables

- Status: accepted
- Date: 2026-10-11
- Amends: [0033](./0033-the-built-in-themes-design.md),
  [0034](./0034-the-built-in-themes-script.md)

## Context
The built-in theme ([0033](./0033-the-built-in-themes-design.md)) centres a
page's text, at most 46rem wide, in `main`. From 75rem, `main` gave a page
with a table of contents a second column for it, 13rem wide, after a gap of
3rem, and a page without one only the first. So the text of a page without
a table of contents, one with no headings to list, or with `toc: false`,
was centred in all of `main`, 8rem to the right of where it was on a page
with one, and it jumped from one page to the next. On a site of 157 pages,
47 have no table of contents, 44 of them pages of API reference from the
docstrings plugin ([0031](./0031-a-plugin-for-pythons-docstrings.md)): at
1440px wide, a package's page started at 488px, and a guide's at 360px.

A table was as wide as its columns, as on GitHub: `display: block`, with
`width: max-content` and `max-width: 100%`, so it scrolled within the page
when it was too wide for it. So a table of a few short columns sat at the
left of the page, its frame ending short of the text's right edge, and
tables one after another were each a different width.

CSS can't give a table both: one that's `display: table` fills a width, but
can't scroll, and one that's `display: block` scrolls, but its cells are
only as wide as their content, as the browser puts them in a table of their
own, inside it.

## Options considered
- **Where a page without a table of contents goes:**
  - centred in `main`, as it was: it uses the room there is, but moves from
    one page to the next;
  - or where it would be with one, beside an empty column: it doesn't move,
    and leaves the same room on its right as a page with one.
- **How wide a table is:**
  - as wide as its columns, as it was, and as on GitHub;
  - as wide as the page, with CSS alone, `display: table` and `width: 100%`:
    its columns share the page's width, and their text wraps to fit, but a
    table that still can't fit, like one with long names in code, runs off
    the page, which then scrolls sideways on a phone;
  - a frame as wide as the page, `display: block` and `width: 100%`, around
    columns as wide as they are: it scrolls, but its rows' lines, and its
    headings' background, stop short of the frame's right edge;
  - or as wide as the page, in a box that scrolls it, which needs a `<div>`
    around each table.
- **Who puts the `<div>` around it:**
  - little, in the HTML it writes: every theme would get it, wanted or not,
    and the theme owns all the markup ([0002](./0002-theme-structure.md));
  - the template, by replacing `<table>` and `</table>` in `page.content`:
    a notebook's dataframe, or a table in HTML, has a `</table>` that would
    close a `<div>` that was never opened;
  - or the theme's script, as Material for MkDocs' does, and as the built-in
    theme's already puts each code block in a `<div class="code-block">`
    for its copy button ([0034](./0034-the-built-in-themes-script.md)).

## Decision
- **The page:** from 75rem, on a site with a nav, `main` has the column for
  the table of contents on every page, empty on a page without one, so the
  page's text starts in the same place on each. A site of one page has no
  nav ([0033](./0033-the-built-in-themes-design.md)), and no other page to
  move from, so its page is centred, as it was.
- **Tables:** where the theme's script runs, a table in a page's content is
  as wide as the page, and the script puts each in a box that scrolls it:
  - The lines at the top of `<head>` add `js` to `<html>` before the page
    is drawn ([0034](./0034-the-built-in-themes-script.md)), so
    `.js .content table` is `display: table` and `width: 100%` from the
    first paint.
  - Once the page has loaded, the script puts each table in a
    `<div class="table-scroll">`, which scrolls sideways when the table is
    too wide for it, and takes over the table's frame, its border and
    rounded corners. So a table is the same size before and after, and
    nothing moves.
  - A `display: table` rounds its corners only with
    `border-collapse: separate` and `border-spacing: 0`, as collapsed
    borders can't be rounded, and `overflow: hidden` clips its headings'
    background to them.
  - A notebook's dataframe, a `table.dataframe`, stays as wide as its
    columns, and isn't put in a box, as the built-in theme shows pandas'
    tables as Jupyter does ([0024](./0024-jupyter-notebooks.md)).
  - Without the script, a table is as wide as its columns, and scrolls
    itself, as it did.

## Consequences
- On the site of 157 pages, every page's text starts at 360px, at 1440px
  wide, whether it has a table of contents or not, and so does this
  manual's 404 page. Below 75rem, where the table of contents is above the
  page, nothing changes.
- The site's 1,044 tables, on 124 pages, are as wide as the page, and the
  10 too wide for it scroll within their frames. The 1,034 that fit are the
  same size before the script runs as after, so nothing shifts. This
  manual's tables are as wide as its pages, and its notebook's dataframe as
  wide as its columns.
- Until the script runs, a table too wide for the page runs off it, as a
  `display: table` can't scroll, and on a phone the page scrolls sideways.
  The script is deferred, so that's until the page has been read and the
  script has loaded. If `assets/script.js` doesn't load, a table too wide
  for the page runs off it for good, as the toggle shows, but does nothing.
- A table in HTML is a table too: one made narrower than the page by its
  own `style` sits in a frame as wide as the page, and a table in another
  table gets a box of its own.
- A theme that builds on the built-in one meets each table in a
  `<div class="table-scroll">` once the script has run, so
  `.content > table` no longer matches it. One that replaces
  `assets/script.js` keeps `js` on `<html>`, from `default.html`, so its
  tables are as wide as the page, but one too wide for it runs off it,
  unless its script puts each in a `.table-scroll` too.
