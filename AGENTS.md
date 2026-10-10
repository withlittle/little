# AGENTS.md — working conventions for little

Read this before making changes. **little** is a static documentation site
generator written in Rust. Its mission is **authoring experience** — see
[`VISION.md`](./VISION.md).

## Commands
```sh
cargo run -- build          # build little's manual (docs/) into dist/, and check its links
cargo run -- build --strict # ...and fail on anything it warns of
cargo run -- serve          # live-reloading dev server at http://127.0.0.1:8080
cargo run -- config         # print the manual's merged config
cargo build --release       # produce the release binary (target/release/little)
cargo test                  # run tests (they build tests/plugin for wasm32-wasip2)
cargo clippy --all-targets  # lint — keep it clean
cargo fmt                   # format — run before committing

# the docstrings plugin, in its own workspace
cargo build --release --target wasm32-wasip2 --manifest-path plugins/docstrings/Cargo.toml
cargo test --manifest-path plugins/docstrings/Cargo.toml
cargo clippy --manifest-path plugins/docstrings/Cargo.toml --all-targets --target wasm32-wasip2
```

## Architecture (keep the core small)
- `src/main.rs` — CLI (clap): `build` (`--strict` fails on a broken link,
  a formula that isn't rendered, a diagram that isn't drawn, a code block
  a plugin can't render, or a plugin's warning), `serve`, `config`.
- `src/config.rs` — the 3-level cascading config (little → theme → site): the
  chain of themes a site uses, its source and destination folders, the levels
  of heading its tables of contents list, the host and port
  `little serve` serves it at, the `url` it's published at, the `repo` and
  `branch` its source is in, whether it's indexed for search, the extensions
  to Markdown its pages are written with, how their math is written, how
  their diagrams are drawn, and look, with Mermaid's own config, the
  classes their code is highlighted with, and the plugins that render their
  code blocks, or add pages, each with its `.wasm` file, the folders it
  reads, and its settings.
- `src/little.yml` — little's own defaults (level 1), compiled into the binary.
- `src/render.rs` — Markdown (comrak, with the extensions the site turns on) +
  YAML frontmatter → HTML fragment, with each relative link led from the
  folder of the file it's in, and each link to a page's `.md` or `.ipynb`
  file led to the page, noting each link as it's written. A notebook's tree
  is written the same way, once plugins have rendered its blocks and its
  diagrams are drawn.
- `src/plugin.rs` — plugins: WebAssembly components, run with wasmtime, built
  in, that export `wit/plugin.wit`. Starts each a build uses once, in a
  sandbox that reaches only the folders it reads, and gives it each fenced
  code block in its languages, before diagrams are drawn, putting the
  Markdown or HTML it gives back in the block's place; one it can't render
  stays code, and is noted, with its line, to warn of. Asks each that adds
  pages for them, and what it warns of, checking each page's path.
- `src/math.rs` — writes each formula as MathML as comrak writes the page,
  with math-core, each read on its own; one it can't read is written as its
  TeX, and noted, as it's written, to warn of.
- `src/mermaid.rs` — draws each Mermaid block in comrak's tree as SVG, with
  merman, before comrak writes the page, from the site's Mermaid config, over
  little's own, with the id `mermaid-1`, and so on, the same in each build;
  one it can't draw stays code, and is noted, with its line, to warn of.
- `src/notebook.rs` — a Jupyter notebook (`.ipynb`, format 4), read with
  serde into comrak's tree of its page: Markdown cells as Markdown, code
  cells as code blocks in the notebook's language, and their outputs as
  `output` code blocks, HTML, or images with `data:` URLs.
- `src/highlight.rs` — highlights each code block as comrak writes it, with
  bat's grammars, through two-face and syntect: a `<span>` with Pygments' or
  GitHub's class around each token, for the theme to colour.
