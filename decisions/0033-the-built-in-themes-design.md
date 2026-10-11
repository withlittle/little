# 0033 — The built-in theme's design

- Status: accepted; amended by [0034](./0034-the-built-in-themes-script.md),
  whose script lets a reader choose light or dark, closes the drawer when
  the window widens, and folds the table of contents above the page once a
  link in it is followed, and by
  [0036](./0036-folding-the-built-in-themes-nav.md), which folds each
  folder in the nav within a top-level one, and by
  [0037](./0037-wrapping-long-names-in-the-built-in-theme.md), which wraps
  a long name in the nav after each `_` and `.`, and by
  [0038](./0038-the-built-in-themes-page-and-tables.md), which keeps a page
  without a table of contents in place, and makes tables as wide as the
  page
- Date: 2026-10-10
- Amends: [0023](./0023-highlighting-code.md),
  [0024](./0024-jupyter-notebooks.md)

## Context
Slice 3 of little's [roadmap](../VISION.md) asks for "a polished,
responsive, accessible default theme". The built-in theme is what every new
site looks like, and what this manual looks like, but it began as a stand-in,
to see little's features while they were built:
- it was light only;
- it had one breakpoint, at 720px, below which the nav and the table of
  contents stacked above the page, so a reader on a phone scrolled past a
  link to every page to reach the one they'd opened;
- it had no skip link, and no focus ring of its own;
- its faintest grey, for the nav's labels, the breadcrumbs, and the footer,
  was 3.2:1 on white, under the 4.5:1 WCAG's AA asks of text;
- its colours were written into each rule, so changing one meant finding
  each.

The theme owns all the markup ([0002](./0002-theme-structure.md)), and
little needs no Node, so its styles ship as CSS little copies, or Sass
little compiles ([0018](./0018-sass-in-themes.md)). It's also a base: a
theme that builds on it ([0003](./0003-choosing-and-layering-themes.md))
should change its look without fighting its selectors.

The docs themes known for reading well, Starlight, Furo, VitePress,
GOV.UK's, and Material for MkDocs, share a shape: a header with search, the
nav in a sidebar that's a drawer on a phone, the page's table of contents
beside it on a wide screen, and colours that follow the reader's system.

## Options considered
- **Tailwind**, through its standalone CLI, which needs no Node: utility
  classes in the templates, compiled to the CSS they use. little would
  download or ship the CLI, or the theme would commit the CSS it builds, and
  a theme that builds on it would need the CLI to use a class it doesn't.
  Its classes fill the markup a theme's author reads and replaces. It may
  suit a first-party theme of its own, later.
- **A classless framework**, like Pico or Water.css: plain HTML styled well,
  with little to write, but the theme would track someone else's styles, and
  override them for what docs need, like a drawer, a table of contents, and
  alerts.
- **Sass**, which little compiles with grass: variables, nesting, and
  partials, but its variables are gone once it's compiled, so a dark mode
  writes each colour twice, and a site can't set one. CSS has custom
  properties, nesting, and layers of its own now.
- **Plain CSS**, as it's written today: custom properties, `light-dark()`,
  cascade layers, `:has()`, and popovers do what docs need, with nothing to
  build, but some are from 2024.

## Decision
- The built-in theme is plain CSS, one stylesheet, `assets/style.css`, with
  no framework, no build step, and no Node. It needs no JavaScript: all but
  search, which is Pagefind's, works without it.
- **Tokens:** its colours, fonts, and sizes are custom properties on
  `:root`, like `--accent`, `--bg`, and `--content-width`. Each colour is a
  pair, `light-dark(light, dark)`, and `color-scheme: light dark` picks one
  as the reader's system is, from the first paint. Setting a token changes it
  everywhere, in light and dark.
- **Layers:** every rule is in a cascade layer, in this order:
  `little.tokens`, `little.base`, `little.layout`, `little.components`,
  `little.content`, `little.print`. So CSS that isn't in a layer, like a
  theme's that builds on it, or a page's `<style>`, wins over any of it,
  however specific its selectors. A later layer wins whatever its selectors,
  so a rule that changes another at a breakpoint is in the same layer.
  Pagefind's `--pf-*` colours are set outside any layer, as Pagefind sets its
  own.
