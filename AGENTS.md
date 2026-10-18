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
cargo package --locked      # package the crate, little-docs, as a release publishes it
cargo test                  # run tests (they build tests/plugin for wasm32-wasip2)
cargo clippy --all-targets  # lint — keep it clean
cargo fmt                   # format — run before committing

# the rustdoc plugin, which the manual uses: build it before the manual
cargo build --release --target wasm32-wasip2 --manifest-path plugins/rustdoc/Cargo.toml
cargo test --manifest-path plugins/rustdoc/Cargo.toml
cargo clippy --manifest-path plugins/rustdoc/Cargo.toml --all-targets --target wasm32-wasip2

# the pydoc plugin, in its own workspace
cargo build --release --target wasm32-wasip2 --manifest-path plugins/pydoc/Cargo.toml
cargo test --manifest-path plugins/pydoc/Cargo.toml
cargo clippy --manifest-path plugins/pydoc/Cargo.toml --all-targets --target wasm32-wasip2

# the jsdoc plugin, in its own workspace
cargo build --release --target wasm32-wasip2 --manifest-path plugins/jsdoc/Cargo.toml
cargo test --manifest-path plugins/jsdoc/Cargo.toml
cargo clippy --manifest-path plugins/jsdoc/Cargo.toml --all-targets --target wasm32-wasip2

# the library they share, in its own workspace
cargo test --manifest-path plugins/api_reference/Cargo.toml
cargo clippy --manifest-path plugins/api_reference/Cargo.toml --all-targets