- `src/site.rs` — build pipeline: walk the source (`docs/`), start the
  site's plugins, render its Markdown and notebooks, and the pages plugins
  add, as Markdown files at their paths, stopping on two that would be the
  same page, copy its other files, apply theme, index it for search, write
  the destination (`dist/`), noting the source file and URL of each file it
  writes. Finds where the source is in its repository, so each page can
  link to its file there, and works out when it's built, for `site.year`,
  its diagrams' today, and its plugins' clocks.
- `src/search.rs` — indexes each page's own content, under its title and URL,
  with Pagefind, in the language the theme gives the page, into the files of
  the destination's `pagefind/`.
- `src/links.rs` — checks the links in the built site with lychee-lib: those
  within the site in the destination, as the site is published under its
  `url`, and, for `little build`, those to other sites over the network, but
  not those into the site's repository. Shows a broken link in Markdown as
  it's written.
- `src/theme.rs` — a theme's templates, static files, and Sass, layered over
  those of the themes it builds on, and the built-in theme; picks each page's
  template by its type (`home`, `section`, `page`, `404`), and gives templates
  the `url` and `absolute_url` filters. A theme is built in, in a folder, or
  downloaded.
- `src/sass.rs` — compiles a stylesheet in a theme's `_static/` to CSS with
  grass, loading only from the theme's `_sass/`, and names the real file of
  each error and warning.
- `src/download.rs` — a theme downloaded from a URL: fetched over HTTPS,
  unpacked from its `.zip` or `.tar.gz`, and kept in the site's `.little/`, so
  it's downloaded only once; and a plugin's `.wasm` file, kept in
  `.little/plugins/`.
- `src/server.rs` — dev server + file watcher + live reload, watching the
  config, source, themes, plugins, and the folders they read; checks the
  links within the site after each build.

Content and theme live outside `src/`:
- `docs/` — little's manual, for people who use little, published at
  <https://withlittle.github.io/little>. It's a little site, so each page
  uses what it documents, where it can, and the README links to it. Pages
  link to each other by their files, and to little's own files on GitHub.
  Meta docs, like ADRs, don't go here.
- `theme/` — little's built-in theme, compiled into the binary: `_templates/`
  (MiniJinja; `default.html` is required) + `_static/` (copied to the site's
  root) + an optional `little.yml`. It's plain CSS, in cascade layers, with
  a script that only adds to it: `builtin` has no `_sass/`.
  List every file in it in `builtin` in `src/theme.rs`; a test checks. Its
  search box is Pagefind's, deferred so it finds the `pagefind/` little
  writes.
- `little.yml` — the manual's config: its `url`, `repo`, and `nav`, which
  orders its pages by hand. It sets `theme: ./theme`, so edits to the
  built-in theme live-reload.
- `wit/plugin.wit` — the interfaces a plugin exports, which little's bindings
  are generated from; plugin authors copy it. `world plugin` renders code
  blocks; `world plugin-with-pages` adds pages, too.
- `tests/plugin/` — a plugin, in Rust, with its own workspace, that the tests
  build for `wasm32-wasip2` (which `rust-toolchain.toml` asks rustup for) and
  drive, block by block, to check what plugins can and can't do; with its
  `pages` feature, it's built for `plugin-with-pages`, and adds the pages its
  settings name.
- `plugins/docstrings/` — the `docstrings` plugin, a crate in its own
  workspace, built for `wasm32-wasip2`, that adds an API reference: a page
  for each Python module in the folders it reads, from its docstrings, in
  NumPy's, Google's, or Sphinx's style, parsed with Ruff's parser. Its tests
  run natively; only its glue to `wit/plugin.wit` needs WebAssembly.
- Project meta (`VISION.md`, `AGENTS.md`, `decisions/`) lives at the repo root.

## Core principles (do not violate)
1. **The theme owns 100% of the markup.** The core never emits baked-in HTML; it
   passes structured `site` / `page` / `nav` data to the theme. Keep it that way.
