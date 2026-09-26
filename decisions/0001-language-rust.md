# 0001 — Build little in Rust

- Status: accepted
- Date: 2026-09-26

## Context
little needs to deliver a single-binary, "drop a folder → wham, a site"
experience with Jekyll-grade themeability and fast builds that scale to large,
versioned sites — without the dependency sprawl of the JS ecosystem. The
maintainer had no prior Rust experience and initially leaned toward
Node/TypeScript, so we built a walking skeleton to decide from evidence.

## Options considered
- **Rust** — single self-contained binary, fastest builds, flexible Jinja-style
  templating (MiniJinja), sandboxed WASM plugins, no runtime. Cost: steeper
  language for the maintainer; themes use a templating language, not components.
- **Go** — single binary and fast, but its templating culture is the Hugo pain we
  explicitly reject.
- **Node/TypeScript** — best component/MDX theming and most familiar, but breaks
  the single-binary promise, is slowest at scale, and brings `node_modules` sprawl.

## Decision
**Rust.** It is the only option that satisfies both hard differentiators —
single-binary simplicity *and* deep themeability — without the ecosystem weight
we set out to avoid. Our workflow mitigates the learning curve: the agent writes
idiomatic Rust and the compiler localizes mistakes precisely; the maintainer
reviews behavior, architecture, and taste.

## Evidence from the skeleton
- Toolchain installed with one command (~90 s), bundling cargo + test + fmt + clippy.
- ~456 lines of Rust for the whole working core.
- First build a few minutes; incremental rebuilds ~2 s.
- Output: a single ~5.6 MB binary linking only system libraries — nothing to install.

## Consequences
- Themes are authored in MiniJinja (structured data → HTML), not React/MDX.
- Plugins will be sandboxed WASM so they stay language-agnostic.
- A smaller contributor pool than JS; accepted given the product benefits.
