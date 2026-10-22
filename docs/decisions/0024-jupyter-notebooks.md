# 0024 — Jupyter notebooks as pages

- Status: accepted; amended by [0025](./0025-rendering-math-as-mathml.md),
  which renders the math in Markdown cells as MathML, without a theme's
  help, by [0027](./0027-drawing-mermaid-diagrams.md), which draws their
  Mermaid diagrams as SVG, and by
  [0029](./0029-plugins-that-render-code-blocks.md), which gives the code
  blocks in their Markdown, but not their code cells, to plugins, and by
  [0033](./0033-the-built-in-themes-design.md), whose built-in theme sets
  `output` blocks apart from code with a dashed border, and by
  [0060](./0060-other-formats-in-plugins.md), which moves notebooks out of
  little, into a plugin, `jupyter`, that reads them with the `formats` hook
- Date: 2026-10-04

## Context
Docs for data and science are often Jupyter notebooks: `.ipynb` files of
Markdown and code cells, saved with each code cell's outputs, like what it
printed, a table, or a chart. GitHub shows a notebook as a page, outputs and
all. little copies an `.ipynb` file as it is, so a reader downloads its JSON.
To publish one, an author converts it to Markdown first, with
`jupyter nbconvert --to markdown`, which needs Python, and keeps the copy,
and the images it extracts, in step with the notebook.

A notebook is JSON, in Jupyter's format 4 since 2015: a list of `cells`, each
`markdown`, `code`, or `raw`, with its `source`. A code cell has its
`outputs`, each of which is:
- a `stream`, text written to stdout or stderr;
- an `execute_result` or a `display_data`, in one or more MIME types, like
  `text/plain`, `text/html`, and `image/png`, for whatever shows it to choose
  from;
- or an `error`, with its traceback.

## Options considered
- **Core or plugin:** a plugin, once there's an API, as for Sphinx's
  docstrings; or core, as Markdown is. Notebooks are how many authors write
  docs, not a niche format, and there's no plugin API yet.
- **Reading a notebook:**
  - [nbformat](https://crates.io/crates/nbformat), runtimed's, reads every
    version of the format, upgrading old ones, but it's stricter than
    Jupyter: it drops a result whose `execution_count` is `null`, as
    `nbstripout --keep-output` saves it, printing its JSON, and stops at a
    cell without `metadata`, or with an `id` it doesn't accept;
  - crates that convert notebooks: `ipynb-to-md` is a command, not a
    library, and leaves outputs out; `mdbook-jupyter` is mdBook's, and
    shows HTML as code; and `notebookx` converts a notebook to a script,
    not a page;
  - our own types, read with serde, of only what a page shows, in format 4.
- **What a notebook becomes:**
  - Markdown text, as nbconvert writes it, rendered as any page is. But
    Markdown ends HTML at a blank line, and pandas' tables have one in their
    `<style>`, so the rest of the table reads as code;
  - comrak's tree of a page, built from the cells, which comrak writes as it
    writes Markdown's, so each output is placed whole.
- **How outputs look:** in `<div>`s, with prompts like `In [3]:`, as
  nbconvert's HTML has them, or Quarto's `<div class="cell-output-stdout">`;
  or as code blocks and images, as nbconvert's Markdown has them, so a
  notebook's page is as plain as any page.

## Decision
- Each `.ipynb` file in the source, but hidden ones, like those in
  `.ipynb_checkpoints/`, is a page, as a `.md` file is: `analysis.ipynb` is
  at `/analysis`, and `README.ipynb` is its folder's page. It has a title, a
  table of contents, and a place in the nav, as a `.md` file's page does, is
  searched, and has its links checked. A link to it, like
  `analysis.ipynb#results`, leads to its page. The `.ipynb` file isn't
  copied. `analysis.md` beside `analysis.ipynb` stops the build, as pages
  that clash do.
- little reads format 4 with its own types, which leave out what a page
  doesn't show, like each cell's `id` and execution count. A notebook in
  format 3 or earlier, or a file that isn't a notebook, stops the build,
  naming it.
- Its cells, in order, become comrak's tree of the page, which is written as
  Markdown's is, with the site's extensions, links led from the notebook's
  folder, code highlighted, and the table of contents:
  - a Markdown cell is Markdown, read on its own, so a code block it leaves
    open doesn't swallow the next. An image attached to it, and shown with
    Markdown, like `![chart.png](attachment:chart.png)`, is put in the page
    as a `data:` URL. Its footnotes are numbered on through the page, and
    listed at its end, and its inline ones, `^[like this]`, named on, as
    comrak names each cell's from `__inline_1`;
  - a code cell is a code block in the language of the notebook's
    `language_info`, else its kernel's, highlighted as in Markdown. An empty
    one is left out, and there are no prompts;
  - a raw cell is left out.
- Each code cell's outputs follow it, in order:
  - text it wrote, to stdout or stderr, an error's traceback, and a result
    that's only text are each a code block in `output`, which little doesn't
    highlight: `<pre class="highlight"><code class="language-output">`. Text
    written to the same stream, one output after another, is one block.
    Terminal colours are taken out, and a line redrawn with `\r`, like a
    progress bar's, is shown as it was last drawn;
  - a result or a display shows the first of its types that little can
    show: HTML, as it is, when `markdown.raw_html` is on; Markdown; an SVG,
    PNG, JPEG, or GIF, as a `data:` URL, in an `<img>` with empty alt text,
    in a `<p>`; then text. SVG, which can hold script, is shown only when
    `markdown.raw_html` is on, as comrak leaves out its `data:` URL
    otherwise. JavaScript, LaTeX, and widgets need Jupyter to show them, so
    their text shows instead.
- Frontmatter is in a raw or Markdown cell at the top that starts with
  `---`, as Quarto and MyST write it. A raw cell is only frontmatter; a
  Markdown cell's Markdown goes on after it.
- The built-in theme shows `output` blocks lighter than code, keeps images,
  and a wide table in a `<div>`, as pandas' are, within the page, and shows
  pandas' tables as Jupyter does, as wide as their columns, and aligned
  right.
- No new crates: little uses serde_json and base64 itself now, which it
  built already, for other crates.

## Consequences
- A notebook is a page as it's saved, without converting it, or Python.
- A notebook's page is as plain as a Markdown page, without Jupyter's
  `<div>`s, prompts, or styles, so a theme styles both alike, and an output
  block by its `language-output`.
- Each image is in its page, as in nbconvert's HTML, so it's downloaded
  with the page, not cached on its own. A page's first image is in its
  search result too, whole.
- Limits, for now:
  - an output that needs Jupyter's JavaScript, like a widget, or HTML that
    loads Plotly with Jupyter's `require`, shows its text, or nothing;
  - math needs `markdown.math`, and a theme that renders it;
  - cells' tags, like `remove-input`, are ignored, and so is an image's
    width and height;
  - a cell magic, like `%%bash`, is highlighted in the notebook's language;
  - a footnote is its cell's own: a note in another cell isn't found, and
    named notes in two cells shouldn't share a name;
  - an attached image is shown from Markdown, not from HTML, like
    `<img src="attachment:chart.png">`, as little doesn't rewrite HTML; and
    an attached SVG only when `markdown.raw_html` is on;
  - the notebook isn't in the site to download, but a theme can link to it
    in the repository, with `page.source_url`;
  - notebooks saved as text, like Jupytext's `.py` files and Quarto's
    `.qmd`, are copied, not built.
