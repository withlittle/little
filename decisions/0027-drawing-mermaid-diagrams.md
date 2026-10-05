# 0027 — Drawing Mermaid diagrams as SVG

- Status: proposed
- Date: 2026-10-05
- Amends: [0016](./0016-checking-links.md),
  [0022](./0022-markdown-extensions.md),
  [0023](./0023-highlighting-code.md),
  [0024](./0024-jupyter-notebooks.md)

## Context
Docs are full of diagrams: how a request flows, the states a job goes
through, what depends on what. On GitHub, an author writes one in Mermaid,
in a ```` ```mermaid ```` block, and a reader sees the diagram. little shows
the same block as code ([0023](./0023-highlighting-code.md)), so an author
draws the diagram in another tool and adds it as an image, which drifts
from the docs, or finds a theme that loads Mermaid's script.

Mermaid is the language Markdown has settled on for diagrams: GitHub,
GitLab, Azure DevOps, and Obsidian draw it. Its own renderer, mermaid.js,
draws a diagram as SVG, in a browser.

## Options considered
- **Where a diagram is drawn:**
  - in each reader's browser, by mermaid.js, which a theme loads: 3.6 MB of
    script, 980 KB compressed, that draws each diagram after the page loads,
    so the page jumps, and a diagram with a mistake shows an error to a
    reader, not to its author;
  - or as little builds each page, as an `<svg>` in it, which shows in every
    theme as soon as the page loads, without a script, and a mistake is
    found as the site is built, as a broken link is.
- **What draws it as little builds:**
  - mermaid.js itself, through mermaid-cli, which needs Node and a headless
    Chromium: little doesn't need Node, and won't
    ([AGENTS.md](../AGENTS.md));
  - a service, like Kroki or mermaid.ink, which sends each diagram away
    over the network, and fails without one;
  - [mermaid-rs-renderer](https://crates.io/crates/mermaid-rs-renderer), a
    renderer in Rust of its own design. Of 13 diagrams, of 9 types, it
    refused some valid ones, drew others wrong without an error, and kept
    `javascript:` links;
  - [merman](https://crates.io/crates/merman), a port of Mermaid 11.12 to
    Rust, which lays out and writes each diagram as mermaid.js does. For the
    same 13, it drew nearly what mermaid.js drew, and it read 3,918
    diagrams cut short at every character, as an author types them,
    without a crash.
- **Other languages,** like Graphviz, D2, and PlantUML: each needs its own
  renderer, and none is as widely written, so they're left for plugins.
- **A diagram that can't be drawn,** like one half typed: stop the build,
  which stops `little serve` too; or warn, as of math that can't be
  rendered ([0025](./0025-rendering-math-as-mathml.md)), and show it as
  code.
- **Whether diagrams are on:** off, so a site turns them on; or on, as on
  GitHub, where a ```` ```mermaid ```` block is always a diagram.
- **What a diagram can do:** mermaid.js cleans each diagram, so it can't
  reach the page around it. merman 0.7 doesn't clean them as well: CSS in
  a diagram's config, or in a `classDef`, can style the whole page, and a
  class diagram can carry HTML past its cleaning, scripts and all. So a
  diagram can do what raw HTML can, and a site that turns raw HTML off for
  Markdown it doesn't trust ([0022](./0022-markdown-extensions.md)) needs
  diagrams off too: without a word, or by stopping the build until it says
  so.

## Decision
- `markdown.mermaid` chooses how little writes a ```` ```mermaid ```` block:
  - `svg`, little's default: as a diagram, an `<svg>` drawn with merman as
    little builds each page;
  - `false`: as code, in a `<code class="language-mermaid">`, for a theme
    that draws diagrams itself.

  Anything else stops the build, naming the two: `true` too, as a site's
  `true` would undo its theme's `false`.
- A block is Mermaid by its language, the first word after the fence, up to
  a comma, in any case, as for highlighting
  ([0023](./0023-highlighting-code.md)).
- `mermaid: svg` with `raw_html: false` stops the build, asking for
  `mermaid: false`, so a site that doesn't trust its Markdown doesn't draw
  it.
- Each diagram's `<svg>` has the id `mermaid-1`, `mermaid-2`, and so on, in
  the order they come in the page, which its own styles are scoped to. A
  theme styles diagrams with `svg[id^="mermaid-"]`.
- A diagram is drawn the same in each build: Mermaid's random seeds are
  fixed, and its today, for a gantt chart's line at today's date and a
  date without a year, is when the build starts, in UTC, or
  `SOURCE_DATE_EPOCH`, as for `site.year`
  ([0026](./0026-the-year-a-site-is-built.md)).
- A gantt chart is drawn 800 pixels wide, as little can't measure the page,
  and merman would draw it 1,184 wide. A diagram's background is
  transparent, so it's on the theme's. The config in a diagram, like a
  `%%{init: …}%%` directive or its frontmatter, still applies to it.
- A diagram merman can't read is warned of, as math is, with the file and
  line it's on, like
  `warning: can't draw the diagram on line 12 of ./docs/guide.md: …`, with
  merman's reason, and written as code, as with `false`, so the page still
  builds. `little build --strict` fails on one, and `little serve` warns
  of each after each build.
- A notebook's Markdown cells' diagrams are drawn too
  ([0024](./0024-jupyter-notebooks.md)), and warned of without a line,
  which would be the cell's.
- The built-in theme centres each diagram, and shrinks one too wide for
  the page.
- New crates: `merman`, and `chrono`, which merman uses, for its today.

## Consequences
- Diagrams show in every theme as soon as a page loads, without a script,
  as on GitHub, and a diagram with a mistake is found as the site is built.
- A theme that draws diagrams with mermaid.js sets `markdown.mermaid:
  false` in its `little.yml`, and a site that sets `raw_html: false` sets
  `mermaid: false` with it.
- little grows by 11.5 MB, to 52 MB, with about 60 more crates, which
  merman reads and lays out each type of diagram with.
- A page grows by the SVG of each diagram, about 10 to 65 KB each, rather
  than loading 980 KB of script once.
- Search doesn't index the words in a diagram, as Pagefind leaves `<svg>`
  out.
- Limits, for now:
  - a link in a diagram, from a `click`, isn't led from the page's folder,
    or put under the path of the site's `url`, and isn't checked;
  - diagrams have Mermaid's default look, and the theme can't change it,
    or show them dark in dark mode; a diagram can still choose its own,
    in its config;
  - a page can't choose its own diagrams in frontmatter;
  - math in a diagram's labels isn't rendered;
  - merman isn't mermaid.js, so a diagram can be drawn a little
    differently than on GitHub, or not at all, and Mermaid's newest syntax
    may need a newer merman;
  - a gantt chart's line at today's date is at the day the site was built,
    unless the diagram turns it off, with `todayMarker off`.