# the third-party notices releases carry, with cargo-about 0.9.2
# (cargo install --locked cargo-about@0.9.2)
scripts/notices.sh little target/notices/little.md # or pydoc, rustdoc, jsdoc
```

## Architecture (keep the core small)
- `src/main.rs` — CLI (clap): `build` (`--strict` fails on a broken link,
  a formula that isn't rendered, a diagram that isn't drawn, a code block
  a plugin can't render, a plugin's warning, or a hook of one that failed
  on a page), `serve`, `config`; and, on musl, jemalloc as the allocator.
- `src/config.rs` — the 3-level cascading config (little → theme → site): the
  chain of themes a site uses, its source and destination folders, the levels
  of heading its tables of contents list, the host and port
  `little serve` serves it at, the `url` it's published at, the `repo` and
  `branch` its source is in, whether it's indexed for search, the extensions
  to Markdown its pages are written with, how their math is written, how
  their diagrams are drawn, and look, with Mermaid's own config, the
  classes their code is highlighted with, and the plugins whose hooks a
  build calls, each with its `.wasm` file, the folders it reads, and its
  settings; and the versions it's built in, each with its
  title and URL, one at `/`, and each from a git ref but one, with the
  folder its pages are in there, and the config of each: the site's, but
  for its own `source`, `nav`, and plugins' settings, with its links to the
  theme's files led to the root. Takes the config plugins' `config` hooks
  give back, but for the settings little reads before they start.
- `src/little.yml` — little's own defaults (level 1), compiled into the binary.
- `src/render.rs` — Markdown (comrak, with the extensions the site turns on) +
  YAML frontmatter → HTML fragment, with each relative link led from the
  folder of the file it's in, and each link to a page's `.md` or `.ipynb`
  file led to the page, noting each link as it's written. A notebook's tree
  is written the same way, once plugins have rendered its blocks and its
  diagrams are drawn.
- `src/plugin.rs` — plugins: WebAssembly components, run with wasmtime, built
  in, that export hooks from `wit/plugin.wit`. Starts each a build uses
  once, in a sandbox that reaches only the folders it reads, and calls each
  hook it exports, plugin after plugin, in the order `plugins` names them,
  each with what the one before gave back, checking what it gives back.
  Stops the build on a hook called once that fails, and notes one called
  for each page that fails, and what a plugin warns of, to warn of. Gives
  each plugin each fenced code block in its languages, before diagrams are
  drawn, putting the Markdown or HTML it gives back in the block's place;
  one it can't render stays code, and is noted, with its line, to warn of.
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
- `src/site.rs` — build pipeline: start the site's plugins, and call their
  hooks at each step: walk the source (`docs/`), with the files plugins add
  and leave out, render its Markdown and notebooks, stopping on two that
  would be the same page, copy its other files, apply theme, index it for
  search, add the files plugins add, write the destination (`dist/`),
  noting the source file and URL of each file it writes. Finds where the
  source is in its repository, so each page can
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
  as plugins' `page` hooks give them back, with Pagefind, in the language
  the theme gives the page, into the files of the destination's `pagefind/`.
- `src/links.rs` — checks the links in the built site with lychee-lib: those
  within the site in the destination, as the site is published under its
  `url`, and, for `little build`, those to other sites over the network, but
  not those into the site's repository, nor those of a version from a git
  ref. Shows a broken link in Markdown as it's written.
- `src/theme.rs` — a theme's templates, static files, and Sass, layered over
  those of the themes it builds on, and the built-in theme, as plugins'
  `theme` hooks give them back; picks each page's
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
  little's ADRs are in `docs/decisions/`, listed in its `README.md`, which alone
  is in `nav`; other meta docs don't go here. little's own API, which the
  `rustdoc` plugin adds from `src/`'s doc comments, is at `/api/`, with
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
  orders its pages by hand, and the `rustdoc` plugin, which documents
  `src/` at `/api/`, from where Cargo builds its `.wasm` file. It sets
  `theme: ./theme`, so edits to the built-in theme live-reload.
- `wit/plugin.wit` — `little:plugin@0.2.0`: an interface for each hook a
  plugin can export, and `warnings`, which it imports. Plugin authors copy
  it, and write a world that exports the hooks they need; little's
  bindings are generated from `world hooks`, which exports every one.
- `tests/plugin/` — a plugin, in Rust, with its own workspace, that the tests
  build for `wasm32-wasip2` (which `rust-toolchain.toml` asks rustup for) and
  drive, hook by hook, to check what plugins can and can't do: it exports
  every hook, and each does what its settings say.
- `plugins/pydoc/` — the `pydoc` plugin, a crate in its own
  workspace, built for `wasm32-wasip2`, that adds an API reference, with the
  `files` hook: a page
  for each Python module in the folders it reads, from its docstrings, in
  NumPy's, Google's, or Sphinx's style, parsed with Ruff's parser. Its tests
  run natively; only its glue to `wit/plugin.wit` needs WebAssembly.
- `plugins/rustdoc/` — the `rustdoc` plugin, a crate in its own
  workspace, built for `wasm32-wasip2`, that adds an API reference, with the
  `files` hook: a page
  for a Rust crate and each of its modules, from their doc comments, parsed
  with syn, and with intra-doc links led to their items. The manual uses it
  on `src/`. Its tests run natively, as pydoc's do.
- `plugins/jsdoc/` — the `jsdoc` plugin, a crate in its own
  workspace, built for `wasm32-wasip2`, that adds an API reference, with the
  `files` hook: a page
  for a JavaScript or TypeScript package and each of its modules, from
  their doc comments, with JSDoc's and TSDoc's tags, parsed with oxc's
  parser, in ES modules, CommonJS, and scripts. Its tests run natively, as
  pydoc's do.
- `plugins/api_reference/` — a library, in its own workspace, not a plugin,
  that the plugins that add an API reference share: what a page documents,
  in no language in particular, and the Markdown of the page.
- `about.toml`, `about.hbs`, `scripts/notices.sh` — the third-party notices
  that little's and the plugins' releases carry, which cargo-about writes,
  through `about.hbs`, from the `about.toml` beside each one's
  `Cargo.toml`: the licences its crates may use, and the crates cargo-about
  can't read rightly, clarified with the licence files it should use,
  pinned by their checksums. `examples/syntax_notices.rs` adds those of the
  syntax definitions two-face bundles to little's. CI writes them all, and
  fails on any warning.
- `.github/workflows/release-plugin.yml` — releases a plugin when a tag
  that names it and its version, like `pydoc-v0.1.0`, is pushed: checks
  the tag, builds the plugin's `.wasm` file, and publishes it, beside its
  third-party notices, in a GitHub release, with notes that git-cliff
  writes, from `cliff.toml`, from the commits that changed the plugin's
  folder or `plugins/api_reference/`.
- `.github/workflows/release.yml` — releases little when a tag of its
  version, like `v0.1.0`, is pushed: checks the tag, builds little for
  Linux, macOS, and Windows, on x64 and arm64, Linux's static, against
  musl, each on a runner of its system, checks each binary runs, and packs
  each in an archive with `LICENSE` and its third-party notices; then
  publishes them, with a `SHA256SUMS`, attested, in a GitHub release, the
  repository's latest, with notes that git-cliff writes from the commits
  that changed what's built into little, and publishes its crate,
  `little-docs`, to crates.io. A pull request that changes it,
  `Cargo.toml`, or `Cargo.lock` runs it, but for publishing.
- Project meta (`VISION.md`, `AGENTS.md`, `RELEASING.md`) lives at the repo
  root; ADRs are in the manual, in `docs/decisions/`.

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
   niche renderers, etc. are plugins (WASM components that hook into each
   step of the build), not core code.
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
- A new crate's licence must be one its component's `about.toml` accepts,
  or CI fails: read it before accepting it, and clarify a crate that
  bundles code its `license` leaves out, as
  [`RELEASING.md`](./RELEASING.md#third-party-notices) says.
- Comment the *why*, not the *what*; don't over-comment.
- Doc comments in `src/` are published, as little's API at `/api/` in the
  manual: give each module a `//!`, and each `pub` item a `///`, in the
  manual's plain style, and link to items by path, like ``[`Config`]``.
- Add unit tests for parsing/merging logic (frontmatter, config cascade).
- A change people using little can see updates its page in the manual
  (`docs/`), and the reference pages, in the same PR; a new page goes in
  `nav` in `little.yml`, but a new ADR in the list in
  `docs/decisions/README.md`. Keep the README short: it links to the manual.
  `cargo run -- build --strict` checks the manual's links.
