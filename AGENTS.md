# AGENTS.md — working conventions for little

Read this before making changes. **little** is a static documentation site
generator written in Rust. Its mission is **authoring experience** — see
[`VISION.md`](./VISION.md).

## Commands
```sh
cargo run -- build          # build the example site (docs/) into dist/
cargo run -- serve          # live-reloading dev server at http://127.0.0.1:8080
cargo build --release       # produce the release binary (target/release/little)
cargo test                  # run tests
cargo clippy --all-targets  # lint — keep it clean
cargo fmt                   # format — run before committing
```

## Architecture (keep the core small)
- `src/main.rs` — CLI (clap): `build`, `serve`.
- `src/config.rs` — the 3-level cascading config (little → theme → site), and
  which theme folder a site uses.
- `src/little.yml` — little's own defaults (level 1), compiled into the binary.
- `src/render.rs` — Markdown (comrak) + YAML frontmatter → HTML fragment.
- `src/site.rs` — build pipeline: walk `docs/`, render, apply theme, write `dist/`.
- `src/server.rs` — dev server + file watcher + live reload.

Content and theme live outside `src/`:
- `docs/` — the example/dogfood site content. This is little's **input**, not
  project documentation — don't put meta docs here.
- `theme/` — the default theme: `templates/` (MiniJinja) + `assets/` + an optional `little.yml`.
- `little.yml` — example site config (optional).
- Project meta (`VISION.md`, `AGENTS.md`, `decisions/`) lives at the repo root.

## Core principles (do not violate)
1. **The theme owns 100% of the markup.** The core never emits baked-in HTML; it
   passes structured `site` / `page` / `nav` data to the theme. Keep it that way.
2. **Config cascades: little → theme → site, last writer wins.** New config
   belongs in this cascade. Only settings little itself owns get a built-in
   default in `src/little.yml`; a site's name, language, and look are the
   theme's to default (in its `little.yml`) and the site's to set. The theme's
   and site's `little.yml` are optional — a missing or empty file just
   contributes nothing. `theme` is the exception: it chooses level 2, so only
   the site can set it, and a site that doesn't gets little's built-in theme.
3. **Keep the core small; push features to plugins.** Sphinx-style API extraction,
   niche renderers, etc. are plugins (planned WASM API), not core code.
4. **No mandatory Node toolchain.** Never add a build step that requires Node.
5. **Authoring experience wins ties.** When unsure, pick what makes writing docs
   simpler.

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
- **Plugins:** sandboxed WASM (planned).
