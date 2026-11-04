---
title: Math
description: Math in LaTeX, written as on GitHub, and rendered as each page is built.
---

# Math

Write math as on GitHub, in LaTeX. little renders it as it builds the page,
as MathML, HTML's own math, so browsers show it without JavaScript.

The roots of $ax^2 + bx + c = 0$ are $x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$,
and

$$
\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}
$$

## Writing math

| You write | For |
| --------- | --- |
| `$x^2$`, or `` $`x^2`$ `` | math in a line of text |
| `$$…$$`, or a ```` ```math ```` block | math on its own |

````markdown
```math
\begin{pmatrix} a & b \\ c & d \end{pmatrix}
\begin{pmatrix} x \\ y \end{pmatrix}
=
\begin{pmatrix} ax + by \\ cx + dy \end{pmatrix}
```
````

is

```math
\begin{pmatrix} a & b \\ c & d \end{pmatrix}
\begin{pmatrix} x \\ y \end{pmatrix}
=
\begin{pmatrix} ax + by \\ cx + dy \end{pmatrix}
```

little reads LaTeX's math, with environments like `aligned` and `pmatrix`,
using [math-core](https://crates.io/crates/math-core).

- As on GitHub, a `$` before a space doesn't start math, and one after a
  space, or before a digit, doesn't end it. So `$5 and $10` is text. Write
  `\$` for a `$` that would start or end math, like `\$HOME/\$PATH`.
- Each formula is read on its own. So a command that `\newcommand` defines
  in one isn't in the next, and equations can't be numbered across a page.
- Math in a heading is MathML there, but TeX in tables of contents, the
  nav, and the page's title, which are plain text.
- Each browser lays out MathML itself, in its own math font, so math looks
  a little different in each. The built-in theme scrolls a formula that's
  too wide for the page. Safari can't, so there it widens the page.
- A formula little can't read, like one with a typo, or a command it
  doesn't have, is written as its TeX. little warns of it, with the file
  it's in:

  ```text
  warning: can't render $\dv{f}{x}$ in ./docs/calculus.md: Unknown command "\dv".
  ```

  `little build --strict` fails on one. `little serve` warns of each, after
  each build.

## Choosing how math is written

`markdown.math` chooses how math is written:

```yaml
# little.yml
markdown:
  math: tex
```

| Value | Math is written… |
| ----- | ---------------- |
| `mathml` | as MathML: a `<math>`, or a `<math display="block">` for `$$…$$` and a ```` ```math ```` block. little's default. |
| `tex` | as its TeX, marked with `data-math-style="inline"` or `"display"`, for a theme that renders math in the browser, with a script like KaTeX or MathJax |
| `false` | not at all: `$` is a dollar, and a ```` ```math ```` block is code |

A theme that renders math itself sets `tex` in its `little.yml`. The site's
`little.yml` has the last word.

- A page can't choose its own in its frontmatter.
- Anything but `mathml`, `tex`, or `false` is an error, even `true`. Every
  command stops, and says why.
