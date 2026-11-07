# 0070 — little's API out of the manual

- Status: accepted
- Date: 2026-11-07
- Supersedes: [0067](./0067-littles-api-at-the-end-of-the-manuals-nav.md)
- Amends: [0044](./0044-a-plugin-for-rusts-doc-comments.md),
  [0056](./0056-publishing-the-manual.md)

## Context
The manual documents little's own code with the `rustdoc` plugin, at
`/api/`, a page for little's crate and each of its modules, as the example
on the page "Rust API reference"
([0044](./0044-a-plugin-for-rusts-doc-comments.md)), and, since
[0067](./0067-littles-api-at-the-end-of-the-manuals-nav.md), lists it as a
link of its own, "API", at the end of its `nav`.

But the manual is for people who use little
([0032](./0032-docs-are-the-manual.md)), and little's API is for those who
work on it: how little's code works, not how to use little. A reader still
meets it in the nav, as the manual's last page, and the manual's search
finds little's modules, and their items, beside its pages. It's a
distraction, as these records were, until
[0065](./0065-decisions-at-the-root.md) moved them out of the manual.

Those who work on little read its doc comments in its code, or with
`cargo doc`, which documents a program's private items too. And the
manual's other pages on API references have no example of their own, as
little has no Python, JavaScript, Go, or OpenAPI document.

## Options considered
- **At the end of the nav:** as 0067 has it, apart from the sections for
  people who use little, but still in their manual, and their search.
- **Out of the nav and search, but built:** reached only from the page
  "Rust API reference", as its example, and left out of search with
  `search.exclude: [/api]` ([0061](./0061-leaving-pages-out-of-search.md)),
  so still built, checked, and published, for few readers.
- **Out of the manual:** read in the code, or with `cargo doc`, as before
  0044.

## Decision
- The manual no longer documents little's code: its `little.yml` no longer
  names the `rustdoc` plugin, or lists `/api/` in its `nav`, so it has no
  `/api/`.
- `.github/workflows/docs.yml` no longer builds `rustdoc` before the
  manual, but still builds the plugins that read its pages in their
  formats.
- little's doc comments stay, as AGENTS.md asks for them: a `//!` for each
  module, and a `///` for each `pub` item, in the manual's plain style
  ([0066](./0066-the-manuals-plain-style.md)), with intra-doc links to
  items by path, for those who read its code.
- The page "Rust API reference" no longer links to little's API, so it has
  no example of its own, as the manual's other pages on API references
  have none. The `rustdoc` plugin doesn't change.

## Consequences
- The manual's nav ends with Reference, and its search finds only its own
  pages.
- Once the manual is published again, its URLs under `/api/` lead nowhere,
  as little has no redirects.
- little's doc comments aren't published, so nothing builds them into
  pages for each pull request: `rustdoc`'s test of little's own `src/`
  still fails on a module it can't find or parse, but nothing checks where
  their intra-doc links lead.
- The manual's build needs one plugin fewer, in CI and in a clone.
- The built-in theme still puts a group's gap above a page at the nav's
  top level that follows a group, as 0067 had it do, for any site.
- `rustdoc` is still tested, built, and released, as each plugin is.
