# AGENTS.md — working conventions for little

Read this before making changes. **little** is a static documentation site
generator written in Rust. Its mission is **authoring experience** — see
[`VISION.md`](./VISION.md).

Keep agents' instructions here, not in a `CLAUDE.md`: Claude Code reads
this file only while there's no `CLAUDE.md` or `CLAUDE.local.md`. Keep it
short: say how to work on little, and point to what's written elsewhere,
as [0073](./decisions/0073-agents-md-points-to-whats-written-elsewhere.md)
sets.

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

# each plugin, like pydoc, in its own workspace; its tests run natively
cargo build --release --target wasm32-wasip2 --manifest-path plugins/pydoc/Cargo.toml
cargo test --manifest-path plugins/pydoc/Cargo.toml
cargo clippy --manifest-path plugins/pydoc/Cargo.toml --all-targets --target wasm32-wasip2

# each library the plugins share, api_reference and markup, natively
cargo test --manifest-path plugins/markup/Cargo.toml
cargo clippy --manifest-path plugins/markup/Cargo.toml --all-targets

# the third-party notices releases carry, with cargo-about 0.9.2
# (cargo install --locked cargo-about@0.9.2)
scripts/notices.sh little target/notices/little.md # or any plugin, like pydoc
```

The manual uses the plugins its `little.yml` names, from where Cargo
builds them: build them before you build the manual.

## Where things are
Each module in `src/` says what it does in its `//!`, and each folder in
`plugins/` in its README. Read them before you change one, and keep them
up to date as you do.
- `src/` — little itself: `main.rs`, the CLI; `site.rs`, the build; and
  `testing.rs`, what the modules' tests share. `src/little.yml` holds
  little's own defaults, compiled in.
- `theme/` — the built-in theme, compiled in. List each of its files in
  `builtin` in `src/theme.rs`; a test checks.
- `docs/` — the manual, for people who use little, built by little, so
  each page uses what it documents where it can. Its pages link to each
  other by their files, and to little's other files on GitHub. Meta docs,
  like ADRs, don't go there, and its pages don't link to `decisions/`.
- `little.yml` — the manual's config. It sets `theme: ./theme`, so edits
  to the built-in theme live-reload.
- `wit/plugin.wit` — `little:plugin`, the hooks a plugin can export.
- `plugins/` — the plugins, each in its own workspace, and the libraries
  they share, `api_reference` and `markup`.
- `tests/plugin/` — a plugin that exports every hook, which little's
  tests build and drive.
- `.github/workflows/` — CI, the manual's build, and releases, each
  explained in its comments; `RELEASING.md` says how to release.
