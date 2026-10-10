---
title: Styling Markdown
description: The HTML little writes for alerts, headings, code, notebooks, math, and diagrams, for a theme's CSS to style.
---

# Styling Markdown

`page.content` is the page's Markdown as comrak renders it, for a theme's CSS
to style, as the built-in theme's
[`style.css`](https://github.com/withlittle/little/blob/main/theme/_static/assets/style.css)
does:

- An [alert](../writing/markdown.md#callouts) is a
  `<div class="markdown-alert markdown-alert-note">`, or `-tip`,
  `-important`, `-warning`, or `-caution`, with its title in a
  `<p class="markdown-alert-title">`, as on GitHub, so CSS written for
  GitHub's alerts styles them too.
- Each [heading](../writing/markdown.md#headings) has an `id`, and after its
  text, an empty `<a class="anchor" href="#…">` that links to it, labelled
  for screen readers. It shows nothing until a theme gives it something to
  show, like the built-in theme's `#`.
- A [highlighted](../writing/code.md#choosing-how-code-is-highlighted) code
  block is a `<pre class="highlight">`, around comrak's
  `<code class="language-rust">`, with a `<span class="k">` around each token
  that has a class, as Pygments writes them: never nested, and never over the
  end of a line. A block in a language little doesn't have is plain text in
  the same `<pre class="highlight">`.
- A [notebook](../writing/notebooks.ipynb)'s output, like what its code
  printed, is a code block in `output`,
  `<pre class="highlight"><code class="language-output">`, which the built-in
  theme shows lighter than code, with `pre:has(> code.language-output)`. A
  chart is an `<img>` with a `data:` URL, in a `<p>` of its own, and a table
  pandas shows is its `<div>`, `<style>`, and `<table class="dataframe">`,
  which the built-in theme shows as Jupyter does: as wide as its columns, and
  aligned right.
- [Math](../writing/math.md#choosing-how-math-is-written) is MathML: a
  `<math>` in a line, and a `<math display="block">` on its own, inside a
  `<p>` for `$$…$$`. The built-in theme scrolls a wide one with
  `math[display="block"] { overflow-x: auto; overflow-y: hidden; }`.
- A [diagram](../writing/diagrams.md#choosing-how-diagrams-are-drawn) is an
  `<svg>` with the id `mermaid-1`, `mermaid-2`, and so on, in the order
  they're in the page, which its own styles are scoped to, so they outweigh a
  theme's CSS: a theme gives diagrams its colours and font with
  [`mermaid`](../writing/diagrams.md#choosing-how-diagrams-look) in its
  `little.yml` instead. It's as wide as it's drawn, but no wider than the
  page. The built-in theme centres each with
  `svg[id^="mermaid-"] { display: block; margin: 1.5rem auto; }`.
- A theme gets the site's
  [extensions](../writing/markdown.md#choosing-markdowns-extensions) as
  `site.markdown`, so it can load a script only for a site that needs it,
  like KaTeX inside `{% if site.markdown.math == "tex" %}`, highlight.js
  inside `{% if not site.markdown.highlight %}`, or Mermaid's inside
  `{% if not site.markdown.mermaid %}`.
