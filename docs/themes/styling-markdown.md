---
title: Styling Markdown
description: The HTML little writes for alerts, headings, code, tables, notebooks, math, and diagrams, for a theme's CSS to style.
---

# Styling Markdown

`page.content` is the page's Markdown as comrak renders it. A theme's CSS
styles it, as the built-in theme's
[`style.css`](https://github.com/withlittle/little/blob/main/theme/_static/assets/style.css)
does. This page shows the HTML little writes for each part of a page.

## Alerts

An [alert](../writing/markdown.md#callouts) is a
`<div class="markdown-alert markdown-alert-note">`, or `-tip`,
`-important`, `-warning`, or `-caution`. Its title is in a
`<p class="markdown-alert-title">`. That's GitHub's HTML, so CSS written
for GitHub's alerts styles them too.

## Headings

Each [heading](../writing/markdown.md#headings) has an `id`. After its text
is an empty `<a class="anchor" href="#…">` that links to it, labelled for
screen readers. It shows nothing until a theme gives it something to show,
like the built-in theme's `#`.

## Code

A [highlighted](../writing/code.md#choosing-how-code-is-highlighted) code
block is a `<pre class="highlight">`, around comrak's
`<code class="language-rust">`. Each token that has a class is in a
`<span>`, like `<span class="k">`, as Pygments writes them. A `<span>` is
never nested, and never runs over the end of a line.

- A block in a language little doesn't have is plain text, in the same
  `<pre class="highlight">`.
- Once the page has loaded, the built-in theme's script puts each block in
  a `<div class="code-block">`, with its
  [copy button](../writing/code.md#copying-code).

## Tables

A [table](../writing/markdown.md#githubs-extensions) is a `<table>`. Its
heading row is in a `<thead>`, and the rest in a `<tbody>`.

- The built-in theme makes each table as wide as the page. A table that
  fills the page can't scroll. So once the page has loaded, the built-in
  theme's script puts each in a `<div class="table-scroll">`, which scrolls
  one that's too wide for it.
- Without the script, a table is as wide as its columns, and scrolls
  itself.
- A column aligned with a colon, like `| :-: |`, has an `align` on each of
  its cells, like `<td align="center">`. Any `text-align` overrides it. So
  the built-in theme aligns only cells without one, with
  `:is(th, td):not([align])`.

## Notebooks

A [notebook](../plugins/jupyter.ipynb)'s output, like what its
code printed, is a code block in `output`:
`<pre class="highlight"><code class="language-output">`. The built-in theme
sets it apart from code with a dashed border, with
`pre:has(> code.language-output)`.

- A chart is an `<img>` with a `data:` URL, in a `<p>` of its own.
- A table pandas shows is its `<div>`, `<style>`, and
  `<table class="dataframe">`. The built-in theme shows it as Jupyter does:
  as wide as its columns, and aligned right. It scrolls itself, not in a
  `.table-scroll`.

## AsciiDoc, Org mode, and reStructuredText

A page in [AsciiDoc](../plugins/asciidoc.adoc),
[Org mode](../plugins/org-mode.org), or
[reStructuredText](../plugins/restructuredtext.rst) is written as Markdown.
So it's styled as Markdown is, except for what Markdown can't write:

- A table without headings has a `<thead>` of empty `<th>`s. The built-in
  theme hides it, with `thead tr:not(:has(th:not(:empty)))`.
- A term or its definition, in a `<dl>`, can hold paragraphs. The built-in
  theme takes off their outer margins, with `:is(dt, dd) > :first-child`,
  and `:last-child`.

## Math

[Math](../writing/math.md#choosing-how-math-is-written) is MathML: a
`<math>` in a line, and a `<math display="block">` on its own, inside a
`<p>` for `$$…$$`. The built-in theme scrolls a wide one with
`math[display="block"] { overflow-x: auto; overflow-y: hidden; }`.

## Diagrams

A [diagram](../writing/diagrams.md#choosing-how-diagrams-are-drawn) is an
`<svg>` with the id `mermaid-1`, `mermaid-2`, and so on, in the order
they're in the page. Its own styles are scoped to that id, so they outweigh
a theme's CSS. So a theme gives diagrams its colours and font with
[`mermaid`](../writing/diagrams.md#choosing-how-diagrams-look) in its
`little.yml` instead.

A diagram is as wide as it's drawn, but no wider than the page. The
built-in theme centres each with
`svg[id^="mermaid-"] { display: block; margin: 1.5rem auto; }`.

## Loading a script a site needs

A theme gets the site's
[extensions](../writing/markdown.md#choosing-markdowns-extensions) as
`site.markdown`. So it can load a script only for a site that needs it:

- KaTeX inside `{% if site.markdown.math == "tex" %}`.
- highlight.js inside `{% if not site.markdown.highlight %}`.
- Mermaid's inside `{% if not site.markdown.mermaid %}`.