2. **Config cascades: little → theme → site, last writer wins.** New config
   belongs in this cascade. Only settings little itself owns get a built-in
   default in `src/little.yml`; a site's name, language, and look are the
   theme's to default (in its `little.yml`) and the site's to set. The theme's
   and site's `little.yml` are optional — a missing or empty file just
   contributes nothing. `theme` is the exception: it chooses level 2, and a
   site that doesn't set it gets little's built-in theme (`theme: true`). A
   theme that sets it builds on that theme, whose config goes beneath its own.
3. **Keep the core small; push features to plugins.** Sphinx-style API extraction,
   niche renderers, etc. are plugins (WASM components that render code
   blocks, or add pages), not core code.
4. **No mandatory Node toolchain.** Never add a build step that requires Node.
5. **Authoring experience wins ties.** When unsure, pick what makes writing docs
   simpler.

## Working rules
- **Agree the scope first.** Before building anything beyond a small, obvious
  change, say briefly what you'll change, list the edge cases you've found and
  how you'd handle each, and wait for a go-ahead. Don't fix edge cases nobody
  asked about.
- **Handle the common case.** For a rare edge case, prefer a clear error or a
  documented limitation over new code.

## Conventions
- Errors: `anyhow::Result` at boundaries; add context with `.context()`.
- Format with `cargo fmt`; keep `cargo clippy` clean.
- Reach for a well-maintained crate before writing our own code; justify a
  heavy dependency in an ADR.
- Comment the *why*, not the *what*; don't over-comment.
- Add unit tests for parsing/merging logic (frontmatter, config cascade).
- A change people using little can see updates its page in the manual
  (`docs/`), and the reference pages, in the same PR; a new page goes in
  `nav` in `little.yml`. Keep the README short: it links to the manual.
  `cargo run -- build --strict` checks the manual's links.
- Commits: imperative subject; include the Copilot co-author trailer.

## Decisions
Record notable choices as short ADRs in `decisions/`. Locked so far:
- **Language:** Rust — see [`decisions/0001-language-rust.md`](./decisions/0001-language-rust.md).
- **Markdown:** comrak. **Templating:** minijinja. **Config:** cascading YAML.
- **Themes:** `_templates/` + `_static/`, a template per type of page — see
  [`decisions/0002-theme-structure.md`](./decisions/0002-theme-structure.md).
- **Choosing themes:** `theme` is `true` (built-in), `false`, a folder, or a
  URL, and a theme can build on another, file by file — see
  [`decisions/0003-choosing-and-layering-themes.md`](./decisions/0003-choosing-and-layering-themes.md).
- **Downloading themes:** an HTTPS `.zip` or `.tar.gz`, unpacked into the
  site's `.little/` and never downloaded again; links and paths that leave the
  archive are refused — see
  [`decisions/0004-downloading-themes.md`](./decisions/0004-downloading-themes.md).
- **Previous and next pages:** `page.prev` and `page.next` follow `site.nav`
  from top to bottom, and are left out at either end — see
  [`decisions/0005-previous-and-next-pages.md`](./decisions/0005-previous-and-next-pages.md).
- **Frontmatter and config win:** every frontmatter key reaches `page` and
  merges over what little works out; a `nav` in `little.yml` replaces
  little's — see
  [`decisions/0006-frontmatter-and-config-win.md`](./decisions/0006-frontmatter-and-config-win.md).
- **A page's table of contents:** `page.toc` is shaped like `site.nav`,
  `{ title, url, children }`, with each `###` under the `##` above it and
  each heading's `#fragment` as its `url` — see
  [`decisions/0007-page-nav-shaped-like-site-nav.md`](./decisions/0007-page-nav-shaped-like-site-nav.md).
- **Naming a page's navigation:** name each for what readers see, since
  authors write the same names in frontmatter: `page.toc`, not `page.nav`,
  beside `page.prev` and `page.next` — see
  [`decisions/0008-page-toc.md`](./decisions/0008-page-toc.md).
