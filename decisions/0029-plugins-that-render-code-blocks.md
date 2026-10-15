# 0029 — Plugins that render code blocks

- Status: accepted; amended by [0030](./0030-plugins-that-add-pages.md),
  which lets a plugin add pages, too, exporting `pages` in a second world,
  `plugin-with-pages`, by [0045](./0045-limits-on-downloads.md), which
  stops a plugin's download that takes too long or is too big, and by
  [0046](./0046-plugin-hooks.md), which makes rendering code blocks one of a
  plugin's hooks, `code-blocks`, with its languages from `languages`, not
  `setup`, in `little:plugin@0.2.0`
- Date: 2026-10-07
- Amends: [0004](./0004-downloading-themes.md),
  [0016](./0016-checking-links.md),
  [0022](./0022-markdown-extensions.md),
  [0024](./0024-jupyter-notebooks.md),
  [0027](./0027-drawing-mermaid-diagrams.md)

## Context
little keeps its core small, and leaves what only some sites need to
plugins ([VISION.md](../VISION.md)): API docs from docstrings, diagrams in
languages other than Mermaid, like Graphviz, D2, and PlantUML
([0027](./0027-drawing-mermaid-diagrams.md)), charts, and tables from data.
little can't load a plugin, so each of these would be built into little,
or loaded by a theme, as a script in each reader's browser.

Most of them start where a diagram does: an author writes a fenced code
block, in a language, and a reader should see something else in its place.
GitHub draws ```` ```mermaid ````, ```` ```geojson ````, and
```` ```stl ```` blocks this way, and mkdocstrings puts a module's API docs
where a page names the module.

## Options considered
- **What a plugin is:**
  - a native library, built in Rust, that little loads: the fastest, but
    built again for each OS and CPU, and for each version of little, as
    Rust has no stable ABI. It can do anything little can, and its crash is
    little's;
  - a program little runs, as mdBook runs its preprocessors, which reads a
    page as JSON and writes it back: written in any language, but it and
    its runtime must be installed on each machine that builds the site, CI
    too, and it can do anything its user can;
  - a WebAssembly component, which little runs: one `.wasm` file for every
    OS and CPU, which keeps working as little changes, until its interface
    does, in a sandbox that reaches only what little gives it, and whose
    crash little catches. Rust, Python, Go, JavaScript, and C build to one.
    dprint, Typst, and Zed load plugins as WebAssembly.
- **What runs a component:**
  - wasmtime, the Bytecode Alliance's runtime, which compiles it to machine
    code with Cranelift, and passes strings, records, and results across
    its interface: about 20 MB, on its own;
  - wasmtime with Winch, its quicker compiler: 18 MB, and no quicker to
    start a small plugin;
  - wasmi, an interpreter: 4 MB, but it runs only core modules, which pass
    numbers, so little and each plugin, in each language, would agree by
    hand how to pass a string;
  - Extism, a framework over wasmtime: 22 MB, with its own way of passing
    data rather than components, on an older wasmtime.
- **What a plugin can change:** a fenced code block; a page's Markdown
  before it's read; the site's pages, adding some, like an API reference;
  the built HTML; or the theme's filters. A code block is how an author
  already asks for a diagram, and a plugin that gives back Markdown gets
  all little does for Markdown, so it covers most plugins, with the least
  to learn. Others can come in later versions of the interface.
- **What a plugin gives back:** HTML, which little writes as it is; or
  Markdown, which little writes as if the author had, so its headings,
  links, math, diagrams, and code are little's own. A diagram needs HTML;
  most else is better as Markdown.
- **What a plugin can reach:** its block alone, or files too, as an API
  reference reads the code it documents. The network would make a build
  differ from one day to the next, and fail offline, and would let a
  plugin send a site's source away.
- **Where plugins come from:** a registry, which little would run and
  police; or a URL, like a theme's ([0004](./0004-downloading-themes.md)),
  so a plugin is one `.wasm` file published anywhere, like a GitHub
  release's, as dprint's plugins are.
- **A block a plugin can't render:** stop the build, which stops
  `little serve` too; or warn, as of a diagram that can't be drawn, and
  leave the block for little.

## Decision
- `plugins`, in a site's or a theme's `little.yml`, names each plugin and
  its `.wasm` file, by a path from the `little.yml`'s folder or an
  `https://` URL; `http://` is an error. A plugin can have settings beside
  `wasm`, which it reads itself:

  ```yaml
  plugins:
    bob: https://example.com/svgbob-1.0.wasm
    python:
      wasm: ./plugins/little_python.wasm
      read: src
      style: google
  ```
- It cascades like any config, plugin by plugin and setting by setting, so
  a site changes its theme's plugin's settings, and turns one off with
  `python: false`. little sets none.
