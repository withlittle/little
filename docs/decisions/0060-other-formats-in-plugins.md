# 0060 — Other formats in plugins

- Status: accepted
- Date: 2026-10-22
- Amends: [0024](./0024-jupyter-notebooks.md),
  [0029](./0029-plugins-that-render-code-blocks.md),
  [0030](./0030-plugins-that-add-pages.md),
  [0046](./0046-plugin-hooks.md),
  [0057](./0057-starting-a-site-with-little-init.md)

## Context
little builds pages from two formats: Markdown, and Jupyter's notebooks,
which [0024](./0024-jupyter-notebooks.md) built in, as notebooks are how
many authors write docs, and there was no plugin API. Authors write docs in
other formats too: AsciiDoc, reStructuredText, and Org-mode each have sites
written in them. Built in, each would be a parser in every site's binary,
and rules in little's core, which keeps itself small, and leaves what only
some sites need to plugins
([VISION.md](https://github.com/withlittle/little/blob/main/VISION.md)).

Since [0046](./0046-plugin-hooks.md), plugins hook into each step of the
build, but none can read a format: little reads each page's text as
Markdown, whatever `source` gives back, and a page `files` adds is a
Markdown file or a notebook. A plugin could add a Markdown page for each
notebook, and leave the notebook out, but Markdown text is what 0024 turned
down: it ends HTML at a blank line, which pandas' tables have in their
`<style>`, and a code block a cell leaves open swallows the cells after it.
0024 built comrak's tree of the page instead, from cells each read on its
own.

## Options considered
- **Where formats are read:** in little, each built in, as Markdown and
  notebooks are; or in plugins, with Markdown the one format built in, as
  every site writes it, and every page a plugin adds is in it.
- **What a plugin gives back:** Markdown text, with 0024's problems;
  comrak's tree, which WIT can't carry without a type for each of comrak's
  nodes, tying plugins to comrak's version; or a list of parts, Markdown,
  HTML, and code, each read on its own, which little joins into comrak's
  tree, as it joined a notebook's cells.
- **Which files are a plugin's:** each file, offered to each plugin in
  turn; a glob; or an extension, as `code-blocks` claims a language, which
  is how little found `.md` and `.ipynb` files, and what a link to a page's
  file ends in.
- **What a plugin knows of the site:** its `markdown` settings, from the
  `config` hook, which a version from a git ref doesn't call
  ([0046](./0046-plugin-hooks.md)); or given with each file, so a plugin
  writes what they allow, like HTML only with `raw_html`.
- **Code:** a code block in Markdown, which would go to `code-blocks`
  hooks, as a notebook's code cells don't
  ([0029](./0029-plugins-that-render-code-blocks.md)), and needs a fence
  longer than any in the code; or a part of its own, written as a code
  block, but given to no hook.
- **A file a plugin can't read:** warned of, as for other hooks called for
  each page, and copied; or stop the build, as a notebook little couldn't
  read did.
- **A notebook no plugin reads:** a warning, which `--strict` fails on,
  though some sites offer notebooks to download; or copied, as before 0024.

## Decision
- `little:plugin` is 0.2.1, with a new hook, `formats`, so plugins built
  for 0.2.0 still load:

  ```wit
  interface formats {
      record code { language: string, text: string }
      variant part { markdown(string), html(string), code(code) }
      extensions: func() -> list<string>;
      read: func(page: string, text: string, markdown: string) -> result<list<part>, string>;
  }
  ```

- `extensions` is called once, after `setup`, for the extensions the plugin
  reads, like `ipynb`, matched as they're written, so `.IPYNB` isn't one.
  Two plugins that read one, `md`, which little reads, or one that isn't a
  word, like `.ipynb` or `tar.gz`, stop the build.
- A file in one, in the source or added by `files`, is a page, as a `.md`
  file is: `analysis.ipynb` is at `/analysis`, `README.ipynb` is its
  folder's page, a link to `analysis.ipynb#results` leads to the page, and
  the file isn't copied. `analysis.md` beside it stops the build, as pages
  that clash do ([0013](./0013-pages-that-clash.md)). A file in an
  extension no plugin reads is copied, as it is, without a warning.
- `read` is called for each, after `source`, with its file, its text, and
  the site's `markdown` settings, as JSON, as `little config` prints them.
  little joins the parts it gives back into comrak's tree of the page,
  which it writes as it writes Markdown's, with its table of contents,
  links, math, diagrams, highlighting, and search:
  - `markdown` is read on its own, with the site's extensions, and written
    as if the page's author had: its code blocks go to `code-blocks` hooks,
    its link references and footnotes are its own, its footnotes are
    numbered on through the page, and frontmatter at the top of the first
    part is the page's;
  - `html` is written as it is, whole, and stops the build with
    `raw_html: false`, as a `code-blocks` hook's does;
  - `code` is a code block in its language, highlighted, which no
    `code-blocks` hook renders, but one in `mermaid` is still drawn, and
    one in `math` written as math.
- An error, a crash, or a file that isn't UTF-8 stops the build, naming the
  file, unlike other hooks called for each page: a page little can't read
  has nothing to build from. A block a plugin can't render, a diagram, or a
  formula in a part is warned of without a line.
- Notebooks are read by a plugin, `jupyter`, in `plugins/jupyter/`, its own
  workspace, released from tags like `jupyter-v0.1.0`
  ([0052](./0052-releasing-plugins-from-tags.md)), with all that
  `src/notebook.rs` did but build the tree: format 4's types, each cell and
  output, frontmatter, attached images, terminal colours, and `\r`. A
  notebook's page is as 0024 built it, and the built-in theme still styles
  its `output` blocks and pandas' tables:
  - a Markdown cell, and an output in Markdown, is a `markdown` part, so
    its code blocks go to plugins; a code cell is a `code` part, so it
    doesn't, as before ([0029](./0029-plugins-that-render-code-blocks.md));
  - an output shows HTML or SVG only with `raw_html`, as before, and the
    next type it has otherwise;
  - it has no settings.
- `little init` keeps a `README.md`, in any case, as a site's home page,
  but not a `README.ipynb`: a new site has no plugins, so it writes a
  `README.md` beside it ([0057](./0057-starting-a-site-with-little-init.md)).
- The manual's page on notebooks is `docs/plugins/jupyter-notebooks.ipynb`,
  beside the other plugins' pages, and still a notebook, read by `jupyter`
  built from the same commit, as `rustdoc` is.

## Consequences
- little reads Markdown alone. A site with notebooks names `jupyter` in its
  `plugins`, or its notebooks are copied, as files to download; this breaks
  it, so little's next release raises its minor version
  ([0058](./0058-features-raise-the-minor-version.md)).
- AsciiDoc, reStructuredText, or Org-mode can each be a plugin, once a site
  needs one, without changing little, and a theme styles their pages as it
  styles Markdown's.
- little's core loses `src/notebook.rs`, and its own dependency on base64;
  `jupyter` is about 460 KB.
- Limits, for now:
  - a file a plugin reads must be UTF-8 text, so a binary format, like
    Word's `.docx`, can't be read;
  - an extension is matched as it's written, and read by one plugin;
  - what's warned of in a part has no line;
  - a part's link references and footnotes are its own;
  - 0024's limits are `jupyter`'s.