- **Which headings a page's table of contents lists:** the levels from
  `toc_min_depth` to `toc_max_depth`, which cascade like any config: little
  sets 2 to 6, every heading but the page's `#` title, and the built-in theme
  a max of 3 — see
  [`decisions/0009-toc-depth-in-config.md`](./decisions/0009-toc-depth-in-config.md).
- **Breadcrumbs:** `page.breadcrumbs` lists the items above a page in
  `site.nav`, from the top down, each `{ title, url }`: the home page, if it's
  at the nav's top, then the sections the page is in, but not the page
  itself — see [`decisions/0010-breadcrumbs.md`](./decisions/0010-breadcrumbs.md).
- **Where `little serve` serves:** `host` and `port`, which cascade like any
  config: little sets `127.0.0.1` and `8080`, and `little serve --host` and
  `--port` override them for a single run — see
  [`decisions/0011-serve-host-and-port-in-config.md`](./decisions/0011-serve-host-and-port-in-config.md).
- **Hidden files:** a file or folder in the source whose name starts with `.`
  is left out, and saving it doesn't rebuild in `little serve`; only names
  inside the source count, so it can be hidden itself, and a theme's
  `_static/` is still copied whole — see
  [`decisions/0012-hidden-files.md`](./decisions/0012-hidden-files.md).
- **Pages that clash:** two files in the source that would be the same page,
  like `guide.md` and `guide/README.md`, stop the build before any page is
  rendered, with an error that names both; URLs that differ only in case
  aren't checked — see
  [`decisions/0013-pages-that-clash.md`](./decisions/0013-pages-that-clash.md).
- **Publishing under a path:** a site's `url`, like
  `https://withlittle.github.io/little`, puts its path before every URL
  little gives the theme and every link in Markdown, and `little serve`
  serves the site under it; a theme writes its own URLs through the `url`
  filter, and makes them absolute with `absolute_url` — see
  [`decisions/0014-site-url-and-its-path.md`](./decisions/0014-site-url-and-its-path.md).
- **Files and relative links:** every file in the source that isn't hidden or
  Markdown is copied to the same path in the destination, over a theme's file
  at that path, but for a destination inside the source; a relative link or
  image leads from the folder of the file it's in, as on GitHub, whatever URL
  its page is served at — see
  [`decisions/0015-files-and-relative-links.md`](./decisions/0015-files-and-relative-links.md).
- **Checking links:** `little build` checks every link in the built site
  with lychee-lib, built in, and warns of each broken page, file, `#fragment`,
  or link to another site, naming the file it's in; `--strict` fails on one,
  and `little serve` checks only links within the site — see
  [`decisions/0016-checking-links.md`](./decisions/0016-checking-links.md).
- **Links to pages' files:** a link in Markdown to a `.md` file, like
  `install.md#usage` or `guide/README.md`, leads to the page built from it,
  as it leads to the file on GitHub, even one that isn't there, so it's
  warned of; a warning shows a broken link in Markdown as it's written, and
  one in the theme or a copied file as a reader follows it — see
  [`decisions/0017-links-to-pages-files.md`](./decisions/0017-links-to-pages-files.md).
- **Sass in themes:** a `.scss` or `.sass` file in a theme's `_static/` is
  compiled with grass, built in, to the CSS file at its path, loading only
  from the theme's `_sass/`, which isn't published and layers like
  `_templates/`; a partial in `_static/` stops the build, and Sass in the
  source is copied as-is — see
  [`decisions/0018-sass-in-themes.md`](./decisions/0018-sass-in-themes.md).
- **Site search:** `search`, which little sets `false` and the built-in
  theme `true`, indexes each page's own content, under its title and URL,
  with Pagefind, built in, into the destination's `pagefind/`, in the language
  of the page's `<html lang>`, but not the 404 page; the theme owns the
  search box, and a page or file where search writes one stops the build —
  see [`decisions/0019-site-search.md`](./decisions/0019-site-search.md).