- `read` is a folder, or a list of them, from the site's folder, that the
  plugin can read, but not change. Only the site's `little.yml` can set it,
  so a theme's plugin reads a site's files only when the site says so. One
  that isn't a folder stops the build.
- The first build that needs a plugin from a URL downloads it into
  `.little/plugins/`, and little never downloads it again, as for a theme.
  To use another version, change the URL.
- A plugin is a WebAssembly component built for WASI 0.2, as Rust's
  `wasm32-wasip2` target builds one, which exports the interface in
  `wit/plugin.wit`:

  ```wit
  package little:plugin@0.1.0;

  interface code-blocks {
      record block { language: string, info: string, text: string, page: string }
      variant output { markdown(string), html(string) }
      setup: func(settings: string) -> result<list<string>, string>;
      render: func(block: block) -> result<output, string>;
  }

  world plugin { export code-blocks; }
  ```
  - `setup`, given the plugin's settings as a JSON object, without `wasm`,
    and with `read` as a list, gives back the languages it renders, each
    one word. little calls it as each build starts, before any page is
    built, and an error stops the build.
  - `render`, given a block's language, its info string, its text, and its
    page's file, from the site's folder, like `docs/guide.md`, gives back
    Markdown or HTML, or an error.
- A fenced code block whose language, the first word after the fence, up
  to a comma, in any case, as for highlighting, is a plugin's goes to that
  plugin, before little draws diagrams or renders math. Two plugins that
  render a language stop the build, naming both.
- Markdown a plugin gives back takes the block's place, as if its author
  had written it: its headings join `page.toc`, its links are led and
  checked, its math is rendered, its diagrams drawn, and its code
  highlighted. It isn't given to plugins again. HTML is written as it is,
  so, like raw HTML, it stops the build with `raw_html: false`.
- A block a plugin can't render, as it gives back an error or crashes, is
  warned of, as a diagram is, like
  ``warning: plugin `bob` can't render the block on line 12 of ./docs/guide.md: …``,
  and left as it was, so a ```` ```mermaid ```` block is still drawn, and
  any other is shown as code. `little build --strict` fails on one. After
  a crash, little starts the plugin again, and calls `setup`, for the next
  block.
- The blocks in a notebook's Markdown cells, and in its cells' Markdown
  outputs, go to plugins too ([0024](./0024-jupyter-notebooks.md)), and
  are warned of without a line; its code cells, and their other outputs,
  don't.
- A plugin reaches nothing little doesn't give it: no network, no
  environment variables, and no files but its `read` folders, at their
  paths from the site's folder, like `src/app.py`. What it prints goes to
  little's stderr. So a site builds the same each time, a plugin's clock
  reads the time the build started, as for `site.year`
  ([0026](./0026-the-year-a-site-is-built.md)), and its random numbers are
  the same in each build.
- A build starts each plugin once, and the plugin keeps its state from
  block to block and page to page. `little serve` compiles each plugin
  once, and again when its file changes, and rebuilds when a plugin's file
  or a `read` folder changes.
- New crates:
  - `wasmtime`, with Cranelift and components, and `wasmtime-wasi`, for
    WASI 0.2;
  - `rand`, which `wasmtime-wasi` already builds, to seed plugins' random
    numbers.

## Consequences
- Diagrams in other languages, charts, API references, and more can be
  plugins, written in Rust, Python, or any language that builds a WASI
  component, without changing little or a theme.
- One `.wasm` file runs wherever little does, and keeps working as little
  changes, until the interface's version does.
- A site whose plugins come from URLs builds offline once they're
  downloaded, as with themes.
- little grows by about 17 MB, to about 69 MB, and by about 60 crates, for
  wasmtime, Cranelift, and WASI.
- A small plugin in Rust, like one of 77 KB that renders CSV as a table,
  adds about 10 ms to a build. The same plugin in Python carries Python
  with it, in 18 MB, and adds about a second, as wasmtime compiles it.
  Either renders 200 tables as quickly as one.
- little's tests build a plugin, so running them needs Rust's
  `wasm32-wasip2` target, which `rust-toolchain.toml` asks rustup for.
- Limits, for now:
  - plugins render code blocks, and nothing else;
  - a plugin has no time or memory limit, so one that never returns stops
    the build until you press Ctrl-C;
  - a download isn't checked against a hash, so little trusts the server
    behind the URL;
  - each `little build` compiles each plugin again, as compiled plugins
    aren't kept;
  - a plugin's Markdown is read on its own, so it can't use the page's
    link references, nor have footnotes;
  - a page can't change a plugin's settings in its frontmatter, but a
    block can pass it options in its info string, which the plugin reads.