- `decisions/` — the ADRs: see [Decisions](#decisions).

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
- Give each module in `src/` a `//!`, and each `pub` item a `///`, in the
  [manual's style](#writing-the-manual), and link to items by path,
  like ``[`Config`]``.
- Add unit tests for parsing/merging logic (frontmatter, config cascade).
- A change people using little can see updates its page in the manual
  (`docs/`), and the reference pages, in the same PR; a new page goes in
  `nav` in `little.yml`. Keep the README short: it links to the manual.
  `cargo run -- build --strict` checks the manual's links, as the Docs
  workflow does for each pull request.
- "The little static documentation site generator for big things." is
  little's tagline. Use it word for word, with its full stop, wherever
  little describes itself to its users, like `Cargo.toml`,
  `little --help`, and the READMEs.
- Versions follow [Semantic Versioning](https://semver.org/): each
  component's, in its `Cargo.toml`, is raised as it's released, and
  `little:plugin`'s, in `wit/plugin.wit`, by the change that needs it.
  Until 1.0, a feature or a breaking change raises the minor version,
  0.1.4 to 0.2.0, and anything else the patch, 0.1.4 to 0.1.5, as
  [0058](./decisions/0058-features-raise-the-minor-version.md) sets;
  but a new hook raises only `little:plugin`'s patch, so plugins built for
  0.2.0 still load. A change is breaking if it breaks what the manual
  documents for sites, themes, or plugins to rely on, as
  [0051](./decisions/0051-semantic-versioning-and-conventional-commits.md)
  lists.
- little and its plugins are released from tags, as
  [`RELEASING.md`](./RELEASING.md) says. A release can't be undone, so
  don't push a tag: the maintainer does.
- Commits follow [Conventional Commits](https://www.conventionalcommits.org/),
  as `committed.toml` checks in CI: `type(scope)!: description`, with a
  lowercase, imperative description, no full stop, and a first line of up
  to 100 characters. `committed main..HEAD`, after
  `cargo install committed`, checks a branch as CI does.
  - Type: `feat`, `fix`, or `perf` for a change to what people use, which
    is released; `docs`, `refactor`, `test`, `style`, `build`, `ci`, or
    `chore` for any other; `revert`, with a `Refs:` footer, to undo one.
  - Scope: the plugin or library a commit is about, as `committed.toml`
    lists them; none for little, its theme, `wit/`, or more than one
    component.
  - Breaking: `!` before the `:`, and a footer,
    `BREAKING-CHANGE: <what to change>`, beside the co-author trailer;
    written `BREAKING CHANGE:`, with a space, git misses the trailer.
  - A commit is one change, with its tests, manual pages, and ADR. A new
    plugin is three: `feat(<plugin>)`, for its folder and `committed.toml`,
    with its page in the manual if its tests read it; `ci:`, for its
    workflows and `RELEASING.md`; and `docs:`, for the rest, like its ADR
    and nav. A change to a workflow is a `ci:` commit of its own, with
    `RELEASING.md`'s lines about it; one to `RELEASING.md` alone is `docs:`.
  - Include your co-author trailer. Rebase a branch on `main`, rather
    than merging `main` into it.
- A pull request's title is a commit's first line: its main commit's, or
  one that sums its commits up. Its body opens with a paragraph on what
  it does and why, then has `## What changes`, a list by file or area;
  `## What to know`, for what the diff doesn't show, like what was
  checked by hand, or left as it was, and why; and `## Checks`, last, the
  commands run, like `cargo clippy --all-targets` and
  `cargo run -- build --strict`, and what they found.

## Writing the manual
The manual is for people who use little. Write so they find what they
need, do it, and stop reading, as
[0072](./decisions/0072-the-manuals-concise-style.md) sets. The README
follows it where it's for them, and so do doc comments in `src/`.
- **One home for each fact.** State it once, on the page a reader would
  look for it, and link to it from anywhere else. Guides teach a task;
  Reference holds the rules a reader looks up. A pointer elsewhere is a
  line and a link, not a second copy.
- **Lead with the task.** Open each page with what most readers type or
  write: a command, a setting, or some Markdown. Put the common case
  first, and end a guide when its task is done.
- **Document little, not the tools around it.** Assume readers know git,
  their CI service, and how they release. Don't assume they use GitHub:
  what's only for one host, like a GitHub Pages workflow, goes in a
  section of its own. Saying little reads Markdown as GitHub does is fine.
- **Say what little does.** No "Limits" sections, or lists of what it
  doesn't do yet. Leave out how it works inside, browser support, and
  rules whose error message says how to fix them.
- **Write natural sentences.** Address the reader as "you". Join ideas
  that belong together, and vary the length. Don't open sentences with
  "So", "Then", or "But" by habit, or compare everything with "as X does".
- **Lists and tables are for lists and lookups**, not for paragraphs of
  rules. Keep table cells short.
- **A rewrite may cut.** Keep each heading a link leads to, or update the
  links, and keep pages' file names, as little has no redirects. Code,
  and what little prints, stay as they are.

## Decisions
Notable choices are recorded as ADRs in `decisions/`, which its
`README.md` lists by title. Before you change or add to what little does,
read the records about it, and any later record each one's `Status:`
names: a record says what was decided when it was made, and the manual
says how little is now.

Record a notable choice as a short ADR, with the next number, and list it
in `decisions/README.md`. One that changes an earlier record names it in
its header, as `Amends:` or `Supersedes:`, and the earlier one's
`Status:` names it back, with what it changed.
