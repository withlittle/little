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

# the doc_comments plugin, which the manual uses: build it before the manual
cargo build --release --target wasm32-wasip2 --manifest-path plugins/doc_comments/Cargo.toml
cargo test --manifest-path plugins/doc_comments/Cargo.toml
cargo clippy --manifest-path plugins/doc_comments/Cargo.toml --all-targets --target wasm32-wasip2

# the docstrings plugin, in its own workspace
cargo build --release --target wasm32-wasip2 --manifest-path plugins/docstrings/Cargo.toml
cargo test --manifest-path plugins/docstrings/Cargo.toml
cargo clippy --manifest-path plugins/docstrings/Cargo.toml --all-targets --target wasm32-wasip2

# the library they share, in its own workspace
cargo test --manifest-path plugins/api_reference/Cargo.toml
cargo clippy --manifest-path plugins/api_reference/Cargo.toml --all-targets
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
  reads, and its settings; and the versions it's built in, each with its
  title and URL, one at `/`, and each from a git ref but one, with the
  folder its pages are in there, and the config of each: the site's, but
  for its own `source`, `nav`, and plugins' settings, with its links to the
  theme's files led to the root.
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
  its diagrams' today, and its plugins' clocks. Builds each of the site's
  versions this way, with the site's config and theme, each in the folder
  of its URL, one at the root, which alone has the theme's files, and those
  from git refs from their pages, and the folders their plugins read,
  there, checked out with `src/git.rs`; stops on a page of the one at the
  root where another version goes; and gives the theme `site.versions`,
  `site.version`, and `page.versions`.
- `src/git.rs` — the git repository a site's in, found with the `git`
  command, and the files a build of a version reads from a git ref: its
  `little.yml`, source, and the folders its plugins read, checked out as
  `git checkout` would, into a temporary folder, deleted after the build.
- `src/search.rs` — indexes each page's own content, under its title and URL,
  with Pagefind, in the language the theme gives the page, into the files of
  the destination's `pagefind/`.
- `src/links.rs` — checks the links in the built site with lychee-lib: those
  within the site in the destination, as the site is published under its
  `url`, and, for `little build`, those to other sites over the network, but
  not those into the site's repository, nor those of a version from a git
  ref. Shows a broken link in Markdown as it's written.
- `src/theme.rs` — a theme's templates, static files, and Sass, layered over
  those of the themes it builds on, and the built-in theme; picks each page's
  template by its type (`home`, `section`, `page`, `404`), and gives templates
  the `url` and `absolute_url` filters. A theme is built in, in a folder, or
  downloaded.
- `src/sass.rs` — compiles a stylesheet in a theme's `_static/` to CSS with
  grass, loading only from the theme's `_sass/`, and names the real file of
  each error and warning.
- `src/download.rs` — a theme downloaded from a URL: fetched over HTTPS,
  within limits of time and size, unpacked from its `.zip` or `.tar.gz`, up
  to 1 GB, and kept in the site's `.little/`, so it's downloaded only once;
  and a plugin's `.wasm` file, kept in `.little/plugins/`.
- `src/server.rs` — dev server + file watcher + live reload, watching the
  config, source, themes, plugins, and the folders they read; builds every
  version again on each change, but doesn't watch git refs; checks the
  links within the site after each build.

Content and theme live outside `src/`:
- `docs/` — little's manual, for people who use little, published at
  <https://withlittle.github.io/little>. It's a little site, so each page
  uses what it documents, where it can, and the README links to it. Pages
  link to each other by their files, and to little's own files on GitHub.
  Meta docs, like ADRs, don't go here. little's own API, which the
  `doc_comments` plugin adds from `src/`'s doc comments, is at `/api/`, with
  no file in `docs/`.
