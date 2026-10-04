# AGENTS.md — working conventions for little

Read this before making changes. **little** is a static documentation site
generator written in Rust. Its mission is **authoring experience** — see
[`VISION.md`](./VISION.md).

## Commands
```sh
cargo run -- build          # build the example site (docs/) into dist/, and check its links
cargo run -- build --strict # ...and fail on a broken link
cargo run -- serve          # live-reloading dev server at http://127.0.0.1:8080
cargo run -- config         # print the example site's merged config
cargo build --release       # produce the release binary (target/release/little)
cargo test                  # run tests
cargo clippy --all-targets  # lint — keep it clean
cargo fmt                   # format — run before committing
```

## Architecture (keep the core small)
- `src/main.rs` — CLI (clap): `build` (`--strict` fails on a broken link),
  `serve`, `config`.
- `src/config.rs` — the 3-level cascading config (little → theme → site): the
  chain of themes a site uses, its source and destination folders, the levels
  of heading its tables of contents list, the host and port
  `little serve` serves it at, the `url` it's published at, the `repo` and
  `branch` its source is in, whether it's indexed for search, and the
  extensions to Markdown its pages are written with.
- `src/little.yml` — little's own defaults (level 1), compiled into the binary.
- `src/render.rs` — Markdown (comrak, with the extensions the site turns on) +
  YAML frontmatter → HTML fragment, with each relative link led from the
  folder of the file it's in, and each link to a page's `.md` file led to the
  page, noting each link as it's written.
- `src/site.rs` — build pipeline: walk the source (`docs/`), render its
  Markdown, copy its other files, apply theme, index it for search, write the
  destination (`dist/`), noting the source file and URL of each file it
  writes. Finds where the source is in its repository, so each page can link
  to its file there.
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
  it's downloaded only once.
- `src/server.rs` — dev server + file watcher + live reload; checks the links
  within the site after each build.

Content and theme live outside `src/`:
- `docs/` — the example/dogfood site content. This is little's **input**, not
  project documentation — don't put meta docs here.
- `theme/` — little's built-in theme, compiled into the binary: `_templates/`
  (MiniJinja; `default.html` is required) + `_static/` (copied to the site's
  root) + an optional `little.yml`. It's plain CSS: `builtin` has no `_sass/`.
  List every file in it in `builtin` in `src/theme.rs`; a test checks. Its
  search box is Pagefind's, deferred so it finds the `pagefind/` little
  writes.
- `little.yml` — example site config (optional). It sets `theme: ./theme`, so
  edits to the built-in theme live-reload.
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
   niche renderers, etc. are plugins (planned WASM API), not core code.
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
- **Plugins:** sandboxed WASM (planned).
