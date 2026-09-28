# AGENTS.md — working conventions for little

Read this before making changes. **little** is a static documentation site
generator written in Rust. Its mission is **authoring experience** — see
[`VISION.md`](./VISION.md).

## Commands
```sh
cargo run -- build          # build the example site (docs/) into dist/
cargo run -- serve          # live-reloading dev server at http://127.0.0.1:8080
cargo run -- config         # print the example site's merged config
cargo build --release       # produce the release binary (target/release/little)
cargo test                  # run tests
cargo clippy --all-targets  # lint — keep it clean
cargo fmt                   # format — run before committing
```

## Architecture (keep the core small)
- `src/main.rs` — CLI (clap): `build`, `serve`, `config`.
- `src/config.rs` — the 3-level cascading config (little → theme → site): the
  chain of themes a site uses, and its source and destination folders.
- `src/little.yml` — little's own defaults (level 1), compiled into the binary.
- `src/render.rs` — Markdown (comrak) + YAML frontmatter → HTML fragment.
- `src/site.rs` — build pipeline: walk the source (`docs/`), render, apply
  theme, write the destination (`dist/`).
- `src/theme.rs` — a theme's templates and static files, layered over those of
  the themes it builds on, and the built-in theme; picks each page's template
  by its type (`home`, `section`, `page`, `404`). A theme is built in, in a
  folder, or downloaded.
- `src/download.rs` — a theme downloaded from a URL: fetched over HTTPS,
  unpacked from its `.zip` or `.tar.gz`, and kept in the site's `.little/`, so
  it's downloaded only once.
- `src/server.rs` — dev server + file watcher + live reload.

Content and theme live outside `src/`:
- `docs/` — the example/dogfood site content. This is little's **input**, not
  project documentation — don't put meta docs here.
- `theme/` — little's built-in theme, compiled into the binary: `_templates/`
  (MiniJinja; `default.html` is required) + `_static/` (copied to the site's
  root) + an optional `little.yml`. List every file in it in `builtin` in
  `src/theme.rs`; a test checks.
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
- Prefer small, well-maintained crates; justify any heavy dependency.
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
- **Plugins:** sandboxed WASM (planned).