- `theme/` — little's built-in theme, compiled into the binary: `_templates/`
  (MiniJinja; `default.html` is required) + `_static/` (copied to the site's
  root) + an optional `little.yml`. It's plain CSS, in cascade layers, with
  a script that only adds to it: `builtin` has no `_sass/`.
  List every file in it in `builtin` in `src/theme.rs`; a test checks. Its
  search box is Pagefind's, deferred so it finds the `pagefind/` little
  writes. Its header has a menu of a site's versions, when it has two or
  more.
- `little.yml` — the manual's config: its `url`, `repo`, and `nav`, which
  orders its pages by hand, and the `doc_comments` plugin, which documents
  `src/` at `/api/`, from where Cargo builds its `.wasm` file. It sets
  `theme: ./theme`, so edits to the built-in theme live-reload.
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
- `plugins/doc_comments/` — the `doc_comments` plugin, a crate in its own
  workspace, built for `wasm32-wasip2`, that adds an API reference: a page
  for a Rust crate and each of its modules, from their doc comments, parsed
  with syn, and with intra-doc links led to their items. The manual uses it
  on `src/`. Its tests run natively, as docstrings' do.
- `plugins/api_reference/` — a library, in its own workspace, not a plugin,
  that the plugins that add an API reference share: what a page documents,
  in no language in particular, and the Markdown of the page.
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
- Doc comments in `src/` are published, as little's API at `/api/` in the
  manual: give each module a `//!`, and each `pub` item a `///`, in the
  manual's plain style, and link to items by path, like ``[`Config`]``.
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
- **Numbers in the nav's order:** little sorts each folder's pages and
  folders by title, whatever their case, with each run of digits as a
  number, by its value, with alphanumeric-sort, 1.5.8 or later, for a total
  order: `2.` before `10.`, and `1` before `01`. A `nav` in `little.yml` is
  kept as it's written — see
  [`decisions/0035-numbers-in-the-navs-order.md`](./decisions/0035-numbers-in-the-navs-order.md).
- **Folding the built-in theme's nav:** each folder within a top-level one
  is a `<details>`, folded, but for those the current page is in, which the
  template finds from the `aria-current="page"` in its items' HTML, so
  little's data doesn't change. A folder with a page folds from a button
  beside its link. The script scrolls only the sidebar to the current page;
  which folders are open isn't remembered — see
  [`decisions/0036-folding-the-built-in-themes-nav.md`](./decisions/0036-folding-the-built-in-themes-nav.md).
- **Wrapping long names in the built-in theme:** a title with no spaces, a
  name, like `enable_iterative_imputer`, gets a `<wbr>` after each `_` and
  `.`, from the template's `breakable` macro, in the sidebar, tables of
  contents, breadcrumbs, and pager, so it wraps at its joints; a title with
  spaces wraps at them. Each also has `overflow-wrap: anywhere`, so nothing
  runs out of its box; `<title>` and hidden names are as they were — see
  [`decisions/0037-wrapping-long-names-in-the-built-in-theme.md`](./decisions/0037-wrapping-long-names-in-the-built-in-theme.md).
- **The built-in theme's page and tables:** from 75rem, on a site with a
  nav, a page without a table of contents keeps its empty column, so the
  text starts in the same place on every page. A table, but a `.dataframe`,
  is as wide as the page from the first paint, with `.js`, and the script
  puts each in a `<div class="table-scroll">`, which scrolls one too wide
  and takes its frame, so nothing moves; without the script, a table is as
  wide as its columns, as before. A column aligned in Markdown keeps its
  alignment — see
  [`decisions/0038-the-built-in-themes-page-and-tables.md`](./decisions/0038-the-built-in-themes-page-and-tables.md).
- **A long site name in the built-in theme's header:** a name too long for
  one line, in the header or the drawer, wraps onto a second, balanced,
  and is cut short with an ellipsis past that, with
  `-webkit-line-clamp: 2`, so the buttons beside it stay on the screen. The
  description has only the room the name leaves, and an icon button
  doesn't shrink; a name that fits on one line looks as it did — see
  [`decisions/0039-a-long-site-name-in-the-built-in-themes-header.md`](./decisions/0039-a-long-site-name-in-the-built-in-themes-header.md).
