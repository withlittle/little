---
title: Math
description: Math in LaTeX, written as on GitHub, rendered as MathML as each page is built, and choosing how it's written.
---

# Math

Write math as on GitHub, in LaTeX, and little renders it as it builds the
page, as MathML, HTML's own math, which browsers show without JavaScript.
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

- little reads LaTeX's math, with environments like `aligned` and `pmatrix`,
  with [math-core](https://crates.io/crates/math-core). Each formula is read
  on its own, so a command that `\newcommand` defines in one isn't in the
  next, and equations can't be numbered across a page.
- As on GitHub, a `$` before a space doesn't start math, and one after a
  space, or before a digit, doesn't end it, so `$5 and $10` is text. Write
  `\$` for a `$` that would, like `\$HOME/\$PATH`.
- A heading's math is MathML in the heading, and TeX in tables of contents,
  the nav, and the page's title, which are text.
- Browsers lay MathML out themselves, in their own math font, so it looks a
  little different in each. The built-in theme scrolls a formula too wide for
  the page, but Safari can't, so there it widens the page.
- A formula little can't read, like one with a typo, or a command it doesn't
  have, is written as its TeX, as with `tex`, below, and warned of, with the
  file it's in, so you can search for it:

  ```text
  warning: can't render $\dv{f}{x}$ in ./docs/calculus.md: Unknown command "\dv".
  ```

  `little build --strict` fails on one, and `little serve` warns of each
  after each build.

## Choosing how math is written

`markdown.math` chooses how math is written:

| Value | Math is written… |
| ----- | ---------------- |
| `mathml` | as MathML: a `<math>`, or a `<math display="block">` for `$$…$$` and a ```` ```math ```` block. little's default. |
| `tex` | as its TeX, marked with `data-math-style="inline"` or `"display"` for a theme that renders math in the browser, with a script like KaTeX or MathJax |
| `false` | not at all: `$` is a dollar, and a ```` ```math ```` block is code |

```yaml
# little.yml
markdown:
  math: tex
```

- A theme that renders math itself sets `tex` in its `little.yml`, and the
  site's `little.yml` has the last word. A page can't choose its own in its
  frontmatter.
- Anything but `mathml`, `tex`, or `false` is an error, `true` too, and every
  command stops and says why.
