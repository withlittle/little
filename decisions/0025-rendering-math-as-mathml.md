# 0025 — Rendering math as MathML

- Status: accepted
- Date: 2026-10-04
- Amends: [0016](./0016-checking-links.md),
  [0022](./0022-markdown-extensions.md),
  [0024](./0024-jupyter-notebooks.md)

## Context
Docs for science and engineering are full of math, and notebooks' Markdown
cells more so. With `markdown.math` on, little finds math as GitHub does, in
`$…$`, `$$…$$`, `` $`…`$ ``, and ```` ```math ```` blocks, and marks each
with `data-math-style` for a theme's script to render
([0022](./0022-markdown-extensions.md)). The built-in theme doesn't load
one, so a reader sees TeX. With `math` off, little's default, it's worse:
`\{` loses its `\`, and `$a*b + c*d$` turns to emphasis, before any script
could read it.

Browsers show MathML, HTML's own math, without a script: Chrome and Edge
since 2023, and Firefox and Safari for longer. TeX can be turned into it as
little builds each page, when it highlights code
([0023](./0023-highlighting-code.md)).

## Options considered
- **Where math is rendered:**
  - in each reader's browser, by KaTeX or MathJax, a script, its CSS, and
    its fonts that a theme loads, so the page shows TeX until it has run,
    and only in a theme that loads it;
  - or as little builds each page, as MathML, which shows in every theme as
    soon as the page loads, as text, to search, to copy, and for a screen
    reader to read. Browsers lay it out themselves, in their own math font,
    so it looks a little different in each, and plainer than KaTeX.
- **What turns TeX into MathML:**
  - [math-core](https://crates.io/crates/math-core), which reads LaTeX's
    math, with environments like `aligned` and `pmatrix`, and writes MathML
    Core, the part of MathML that every browser lays out. It read all 242
    formulas of one site of notebooks about quantum computing, and adds
    about 260 KB to little;
  - [pulldown-latex](https://crates.io/crates/pulldown-latex), which read
    195 of them, without `\ket` or `\braket`;
  - KaTeX itself, which needs Node, or a JavaScript engine in little.
- **A formula that can't be read,** like a typo, or a command math-core
  doesn't have: stop the build, which stops `little serve` too, while a
  formula is half typed; or warn, as of a broken link
  ([0016](./0016-checking-links.md)), and show its TeX.
- **How a site chooses:** one setting, `math`, for what little writes:
  MathML, TeX for a theme's script, or neither; or two, one to find math
  and one to choose MathML, where a site that turned math on, as it does
  now, couldn't tell it apart from undoing a theme's choice of TeX.
- **Whether math is on:** off, as now, so `$` is always a dollar; or on, as
  on GitHub. There, `$5 and $10` is still text, as a `$` can't open math
  before a space, or close it after a space or before a digit.

## Decision
- `markdown.math` chooses how little writes math, in `$…$`, `$$…$$`,
  `` $`…`$ ``, and ```` ```math ```` blocks:
  - `mathml`, little's default: as MathML, written with math-core as little
    builds each page, a `<math>`, or a `<math display="block">` for `$$…$$`
    and a ```` ```math ```` block;
  - `tex`: as before, its TeX marked with `data-math-style`, for a theme
    that renders math itself;
  - `false`: none, so `$` is a dollar, and a ```` ```math ```` block is
    code.

  Anything else stops the build, naming the three: `true` too, as a site's
  `true` would undo its theme's `tex`.
- Each formula is read on its own: a command `\newcommand` defines in one
  isn't in the next, and `\eqref` only finds a `\label` in the same formula.
- A formula math-core can't read is warned of, as a broken link is, as it's
  written, on one line, with the file it's in, like
  `warning: can't render $\dv{f}{x}$ in ./docs/intro.md: Unknown command "\dv".`
  A ```` ```math ```` block's is shown in `$$…$$`. Its TeX is written as
  with `tex`, so the page still builds, and reads as it was written.
  `little build --strict` fails on one, as on a broken link, and
  `little serve` warns of each after each build.
- A heading's math is MathML in the heading, but TeX in `page.toc`, the nav,
  and the page's title, which are text; its id doesn't change.
- The built-in theme scrolls a formula too wide for the page, rather than
  the page, where the browser can, and leaves its font to the browser.
- New crate: `math-core`.

## Consequences
- Math shows in every theme as soon as a page loads, without a script, and
  a notebook's Markdown cells show theirs, as in Jupyter.
- A theme that renders math with KaTeX or MathJax sets `markdown.math: tex`
  in its `little.yml`, and can load its script only when
  `site.markdown.math` is `tex`.
- A site that set `math: true` sets `mathml` or `tex` instead, or nothing,
  now that math is on.
- A `$` in prose can start math, as on GitHub: `$HOME/$PATH` is math, unless
  it's written `\$HOME/\$PATH`, or in code.
- Search indexes a formula's MathML as text, so `x^2` is found as `x2`.
- little grows by about 260 KB.
- Limits, for now:
  - each formula is read on its own, so a page can't number its equations,
    or share macros, across formulas;
  - browsers' math fonts differ, and some Linux desktops have none, so math
    looks different there, unless a theme brings its own;
  - Safari, and every browser on iOS, can't scroll a `<math>`, so there a
    formula too wide for the page widens it;
  - math in a notebook's `text/latex` outputs, and environments like
    `\begin{align}` outside `$$`, aren't rendered;
  - a page can't choose its own math in frontmatter.
