---
title: Math
description: Math in LaTeX, rendered as each page is built.
---

# Math

Write math in LaTeX, and little renders it as it builds the page, as
MathML, HTML's own math, so browsers show it without JavaScript:

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

little reads LaTeX's math, with environments like `aligned` and `pmatrix`.
Each formula is read on its own, so a command that `\newcommand` defines in
one isn't in the next.

A `$` before a space doesn't start math, and one after a space, or before
a digit, doesn't end it, so `$5 and $10` is text. Write `\$` for a `$` that
would start or end math, like `\$HOME/\$PATH`.

little writes a formula it can't read, like one with a typo or a command it
doesn't have, as its TeX, and warns of it:

```text
warning: can't render $\dv{f}{x}$ in ./docs/calculus.md: Unknown command "\dv".
```

## Choosing how math is written

`markdown.math` chooses how math is written:

```yaml
# little.yml
markdown:
  math: tex
```

| Value | Math is written… |
| ----- | ---------------- |
| `mathml` | as MathML. little's default. |
| `tex` | as TeX, marked with `data-math-style="inline"` or `"display"`, for a script like KaTeX or MathJax to render |
| `false` | not at all: `$` is a dollar, and a ```` ```math ```` block is code |
