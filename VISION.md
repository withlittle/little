# little — Vision

little is a static **documentation** site generator whose mission is to make
*authoring* documentation effortless: drop Markdown in a folder and get a great
site, with the freedom to theme it completely.

## North star
**Authoring experience.** Markdown that just works, minimal configuration, and
excellent defaults. Every feature is judged by whether it makes writing and
maintaining docs easier. Everything else serves this.

## Who it's for
Everyone on the documentation spectrum. little must **scale gracefully from a
single page to a large, versioned product site** without switching tools or
rewriting content.

## What we steal, what we reject
little is a deliberate synthesis of the best ideas from the tools we've used —
and a rejection of their worst friction.

| Tool | Steal | Reject |
| ---- | ----- | ------ |
| **MkDocs** | Drop Markdown in `docs/` and *wham* — a site, near-zero config | Painful, restrictive theming |
| **Jekyll** | Total themeability: the theme receives **raw structured data**, no baked-in HTML, so any framework (Tailwind, Bootstrap) just works | Slow Ruby builds; dated tooling |
| **Sphinx** | Best-in-class docstring / API extraction | Narrow: does one thing well, everything else poorly; reStructuredText friction — so in little this is a **plugin**, never the core |
| **Hugo** | One fast binary, folder-to-site simplicity | Painful templating |
| **Astro / 11ty** | Fast, modern, content-first | Config sprawl; JS-ecosystem weight for pure docs |

## Superpowers (what little does that others don't)
1. **3-level cascading config** — `little` defaults → **theme** → **site**, each
   deep-merging over the previous, last writer wins. Themes ship sensible config;
   a site overrides only what it needs. (`src/config.rs`.)
2. **Jekyll-grade themeability with zero baked HTML** — little hands the theme
   structured `site` / `page` / `nav` data; the theme owns 100% of the markup.
3. **Single self-contained binary** — download one file, run `little serve`. No
   runtime, no `node_modules`, no version managers.

## Non-goals
- Not a general-purpose website/app framework — documentation first.
- No baked-in HTML or forced theme in the core.
- No mandatory Node/JS toolchain to build a site.
- Kitchen-sink features live in **plugins**, keeping the core small.

## Tech foundation
- **Language:** Rust — single binary, fast builds at scale, no runtime deps.
- **Markdown:** `comrak` (CommonMark + GFM), with code highlighted as each
  page is built (syntect, with bat's grammars, built in), math rendered as
  MathML, and Mermaid diagrams drawn as SVG, which browsers show without a
  script (math-core and merman, built in).
- **Notebooks:** Jupyter's `.ipynb`, outputs and all, built into comrak's
  tree, so a notebook is a page as Markdown is, without Python.
- **Theming:** `minijinja` templates; structured data in, any HTML out.
- **Config:** cascading YAML.
- **Plugins:** sandboxed **WASM** components (wasmtime, built in), so
  plugins are language-agnostic, and one `.wasm` file runs wherever little
  does. For now, they render fenced code blocks in languages of their own
  (e.g. Sphinx-style docstring extraction as a plugin).
- **Styling:** bring-your-own; little compiles a theme's Sass (grass, built
  in), and pairs with the standalone Tailwind CLI (no Node).
- **Search:** Pagefind, built in, indexes each page as it's built, for a
  theme's search box to search in the browser.

## Roadmap (thin vertical slices)
Each slice ships as a small, reviewable PR, dogfooded against little's own manual in `docs/`.

- [x] **0 — Walking skeleton:** Markdown → themed HTML, cascading config, live reload.
- [x] **1 — Navigation & structure:** nested sections, explicit ordering, sidebar tree.
- [x] **2 — Authoring essentials:** syntax highlighting, callouts/admonitions, per-page TOC, heading anchors.
- [ ] **3 — `little init` + default theme:** scaffold a site in one command; a polished, responsive, accessible default theme.
- [x] **4 — Search:** client-side search index out of the box.
- [x] **5 — Plugin API (WASM):** sandboxed plugins; first plugin — docstring / API extraction.
- [x] **6 — Versioning:** multiple doc versions for large product sites.
- [ ] **7 — Performance:** incremental builds for large sites.