- **Where search's files go:** `pagefind/`, Pagefind's default, and the
  built-in theme loads Pagefind's search box with `defer`, not as a module,
  so it finds them itself, even under the path of the site's `url`; each
  page is indexed at its URL from the root of the site, and Pagefind puts
  the site's path back, from where it finds them — see
  [`decisions/0021-search-files-in-pagefind.md`](./decisions/0021-search-files-in-pagefind.md).
- **Linking to a page's file:** `repo`, the repository's address as a
  browser shows it, reaches the theme as `site.repo`, and, on GitHub and
  GitLab, gives each page `page.source_url` and `page.edit_url`, on `branch`,
  which little sets to `main`, with each path from the nearest folder with a
  `.git`, else the site's folder; links into the repository aren't
  checked — see
  [`decisions/0020-links-to-a-pages-file-in-its-repo.md`](./decisions/0020-links-to-a-pages-file-in-its-repo.md).
- **Markdown's extensions:** `markdown` turns each of comrak's extensions
  to what an author can write on or off, under little's own names, like
  `emoji` and `raw_html`, and cascades like any config: little turns on
  GitHub's and leaves the rest off. Callouts are GitHub's alerts, in GitHub's
  markup — see
  [`decisions/0022-markdown-extensions.md`](./decisions/0022-markdown-extensions.md).
- **Highlighting code:** little highlights each code block as it builds the
  page, with bat's grammars, built in through two-face, in the classes
  `markdown.highlight` chooses: `pygments`, little's default, `github`, or
  `false`, for none. A highlighted block is a `<pre class="highlight">`, with
  a flat `<span>` for each token with a class; a language bat doesn't have
  stays plain — see
  [`decisions/0023-highlighting-code.md`](./decisions/0023-highlighting-code.md).
- **Jupyter notebooks:** each `.ipynb` file in the source is a page, as a
  `.md` file is, read in format 4 with little's own serde types, not the
  stricter `nbformat` crate, and built into comrak's tree, so it's written as
  Markdown is. Outputs are code blocks in `output`, HTML and SVG when
  `raw_html` is on, or images with `data:` URLs; frontmatter is in a raw or
  Markdown cell at the top — see
  [`decisions/0024-jupyter-notebooks.md`](./decisions/0024-jupyter-notebooks.md).
- **Rendering math:** `markdown.math` writes math as `mathml`, little's
  default, rendered with math-core, built in, as each page is built; `tex`,
  marked with `data-math-style` for a theme's script; or `false`, for none,
  and `true` is an error. Each formula is read on its own, and one that
  can't be is written as TeX and warned of, as a broken link is; a heading's
  math is TeX in `page.toc` — see
  [`decisions/0025-rendering-math-as-mathml.md`](./decisions/0025-rendering-math-as-mathml.md).
- **The year a site is built:** `site.year`, for a theme's copyright notice,
  is the year the build starts in, in UTC, or that of `SOURCE_DATE_EPOCH`,
  for reproducible builds; it's worked out once, beneath the config, like
  `site.nav`, and the built-in theme shows no notice — see
  [`decisions/0026-the-year-a-site-is-built.md`](./decisions/0026-the-year-a-site-is-built.md).
- **Drawing diagrams:** `markdown.mermaid` writes a ```` ```mermaid ````
  block as `svg`, little's default, an `<svg>` drawn with merman, built in,
  as each page is built, with the id `mermaid-N`, the same in each build; or
  `false`, as code, for a theme's script. One merman can't read is code, and
  warned of, as a broken link is. A diagram can carry HTML, so `svg` with
  `raw_html: false` stops the build — see
  [`decisions/0027-drawing-mermaid-diagrams.md`](./decisions/0027-drawing-mermaid-diagrams.md).
- **How diagrams look:** `mermaid` holds Mermaid's own config, in Mermaid's
  own names, like `theme` and `themeVariables`, which each diagram little
  draws starts from, as a site passes it to `mermaid.initialize()`. It
  cascades like any config, but little sets none, keeping its seeds and a
  gantt chart's width in code, beneath it; a diagram's own config goes over
  it. little checks only that `theme` is one of Mermaid's and that no
  setting is empty, as an unquoted `#` colour is — see
  [`decisions/0028-how-diagrams-look.md`](./decisions/0028-how-diagrams-look.md).
