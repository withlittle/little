# 0036 — Folding the built-in theme's nav

- Status: accepted
- Date: 2026-10-10
- Amends: [0033](./0033-the-built-in-themes-design.md),
  [0034](./0034-the-built-in-themes-script.md)

## Context
The built-in theme ([0033](./0033-the-built-in-themes-design.md)) shows all
of `site.nav` in its sidebar, every folder open, at every depth. That suits
this manual, of 25 pages in five groups, but a site of 200 pages, in
folders three deep, has a sidebar 200 links long: a reader scrolls past the
folders they don't want to find the one they do, and the current page's
link is often below the bottom of the sidebar, so they can't see where they
are.

The docs themes 0033 learned from fold their navs: Starlight, Furo, and
Docusaurus fold each folder, but those the current page is in. Starlight
folds one with `<details>`, Furo with a checkbox and its label, and
Docusaurus with a script.

`site.nav` is `{ title, url, children }`, the same on every page
([0007](./0007-page-nav-shaped-like-site-nav.md)), with no mark of which
folders the current page is in, and the theme owns all the markup
([0002](./0002-theme-structure.md)).

## Options considered
- **How a folder folds:**
  - with a script, which hides each folder's pages: nothing folds without
    it, and the sidebar jumps as it runs;
  - with a checkbox and its label, as Furo has: no script, but a screen
    reader reads a checkbox, not a button that's expanded or collapsed;
  - or with `<details>` and `<summary>`, as Starlight has: the browser opens
    and closes it, from the mouse or the keyboard, with no script, and a
    screen reader reads its `<summary>` as a button, expanded or collapsed.
- **Which folders fold:**
  - every folder, the top-level ones too: a reader opens a group to see
    what's in it, and this manual's groups, Getting started to Reference,
    would hide all its pages;
  - only those the current page is in, or beside: the sidebar would still
    list every folder above it, open;
  - or each folder within a top-level one: the top level is as before, an
    outline of the site, and the rest folds.
- **Knowing which folders the current page is in:**
  - little marking each item in `site.nav` the current page is in, with
    something like `active`: a nav for each page, and a field in every
    theme's data for what one theme shows;
  - matching `page.breadcrumbs`, the folders above the page
    ([0010](./0010-breadcrumbs.md)): a folder without a `README.md` has no
    URL to match, and two can have the same title;
  - or the template finding it in what it writes: it marks the current
    page's link with `aria-current="page"`, so a folder holds the current
    page if that's in the HTML of its items.
- **A folder with a page of its own**, from its `README.md`:
  - its link inside its `<summary>`: a link inside a button, which screen
    readers handle badly, and which, but for a script, opens the folder as
    well as following the link;
  - or its link as before, with a `<summary>` beside it, a button that
    folds it, as Furo and Docusaurus have: the link goes to the page, and
    the button opens the folder.

## Decision
- In the built-in theme's sidebar, each top-level item is as it was: a
  link, or a label, with its items under it. Each folder within one, at
  every depth, is a `<details>`, folded, but open if the current page is in
  it, or is its own page.
  - A folder without a page of its own folds from its label, its
    `<summary>`, which spans its row, with a chevron at its end.
  - A folder with one has its link, as before, and a `<summary>` at the end
    of its row, 2rem wide, showing only the chevron, with the folder's
    title, visually hidden, to name it to a screen reader: so it reads a
    link, then a button of the same name, expanded or collapsed.
  - The chevron points right when the folder's folded, and down when it's
    open, turning between them unless the reader prefers reduced motion.
- The template writes each folder's items before the folder, with
  `loop(item.children)`, into a variable, and opens the folder if
  `aria-current="page"` is in it. A title can't put it there, as titles are
  escaped, and little's data doesn't change.
- The script, if the current page's link is out of sight, scrolls the
  sidebar to put it a third of the way down: as the page opens, and as the
  drawer does. It scrolls only the sidebar, not the page, as
  `scrollIntoView()` would.
- Which folders a reader opens or closes isn't remembered: each page opens
  those it's in, and folds the rest.

## Consequences
- A large site's sidebar is as long as its top level, and the folders the
  current page is in: about 40 lines, on the home page of the site of 200
  pages that prompted this. The drawer, on a phone, is the same nav, folded
  the same way.
- A site with no folder within a top-level one, like this manual, looks as
  it did.
- Without the script, the folders fold and open, but the sidebar doesn't
  scroll to the current page.
- A folder a reader opens folds again on the next page, if the page isn't in
  it. Remembering it would need storage, and a script that runs before the
  nav is drawn, so it doesn't jump.
- A theme that builds on the built-in one, and styles `.nav`, meets a
  `<details>` around each folder within a top-level one.