- A few declarations are `!important`, where nothing else wins: restyling
  Pagefind's search button on a narrow screen, as Pagefind's selectors are as
  specific as an ID, and outside any layer; a diagram's background, which
  merman sets on the `<svg>` itself; and what print hides. In a layer,
  `!important` wins over CSS outside one, even `!important`, so a theme
  overrides these in the same layer, or, for a diagram's background, sets
  `--diagram-bg`.
- **Breakpoints:**

  | Width | What changes |
  | --- | --- |
  | under 30rem | the pager is one column |
  | under 40rem | the site's description is hidden, and code blocks run to the screen's edges |
  | 48rem and up | the gutter widens, and the search button becomes a box |
  | under 60rem | the nav is a drawer, opened by a menu button in the header |
  | 60rem and up | the nav is a column on the left, which scrolls on its own |
  | under 75rem | the table of contents folds up above the page, as "On this page", in a `<details>` |
  | 75rem and up | the table of contents is a column on the right |

  The page's text is at most 46rem wide, and the layout 90rem.
- **The drawer** is a popover, `<nav popover>`, which the menu button opens
  with `popovertarget`, so it needs no script. It opens over the page, with a
  backdrop, and closes with its close button, Escape, or a tap outside it. It
  slides in, unless the reader prefers reduced motion. From 60rem, the same
  `<nav>` is the column, so the markup has one nav.
- A site of one page has no nav, as there's nowhere else to go.
- **Accessibility:**
  - a "Skip to content" link, first on the page, shown when it has focus;
  - landmarks: the header, the nav, named "Main", `main`, the breadcrumbs,
    the tables of contents, and the footer;
  - `aria-current="page"` on the current page, in the nav and the
    breadcrumbs;
  - a visible focus ring, in the accent colour, on whatever has focus from
    the keyboard, with `:focus-visible`;
  - text meets WCAG's AA contrast, 4.5:1, on each background it's on, in
    light and dark, code's colours and alerts' titles too, and the focus ring
    3:1;
  - links in a page's content are underlined, so they don't stand out by
    colour alone;
  - smooth scrolling, and the drawer's slide, only for a reader who hasn't
    asked for reduced motion;
  - in forced colours, like Windows' contrast themes, the current page in
    the nav is outlined, and an alert's icon is drawn in the text's colour,
    where colour alone marked them;
  - a printed page is light, without the header, nav, tables of contents,
    breadcrumbs, or pager, and with each link's URL after it;
  - icons are hidden from screen readers, and each button or link with only
    an icon is named, like "Menu" and "Repository".
- **Code** is light or dark, as the page is, in GitHub's colours, and the
  theme colours GitHub's classes as well as Pygments', whichever
  `markdown.highlight` chooses. This amends
  [0023](./0023-highlighting-code.md), whose built-in theme had dark code
  blocks, and coloured only Pygments' classes. A notebook's `output` is set
  apart from code by a dashed border, on the page's own background, which
  amends [0024](./0024-jupyter-notebooks.md), whose built-in theme showed it
  lighter than code.
- **Diagrams** are drawn in one look, Mermaid's light default
  ([0028](./0028-how-diagrams-look.md)), so in dark mode, each sits on a
  light card.
- **Browsers:** what the theme needs is Baseline 2024: `light-dark()` has
  been in every major browser since May 2024, and the `popover` attribute,
  `popovertarget`, and `:popover-open` since April 2024. Cascade layers have
  been since 2022. `@starting-style`, from August 2024, only slides the
  drawer in. The theme has no fallbacks for older browsers.

## Consequences
- A site looks finished out of the box, on a phone and on a wide screen, in
  light and dark, with nothing to install, and works without JavaScript, but
  for search.
- The stylesheet grows from about 350 lines to about 970.
- A theme that builds on the built-in one restyles it without `!important`,
  or matching its selectors, but for the few above, and recolours it by
  setting its tokens, in light and dark.
- Limits, for now:
  - a reader can't choose light or dark for a site: it's as their system
    is;
  - if a window widens past 60rem while the drawer is open, the drawer stays
    open, over the page, until it's closed;
  - the table of contents above the page stays open after a link in it is
    followed;
  - a site can't add a stylesheet of its own to the built-in theme: a theme
    that builds on it replaces `default.html`, to link one, or
    `assets/style.css`;
  - diagrams have no dark colours, so they sit on a light card in dark mode;
  - a browser from before 2024 isn't supported.