- **Plugins that render code blocks:** `plugins` names each plugin and its
  `.wasm` file, a WebAssembly component built for WASI 0.2 that exports
  `wit/plugin.wit`, by a path or an HTTPS URL, downloaded once into
  `.little/plugins/`, and cascades plugin by plugin and setting by setting;
  only the site can set `read`, the folders a plugin reads. A fenced block in
  a plugin's language goes to it, before diagrams are drawn, and the Markdown
  it gives back is written as the author's, or the HTML as it is. A plugin
  runs in wasmtime's sandbox, with no network, environment, or files but
  those it reads, a clock at the build's start, and the same random numbers
  each build; a block it can't render is warned of, as a broken link is —
  see
  [`decisions/0029-plugins-that-render-code-blocks.md`](./decisions/0029-plugins-that-render-code-blocks.md).
- **Plugins that add pages:** a plugin built for `world plugin-with-pages`
  exports `pages`, which little calls once each build, after `setup`,
  before any page is built. Each page is built as if it were a Markdown file
  at its `path` in the source, relative, ending in `.md`, but has no file in
  the repository; one that would be the same page as a file or another
  plugin's page stops the build, as does an error from `pages`, or a crash.
  Its warnings are warned of, and `--strict` fails on them — see
  [`decisions/0030-plugins-that-add-pages.md`](./decisions/0030-plugins-that-add-pages.md).
- **A plugin for Python's docstrings:** `plugins/docstrings/`, in little's
  repository, in its own workspace, adds a page for each module and package
  in the folders it reads, under `url`, `/reference` by default, like
  mkdocstrings', parsing Python with Ruff's parser, without running it, and
  finding each docstring's style, NumPy's, Google's, or Sphinx's, for
  itself. A module it can't parse is warned of, and left out — see
  [`decisions/0031-a-plugin-for-pythons-docstrings.md`](./decisions/0031-a-plugin-for-pythons-docstrings.md).
- **Docs are the manual:** `docs/` is little's manual, published at
  `https://withlittle.github.io/little`, grouped by what a reader is trying
  to do, and ordered by `nav` in `little.yml`; each page uses what it
  documents, and the README points to it — see
  [`decisions/0032-docs-are-the-manual.md`](./decisions/0032-docs-are-the-manual.md).
- **The built-in theme's design:** plain CSS, with no framework or Node, in
  cascade layers, `little.tokens` to `little.print`, so CSS that isn't in a
  layer wins over it, but for a few `!important`s, and it works without a
  script. Its colours are
  tokens, each a `light-dark()` pair, light or dark as the reader's system
  is. Below 60rem, the nav is a popover drawer, and below 75rem, the table
  of contents folds up above the page. It has a skip link, landmarks, a
  visible focus ring, AA contrast, underlined links, and styles for reduced
  motion, forced colours, and print, and needs browsers from 2024 — see
  [`decisions/0033-the-built-in-themes-design.md`](./decisions/0033-the-built-in-themes-design.md).
- **The built-in theme's script:** `assets/script.js`, deferred, only adds
  to the theme, so every page works without it. A button in the header
  chooses light or dark, kept in `localStorage` as `little-theme`, and
  forgotten when it's what the system shows, which a few lines inline at the
  top of `<head>` apply before the page is drawn. It closes the drawer when
  the window widens, folds the table of contents above the page once a link
  in it is picked, adds a button to copy each code block, where the browser
  has a clipboard, but not to a notebook's output, and marks where the
  reader is in the table of contents beside the page with `data-current`.
  It uses nothing newer than Baseline 2024, so not the `popover`
  property — see
  [`decisions/0034-the-built-in-themes-script.md`](./decisions/0034-the-built-in-themes-script.md).