- Versions follow [Semantic Versioning](https://semver.org/): each
  component's, in its `Cargo.toml`, is raised as it's released, and
  `little:plugin`'s, in `wit/plugin.wit`, by the change that needs it.
  Until 1.0, a breaking change raises the minor version, 0.1.4 to 0.2.0,
  and anything else the patch, 0.1.4 to 0.1.5. A change is breaking if it
  breaks what the manual documents for sites, themes, or plugins to rely
  on, as [0051](./docs/decisions/0051-semantic-versioning-and-conventional-commits.md)
  lists.
- little is released from a tag on `main` like `v0.2.0`, and a plugin from
  one like `pydoc-v0.2.0`, each after a pull request that raises its
  version, as [`RELEASING.md`](./RELEASING.md) says. A release can't be
  undone, so don't push a tag: the maintainer does.
- Commits follow [Conventional Commits](https://www.conventionalcommits.org/),
  as `committed.toml` checks in CI: `type(scope)!: description`, with a
  lowercase, imperative description, no full stop, and a first line of up
  to 100 characters. `committed main..HEAD`, after
  `cargo install committed`, checks a branch as CI does.
  - Type: `feat`, `fix`, or `perf` for a change to what people use, which
    is released; `docs`, `refactor`, `test`, `style`, `build`, `ci`, or
    `chore` for any other; `revert`, with a `Refs:` footer, to undo one.
  - Scope: the plugin a commit is about, `pydoc`, `rustdoc`, `jsdoc`, or
    `api_reference`; none for little, its theme, `wit/`, or more than one
    component.
  - Breaking: `!` before the `:`, and a footer,
    `BREAKING-CHANGE: <what to change>`, beside the co-author trailer;
    written `BREAKING CHANGE:`, with a space, git misses the trailer.
  - Include the Copilot co-author trailer. Rebase a branch on `main`,
    rather than merging `main` into it.

## Decisions
Record notable choices as short ADRs in `docs/decisions/`, part of the
manual, and list each in its `README.md`. Locked so far:
- **Language:** Rust — see [`docs/decisions/0001-language-rust.md`](./docs/decisions/0001-language-rust.md).
- **Markdown:** comrak. **Templating:** minijinja. **Config:** cascading YAML.
- **Themes:** `_templates/` + `_static/`, a template per type of page — see
  [`docs/decisions/0002-theme-structure.md`](./docs/decisions/0002-theme-structure.md).
- **Choosing themes:** `theme` is `true` (built-in), `false`, a folder, or a
  URL, and a theme can build on another, file by file — see
  [`docs/decisions/0003-choosing-and-layering-themes.md`](./docs/decisions/0003-choosing-and-layering-themes.md).
- **Downloading themes:** an HTTPS `.zip` or `.tar.gz`, unpacked into the
  site's `.little/` and never downloaded again; links and paths that leave the
  archive are refused — see
  [`docs/decisions/0004-downloading-themes.md`](./docs/decisions/0004-downloading-themes.md).
- **Previous and next pages:** `page.prev` and `page.next` follow `site.nav`
  from top to bottom, and are left out at either end — see
  [`docs/decisions/0005-previous-and-next-pages.md`](./docs/decisions/0005-previous-and-next-pages.md).
- **Frontmatter and config win:** every frontmatter key reaches `page` and
  merges over what little works out; a `nav` in `little.yml` replaces
  little's — see
  [`docs/decisions/0006-frontmatter-and-config-win.md`](./docs/decisions/0006-frontmatter-and-config-win.md).
- **A page's table of contents:** `page.toc` is shaped like `site.nav`,
  `{ title, url, children }`, with each `###` under the `##` above it and
  each heading's `#fragment` as its `url` — see
  [`docs/decisions/0007-page-nav-shaped-like-site-nav.md`](./docs/decisions/0007-page-nav-shaped-like-site-nav.md).
- **Naming a page's navigation:** name each for what readers see, since
  authors write the same names in frontmatter: `page.toc`, not `page.nav`,
  beside `page.prev` and `page.next` — see
  [`docs/decisions/0008-page-toc.md`](./docs/decisions/0008-page-toc.md).
- **Which headings a page's table of contents lists:** the levels from
  `toc_min_depth` to `toc_max_depth`, which cascade like any config: little
  sets 2 to 6, every heading but the page's `#` title, and the built-in theme
  a max of 3 — see
  [`docs/decisions/0009-toc-depth-in-config.md`](./docs/decisions/0009-toc-depth-in-config.md).
- **Breadcrumbs:** `page.breadcrumbs` lists the items above a page in
  `site.nav`, from the top down, each `{ title, url }`: the home page, if it's
  at the nav's top, then the sections the page is in, but not the page
  itself — see [`docs/decisions/0010-breadcrumbs.md`](./docs/decisions/0010-breadcrumbs.md).
- **Where `little serve` serves:** `host` and `port`, which cascade like any
  config: little sets `127.0.0.1` and `8080`, and `little serve --host` and
  `--port` override them for a single run — see
  [`docs/decisions/0011-serve-host-and-port-in-config.md`](./docs/decisions/0011-serve-host-and-port-in-config.md).
- **Hidden files:** a file or folder in the source whose name starts with `.`
  is left out, and saving it doesn't rebuild in `little serve`; only names
  inside the source count, so it can be hidden itself, and a theme's
  `_static/` is still copied whole — see
  [`docs/decisions/0012-hidden-files.md`](./docs/decisions/0012-hidden-files.md).
- **Pages that clash:** two files in the source that would be the same page,
  like `guide.md` and `guide/README.md`, stop the build before any page is
  rendered, with an error that names both; URLs that differ only in case
  aren't checked — see
  [`docs/decisions/0013-pages-that-clash.md`](./docs/decisions/0013-pages-that-clash.md).
- **Publishing under a path:** a site's `url`, like
  `https://withlittle.github.io/little`, puts its path before every URL
  little gives the theme and every link in Markdown, and `little serve`
  serves the site under it; a theme writes its own URLs through the `url`
  filter, and makes them absolute with `absolute_url` — see
  [`docs/decisions/0014-site-url-and-its-path.md`](./docs/decisions/0014-site-url-and-its-path.md).
- **Files and relative links:** every file in the source that isn't hidden or
  Markdown is copied to the same path in the destination, over a theme's file
  at that path, but for a destination inside the source; a relative link or
  image leads from the folder of the file it's in, as on GitHub, whatever URL
  its page is served at — see
  [`docs/decisions/0015-files-and-relative-links.md`](./docs/decisions/0015-files-and-relative-links.md).
- **Checking links:** `little build` checks every link in the built site
  with lychee-lib, built in, and warns of each broken page, file, `#fragment`,
  or link to another site, naming the file it's in; `--strict` fails on one,
  and `little serve` checks only links within the site — see
  [`docs/decisions/0016-checking-links.md`](./docs/decisions/0016-checking-links.md).
- **Links to pages' files:** a link in Markdown to a `.md` file, like
  `install.md#usage` or `guide/README.md`, leads to the page built from it,
  as it leads to the file on GitHub, even one that isn't there, so it's
  warned of; a warning shows a broken link in Markdown as it's written, and
  one in the theme or a copied file as a reader follows it — see
  [`docs/decisions/0017-links-to-pages-files.md`](./docs/decisions/0017-links-to-pages-files.md).
- **Sass in themes:** a `.scss` or `.sass` file in a theme's `_static/` is
  compiled with grass, built in, to the CSS file at its path, loading only
  from the theme's `_sass/`, which isn't published and layers like
  `_templates/`; a partial in `_static/` stops the build, and Sass in the
  source is copied as-is — see
  [`docs/decisions/0018-sass-in-themes.md`](./docs/decisions/0018-sass-in-themes.md).
- **Site search:** `search`, which little sets `false` and the built-in
  theme `true`, indexes each page's own content, under its title and URL,
  with Pagefind, built in, into the destination's `pagefind/`, in the language
  of the page's `<html lang>`, but not the 404 page; the theme owns the
  search box, and a page or file where search writes one stops the build —
  see [`docs/decisions/0019-site-search.md`](./docs/decisions/0019-site-search.md).
- **Where search's files go:** `pagefind/`, Pagefind's default, and the
  built-in theme loads Pagefind's search box with `defer`, not as a module,
  so it finds them itself, even under the path of the site's `url`; each
  page is indexed at its URL from the root of the site, and Pagefind puts
  the site's path back, from where it finds them — see
  [`docs/decisions/0021-search-files-in-pagefind.md`](./docs/decisions/0021-search-files-in-pagefind.md).
- **Linking to a page's file:** `repo`, the repository's address as a
  browser shows it, reaches the theme as `site.repo`, and, on GitHub and
  GitLab, gives each page `page.source_url` and `page.edit_url`, on `branch`,
  which little sets to `main`, with each path from the nearest folder with a
  `.git`, else the site's folder; links into the repository aren't
  checked — see
  [`docs/decisions/0020-links-to-a-pages-file-in-its-repo.md`](./docs/decisions/0020-links-to-a-pages-file-in-its-repo.md).
- **Markdown's extensions:** `markdown` turns each of comrak's extensions
  to what an author can write on or off, under little's own names, like
  `emoji` and `raw_html`, and cascades like any config: little turns on
  GitHub's and leaves the rest off. Callouts are GitHub's alerts, in GitHub's
  markup — see
  [`docs/decisions/0022-markdown-extensions.md`](./docs/decisions/0022-markdown-extensions.md).
- **Highlighting code:** little highlights each code block as it builds the
  page, with bat's grammars, built in through two-face, in the classes
  `markdown.highlight` chooses: `pygments`, little's default, `github`, or
  `false`, for none. A highlighted block is a `<pre class="highlight">`, with
  a flat `<span>` for each token with a class; a language bat doesn't have
  stays plain — see
  [`docs/decisions/0023-highlighting-code.md`](./docs/decisions/0023-highlighting-code.md).
- **Jupyter notebooks:** each `.ipynb` file in the source is a page, as a
  `.md` file is, read in format 4 with little's own serde types, not the
  stricter `nbformat` crate, and built into comrak's tree, so it's written as
  Markdown is. Outputs are code blocks in `output`, HTML and SVG when
  `raw_html` is on, or images with `data:` URLs; frontmatter is in a raw or
  Markdown cell at the top — see
  [`docs/decisions/0024-jupyter-notebooks.md`](./docs/decisions/0024-jupyter-notebooks.md).
- **Rendering math:** `markdown.math` writes math as `mathml`, little's
  default, rendered with math-core, built in, as each page is built; `tex`,
  marked with `data-math-style` for a theme's script; or `false`, for none,
  and `true` is an error. Each formula is read on its own, and one that
  can't be is written as TeX and warned of, as a broken link is; a heading's
  math is TeX in `page.toc` — see
  [`docs/decisions/0025-rendering-math-as-mathml.md`](./docs/decisions/0025-rendering-math-as-mathml.md).
- **The year a site is built:** `site.year`, for a theme's copyright notice,
  is the year the build starts in, in UTC, or that of `SOURCE_DATE_EPOCH`,
  for reproducible builds; it's worked out once, beneath the config, like
  `site.nav`, and the built-in theme shows no notice — see
  [`docs/decisions/0026-the-year-a-site-is-built.md`](./docs/decisions/0026-the-year-a-site-is-built.md).
- **Drawing diagrams:** `markdown.mermaid` writes a ```` ```mermaid ````
  block as `svg`, little's default, an `<svg>` drawn with merman, built in,
  as each page is built, with the id `mermaid-N`, the same in each build; or
  `false`, as code, for a theme's script. One merman can't read is code, and
  warned of, as a broken link is. A diagram can carry HTML, so `svg` with
  `raw_html: false` stops the build — see
  [`docs/decisions/0027-drawing-mermaid-diagrams.md`](./docs/decisions/0027-drawing-mermaid-diagrams.md).
- **How diagrams look:** `mermaid` holds Mermaid's own config, in Mermaid's
  own names, like `theme` and `themeVariables`, which each diagram little
  draws starts from, as a site passes it to `mermaid.initialize()`. It
  cascades like any config, but little sets none, keeping its seeds and a
  gantt chart's width in code, beneath it; a diagram's own config goes over
  it. little checks only that `theme` is one of Mermaid's and that no
  setting is empty, as an unquoted `#` colour is — see
  [`docs/decisions/0028-how-diagrams-look.md`](./docs/decisions/0028-how-diagrams-look.md).
- **Plugins that render code blocks:** `plugins` names each plugin and its
  `.wasm` file, a WebAssembly component built for WASI 0.2 that exports
  hooks from `wit/plugin.wit`, by a path or an HTTPS URL, downloaded once into
  `.little/plugins/`, and cascades plugin by plugin and setting by setting;
  only the site can set `read`, the folders a plugin reads. A fenced block in
  a plugin's language goes to its `code-blocks` hook, before diagrams are
  drawn, and the Markdown
  it gives back is written as the author's, or the HTML as it is. A plugin
  runs in wasmtime's sandbox, with no network, environment, or files but
  those it reads, a clock at the build's start, and the same random numbers
  each build; a block it can't render is warned of, as a broken link is —
  see
  [`docs/decisions/0029-plugins-that-render-code-blocks.md`](./docs/decisions/0029-plugins-that-render-code-blocks.md).
- **Plugins that add pages:** a plugin's `files` hook, in place of 0030's
  `pages`, adds files, each built as if it were in the source at its path,
  so a Markdown file or a notebook is a page, but has no file in the
  repository; one that would be the same page as a file or another
  plugin's page stops the build, as does an error from `files`, or a crash.
  What a plugin warns of is warned of, and `--strict` fails on it — see
  [`docs/decisions/0030-plugins-that-add-pages.md`](./docs/decisions/0030-plugins-that-add-pages.md).
- **A plugin for Python's docstrings:** `plugins/pydoc/`, in little's
  repository, in its own workspace, adds a page for each module and package
  in the folders it reads, under `url`, `/` by default, like
  mkdocstrings', parsing Python with Ruff's parser, without running it, and
  finding each docstring's style, NumPy's, Google's, or Sphinx's, for
  itself. A module it can't parse is warned of, and left out — see
  [`docs/decisions/0031-a-plugin-for-pythons-docstrings.md`](./docs/decisions/0031-a-plugin-for-pythons-docstrings.md).
- **Docs are the manual:** `docs/` is little's manual, published at
  `https://withlittle.github.io/little`, grouped by what a reader is trying
  to do, and ordered by `nav` in `little.yml`; each page uses what it
  documents, and the README points to it — see
  [`docs/decisions/0032-docs-are-the-manual.md`](./docs/decisions/0032-docs-are-the-manual.md).
- **The built-in theme's design:** plain CSS, with no framework or Node, in
  cascade layers, `little.tokens` to `little.print`, so CSS that isn't in a
  layer wins over it, but for a few `!important`s, and it works without a
  script. Its colours are
  tokens, each a `light-dark()` pair, light or dark as the reader's system
  is. Below 60rem, the nav is a popover drawer, and below 75rem, the table
  of contents folds up above the page. It has a skip link, landmarks, a
  visible focus ring, AA contrast, underlined links, and styles for reduced
  motion, forced colours, and print, and needs browsers from 2024 — see
  [`docs/decisions/0033-the-built-in-themes-design.md`](./docs/decisions/0033-the-built-in-themes-design.md).
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
  [`docs/decisions/0034-the-built-in-themes-script.md`](./docs/decisions/0034-the-built-in-themes-script.md).
- **Numbers in the nav's order:** little sorts each folder's pages and
  folders by title, whatever their case, with each run of digits as a
  number, by its value, with alphanumeric-sort, 1.5.8 or later, for a total
  order: `2.` before `10.`, and `1` before `01`. A `nav` in `little.yml` is
  kept as it's written — see
  [`docs/decisions/0035-numbers-in-the-navs-order.md`](./docs/decisions/0035-numbers-in-the-navs-order.md).
- **Folding the built-in theme's nav:** each folder within a top-level one
  is a `<details>`, folded, but for those the current page is in, which the
  template finds from the `aria-current="page"` in its items' HTML, so
  little's data doesn't change. A folder with a page folds from a button
  beside its link. The script scrolls only the sidebar to the current page;
  which folders are open isn't remembered — see
  [`docs/decisions/0036-folding-the-built-in-themes-nav.md`](./docs/decisions/0036-folding-the-built-in-themes-nav.md).
- **Wrapping long names in the built-in theme:** a title with no spaces, a
  name, like `enable_iterative_imputer`, gets a `<wbr>` after each `_` and
  `.`, from the template's `breakable` macro, in the sidebar, tables of
  contents, breadcrumbs, and pager, so it wraps at its joints; a title with
  spaces wraps at them. Each also has `overflow-wrap: anywhere`, so nothing
  runs out of its box; `<title>` and hidden names are as they were — see
  [`docs/decisions/0037-wrapping-long-names-in-the-built-in-theme.md`](./docs/decisions/0037-wrapping-long-names-in-the-built-in-theme.md).
- **The built-in theme's page and tables:** from 75rem, on a site with a
  nav, a page without a table of contents keeps its empty column, so the
  text starts in the same place on every page. A table, but a `.dataframe`,
  is as wide as the page from the first paint, with `.js`, and the script
  puts each in a `<div class="table-scroll">`, which scrolls one too wide
  and takes its frame, so nothing moves; without the script, a table is as
  wide as its columns, as before. A column aligned in Markdown keeps its
  alignment — see
  [`docs/decisions/0038-the-built-in-themes-page-and-tables.md`](./docs/decisions/0038-the-built-in-themes-page-and-tables.md).
- **A long site name in the built-in theme's header:** a name too long for
  one line, in the header or the drawer, wraps onto a second, balanced,
  and is cut short with an ellipsis past that, with
  `-webkit-line-clamp: 2`, so the buttons beside it stay on the screen. The
  description has only the room the name leaves, and an icon button
  doesn't shrink; a name that fits on one line looks as it did — see
  [`docs/decisions/0039-a-long-site-name-in-the-built-in-themes-header.md`](./docs/decisions/0039-a-long-site-name-in-the-built-in-themes-header.md).
- **Versions:** `versions` lists a site's versions, the latest first, each
  with the git `ref`, a tag, branch, or commit, it's read from, but for at
  most one, the site's files as they are. `little build` builds them all,
  each in its folder, with its own search index, in the site's theme, by
  today's little, each from its ref, read with `git`. The theme gets
  `site.versions`, `site.version`, and `page.versions`, the same page in
  each version, else its home, and the built-in theme has a menu of them in
  its header. A shallow clone stops the build; a version from a ref's links
  to other sites aren't checked — see
  [`docs/decisions/0040-versions-from-git-refs.md`](./docs/decisions/0040-versions-from-git-refs.md).
- **A version's title and URL:** each version has a `title`, what the menu
  shows, like `2.0 (Alpha)`, and a `url`, where it's published from the
  root of the site, like `/1.4` or `/versions/1.4`, as the nav's items do,
  in place of a `name`. Exactly one is at `/`, whatever its place in the
  list, which is the menu's order; others can be in its folders, but not in
  each other's. The one at `/` has the 404 page, and its pages are checked
  against the others' folders. `site.version` is the title, and each entry
  in `site.versions` and `page.versions` has `root`, true for the one at
  `/` — see
  [`docs/decisions/0041-version-titles-and-urls.md`](./docs/decisions/0041-version-titles-and-urls.md).
- **What versions share:** every version is built with the site's config,
  theme, and plugins, as the site's `little.yml` sets them; one from a ref
  has its own pages and files, nav, and the folders its plugins read, from
  there. Of its ref's `little.yml`, if it has one, only `source` and `nav`
  count, and its entry's `source` wins over both, for docs that have
  moved. The theme's files are written once, with the version at `/`, and
  every version links to them there, but to a file of its own at the same
  path — see
  [`docs/decisions/0042-versions-share-the-sites-config-and-theme-files.md`](./docs/decisions/0042-versions-share-the-sites-config-and-theme-files.md).
- **A version's own nav and plugins:** an entry in `versions` with a `ref`
  can set its own `nav`, with each URL from its home, and `plugins`, over
  the site's, setting by setting, with a `read` from the site's folder at
  the ref, or `false`, for none, beside its `source`. It can name only the
  site's plugins, and give none a `.wasm` file. Any other setting of
  little's there, like `theme` or `search`, stops the build; any other key
  is for the theme — see
  [`docs/decisions/0043-a-versions-own-nav-and-plugins.md`](./docs/decisions/0043-a-versions-own-nav-and-plugins.md).
- **A plugin for Rust's doc comments:** `plugins/rustdoc/`, in its own
  workspace, adds a page for a crate and each of its modules, `pub` or not,
  under `url`, `/<crate>` by default, like Rust's own rustdoc's,
  parsing Rust with syn, without compiling it, from the one folder it
  reads, with `lib.rs` or `main.rs`, and `crate`, its name. It documents
  each `pub` item with docs where it's defined, leads intra-doc links to
  their items, and shares its model and Markdown with `pydoc`, in
  `plugins/api_reference/`. The manual documents little with it, at
  `/api/` — see
  [`docs/decisions/0044-a-plugin-for-rusts-doc-comments.md`](./docs/decisions/0044-a-plugin-for-rusts-doc-comments.md).
- **Limits on downloads:** a download of a theme or a plugin stops if the
  server takes over 30 seconds to connect, or 30 more to answer, or if it
  takes over 5 minutes in all, or is over 200 MB, with ureq's own limits,
  and a theme's archive stops unpacking once the bytes it writes come to
  over 1 GB. Nothing is kept, and the error says which limit it hit; the
  limits are fixed, in `src/download.rs`, not settings — see
  [`docs/decisions/0045-limits-on-downloads.md`](./docs/decisions/0045-limits-on-downloads.md).
- **Plugin hooks:** there's one kind of plugin, which exports the hooks it
  needs from `little:plugin@0.2.0`, each an interface in
  `wit/plugin.wit`: `setup`, `config`, `theme`, `files`, `source`,
  `code-blocks`, `content`, `site`, `page`, `html`, and `destination`, in
  the order the build reaches them. Each is given what the build has made,
  and gives back what it goes on with, plugin after plugin, in the order
  `plugins` names them, a theme's first. One called once that fails stops
  the build; one called for each page or block is warned of, and the build
  goes on with what it was given. `config` can't change the settings little
  reads before plugins start; search indexes what `page` gives back, and
  links are checked in what `html` gives back. A plugin built for 0.1.0
  doesn't load — see
  [`docs/decisions/0046-plugin-hooks.md`](./docs/decisions/0046-plugin-hooks.md).
- **Naming API reference plugins:** a plugin that adds an API reference
  from a language's code is named `<language>doc`, as the language's own
  tool is: `rustdoc`, for Rust's doc comments, and `pydoc`, for Python's
  docstrings, in place of `doc_comments` and `docstrings`, which other
  languages write too; `jsdoc` and `godoc` would follow. The name is its
  folder in `plugins/`, its crate, `little-rustdoc`, its `.wasm` file,
  `little_rustdoc.wasm`, and its entry in the manual's `plugins`. Neither
  runs its language's tool, and its page says so first; the manual's pages
  keep their names, like "Rust API reference" — see
  [`docs/decisions/0047-naming-api-reference-plugins.md`](./docs/decisions/0047-naming-api-reference-plugins.md).
- **A plugin for JavaScript's doc comments:** `plugins/jsdoc/`, in its own
  workspace, adds a page for a JavaScript or TypeScript package and each of
  its modules, under `url`, `/<package>` by default, parsing its
  code with oxc's parser, without running it, or Node, from the one folder
  it reads, and `package`, its name on npm. It documents what each module
  exports that has docs, where it's defined, from JSDoc's and TSDoc's tags,
  in ES modules, CommonJS, as TypeScript reads it, and scripts; lists
  re-exports; leads `{@link}`s to their items; and shares its model and
  Markdown with `pydoc` and `rustdoc`. It doesn't infer types — see
  [`docs/decisions/0048-a-plugin-for-javascripts-doc-comments.md`](./docs/decisions/0048-a-plugin-for-javascripts-doc-comments.md).
- **An API reference's URL and title:** unless `url` is set, `pydoc` puts
  each package in a folder of its name at the root, and `rustdoc` and
  `jsdoc` put their pages in `/<crate>` and `/<package>`, not under
  `/reference`, a folder of little's choosing. Each takes `title`, which it
  writes in the frontmatter of the page of what it documents, so the nav,
  breadcrumbs, tab, and search call it that, while its `#` heading is still
  the name; `pydoc` with `title` documents one package or module, and more
  stops the build. `crate` and `package` stay, for links and the default
  `url` — see
  [`docs/decisions/0049-an-api-references-url-and-title.md`](./docs/decisions/0049-an-api-references-url-and-title.md).
- **One repository:** little, its built-in theme, `wit/plugin.wit`, its
  plugins and the library they share, and the manual stay in one
  repository, `withlittle/little`, with little at its root, so a change to
  what little gives themes or plugins is one pull request, tested together.
  Each plugin is its own Cargo workspace, and each component is released on
  its own, from a tag that names it. One moves to a repository of its own
  once it has maintainers of its own, or its CI holds the rest up, but not
  while the plugin interface or templates' data still breaks — see
  [`docs/decisions/0050-one-repository-for-little-its-themes-and-plugins.md`](./docs/decisions/0050-one-repository-for-little-its-themes-and-plugins.md).
- **Versions and commits:** little, `pydoc`, `rustdoc`, and `jsdoc` each
  follow Semantic Versioning, with its own version, in its `Cargo.toml`,
  and `little:plugin` has its own, in `wit/plugin.wit`. Until 1.0, where
  SemVer sets no rules, a breaking change raises the minor version, and
  anything else the patch, as Cargo and wasmtime read 0.y.z; a change is
  breaking if it breaks what the manual documents for sites, themes, or
  plugins to rely on. Commits follow Conventional Commits, with a
  plugin's name as the scope, or none, for little; committed checks each
  one a pull request adds, and pull requests are still merged with a
  merge commit — see
  [`docs/decisions/0051-semantic-versioning-and-conventional-commits.md`](./docs/decisions/0051-semantic-versioning-and-conventional-commits.md).
- **Releasing plugins:** a plugin is released from a tag that names it and
  its version, like `pydoc-v0.2.0`, on `main`, pushed by hand, once a pull
  request, `chore(pydoc): release 0.2.0`, has raised the version in its
  `Cargo.toml`, its `Cargo.lock`, and the manual's URLs for it. A workflow
  checks the tag, builds the plugin's `.wasm` file, and publishes it, not
  as the repository's latest release, which is little's, with notes that
  name the `little:plugin` it's built for, and list what breaks, then the
  features, fixes, and performance, from the commits that changed its
  folder or `plugins/api_reference/`, with git-cliff. The manual gives each
  plugin's URL with its version, and releases are immutable — see
  [`docs/decisions/0052-releasing-plugins-from-tags.md`](./docs/decisions/0052-releasing-plugins-from-tags.md).
- **Decisions in the manual:** these ADRs are in `docs/decisions/`, each a
  page of the manual, at `/decisions/`, built, searched, and checked like
  any other, and still read on GitHub; `docs/decisions/README.md` lists
  them, and alone is in `nav`, under Reference. A record links to little's
  files outside `docs/`, like `VISION.md`, on GitHub. A page can't leave
  itself out of search yet, so search finds amended records too — see
  [`docs/decisions/0053-decisions-in-the-manual.md`](./docs/decisions/0053-decisions-in-the-manual.md).
- **Third-party notices:** little's releases, and each plugin's, carry a
  `THIRD-PARTY-NOTICES.md`, written as the release is made, not committed,
  by cargo-about 0.9.2, through `about.hbs`, from the `about.toml` beside
  each one's `Cargo.toml`. Each accepts only the licences its crates use;
  little's clarifies seven crates, whose `license` leaves out the C code
  they bundle, or whose licence files hold more than one licence, or
  others' notices, pinned by checksums; and any other crate gets its own
  licence file, or SPDX's text. little's end with those of the syntax definitions two-face bundles,
  from `examples/syntax_notices.rs`. `scripts/notices.sh` fails on any
  warning, and CI runs it for each pull request — see
  [`docs/decisions/0054-third-party-notices.md`](./docs/decisions/0054-third-party-notices.md).
- **Releasing little:** little is released from a tag of its version, like
  `v0.2.0`, on `main`, pushed by hand, once a pull request,
  `chore: release 0.2.0`, has raised it in `Cargo.toml`, `Cargo.lock`, and
  the manual's Pages workflow. `release.yml` checks the tag as a plugin's
  is, builds little for six targets, each on a GitHub runner of its system,
  Linux's static, against musl, with jemalloc, and Windows' with the C
  runtime built in, and checks each runs. It publishes each in an archive,
  `little-<target>.tar.gz`, or `.zip`, with its files at the root, and no
  version in its name, beside its licence and notices, with a `SHA256SUMS`
  and GitHub's attestations, as the repository's latest release, with
  notes from git-cliff. Then it publishes the crate, `little-docs`, as
  `little` is taken, though the command is still `little`, with crates.io's
  trusted publishing, but for its first version, published by hand; CI
  packages it for each pull request, and cargo-binstall downloads the
  release's archives. The binaries aren't signed yet — see
  [`docs/decisions/0055-releasing-little.md`](./docs/decisions/0055-releasing-little.md).