- **Versions:** `versions` lists a site's versions, the latest first, each
  with the git `ref`, a tag, branch, or commit, it's read from, but for at
  most one, the site's files as they are. `little build` builds them all,
  each in its folder, with its own search index, in the site's theme, by
  today's little, each from its ref, read with `git`. The theme gets
  `site.versions`, `site.version`, and `page.versions`, the same page in
  each version, else its home, and the built-in theme has a menu of them in
  its header. A shallow clone stops the build; a version from a ref's links
  to other sites aren't checked — see
  [`decisions/0040-versions-from-git-refs.md`](./decisions/0040-versions-from-git-refs.md).
- **A version's title and URL:** each version has a `title`, what the menu
  shows, like `2.0 (Alpha)`, and a `url`, where it's published from the
  root of the site, like `/1.4` or `/versions/1.4`, as the nav's items do,
  in place of a `name`. Exactly one is at `/`, whatever its place in the
  list, which is the menu's order; others can be in its folders, but not in
  each other's. The one at `/` has the 404 page, and its pages are checked
  against the others' folders. `site.version` is the title, and each entry
  in `site.versions` and `page.versions` has `root`, true for the one at
  `/` — see
  [`decisions/0041-version-titles-and-urls.md`](./decisions/0041-version-titles-and-urls.md).
- **What versions share:** every version is built with the site's config,
  theme, and plugins, as the site's `little.yml` sets them; one from a ref
  has its own pages and files, nav, and the folders its plugins read, from
  there. Of its ref's `little.yml`, if it has one, only `source` and `nav`
  count, and its entry's `source` wins over both, for docs that have
  moved. The theme's files are written once, with the version at `/`, and
  every version links to them there, but to a file of its own at the same
  path — see
  [`decisions/0042-versions-share-the-sites-config-and-theme-files.md`](./decisions/0042-versions-share-the-sites-config-and-theme-files.md).
- **A version's own nav and plugins:** an entry in `versions` with a `ref`
  can set its own `nav`, with each URL from its home, and `plugins`, over
  the site's, setting by setting, with a `read` from the site's folder at
  the ref, or `false`, for none, beside its `source`. It can name only the
  site's plugins, and give none a `.wasm` file. Any other setting of
  little's there, like `theme` or `search`, stops the build; any other key
  is for the theme — see
  [`decisions/0043-a-versions-own-nav-and-plugins.md`](./decisions/0043-a-versions-own-nav-and-plugins.md).
- **A plugin for Rust's doc comments:** `plugins/doc_comments/`, in its own
  workspace, adds a page for a crate and each of its modules, `pub` or not,
  under `url`, `/reference/<crate>` by default, like rustdoc's, parsing Rust
  with syn, without compiling it, from the one folder it reads, with
  `lib.rs` or `main.rs`, and `crate`, its name. It documents each `pub`
  item with docs where it's defined, leads intra-doc links to their items,
  and shares its model and Markdown with docstrings, in
  `plugins/api_reference/`. The manual documents little with it, at
  `/api/` — see
  [`decisions/0044-a-plugin-for-rusts-doc-comments.md`](./decisions/0044-a-plugin-for-rusts-doc-comments.md).
- **Limits on downloads:** a download of a theme or a plugin stops if the
  server takes over 30 seconds to connect, or 30 more to answer, or if it
  takes over 5 minutes in all, or is over 200 MB, with ureq's own limits,
  and a theme's archive stops unpacking once the bytes it writes come to
  over 1 GB. Nothing is kept, and the error says which limit it hit; the
  limits are fixed, in `src/download.rs`, not settings — see
  [`decisions/0045-limits-on-downloads.md`](./decisions/0045-limits-on-downloads.md).
