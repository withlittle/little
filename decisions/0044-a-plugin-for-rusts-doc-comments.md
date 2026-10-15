# 0044 — A plugin for Rust's doc comments

- Status: accepted; amended by [0046](./0046-plugin-hooks.md), whose
  hooks it exports in place of `plugin-with-pages`: `setup`, and
  `files`, which adds its pages, and by
  [0047](./0047-naming-api-reference-plugins.md), which renames it
  `rustdoc`, in `plugins/rustdoc/`, after Rust's own tool, which it
  doesn't run
- Date: 2026-10-14
- Amends: [0032](./0032-docs-are-the-manual.md)

## Context
[0031](./0031-a-plugin-for-pythons-docstrings.md) added a plugin that
builds an API reference from Python's docstrings, and planned one for each
other language, sharing a library for what a page shows, and how it's
written. Rust's docs are in its code too, as doc comments, `///` and `//!`,
in Markdown, which rustdoc builds into a site of its own, in its own theme,
apart from a project's other docs.

little is written in Rust, and documents each of its modules, and each of
their `pub` items, with doc comments, for those who work on it. They're read
only in its code, or with `cargo doc`, as little is a program, not a
library, so it's not on docs.rs. little's manual is for people who use
little ([0032](./0032-docs-are-the-manual.md)), and each of its pages uses
what it documents, where it can, but the page on Python's API reference
can't, as little has no Python.

## Options considered
- **How it reads a crate:**
  - rustdoc's JSON, which resolves every name and type, but is only on
    nightly, changes its format from release to release, and needs Cargo,
    which can't run in the sandbox, so a site would run a step of its own
    before `little build`;
  - rust-analyzer's crates, which resolve names too, but bring its whole
    engine, which wants Cargo's metadata, and the standard library's
    source, neither of which is in the sandbox;
  - parsing it, with syn, which reads all of Rust, is pure Rust, and builds
    for `wasm32-wasip2`, with prettyplease to write a signature as rustfmt
    would, but which sees only what's written, not what macros make, nor
    what a name means in another crate.
- **What it documents:** a library's public API, as rustdoc does, which
  needs every re-export followed, through modules that aren't `pub`; or
  each module's `pub` items, where they're defined, which is a library's API,
  and, in a program like little, each module's interface to the rest.
- **The crate's name:** from its `Cargo.toml`, which `read: ./src` doesn't
  reach, while reading `.` would have `little serve` watch `target/` and
  `.git/`, and build again on each `cargo build`; or a setting.
- **Where little's own API goes:**
  - under `/reference/`, beside the commands and settings, where it looks
    like an API a reader calls;
  - in a site of its own, which is a second site to build and publish;
  - or in the manual, at `/api/`, apart from the rest, as the example of
    the plugin's page.
- **Where the manual finds the plugin:** a `.wasm` file at the root of the
  repository, committed, which goes stale, and is easy to commit by
  mistake; or where Cargo builds it, which a clone builds first.

## Decision
- `plugins/doc_comments/` is the plugin, a crate, `little-doc-comments`, in
  its own Cargo workspace, which builds `little_doc_comments.wasm`:

  ```sh
  cargo build --release --target wasm32-wasip2 --manifest-path plugins/doc_comments/Cargo.toml
  ```

  It exports `plugin-with-pages`, renders no blocks, parses Rust with syn,
  and writes signatures with prettyplease.
- docstrings' model and Markdown move to a library,
  `plugins/api_reference/`, `little-api-reference`, in its own workspace,
  which both plugins use, as 0031 planned. Its model has Rust's kinds of
  item beside Python's, and fields and variants beside parameters, and a
  page is written in a language, with its own name for code blocks and its
  own separator, `.` or `::`, and lists of its own, like its modules and
  re-exports. docstrings' pages are as they were.
- A site names it, the folder of the crate's `lib.rs` or `main.rs` in
  `read`, which is one folder, the crate's name in `crate`, and, in `url`,
  the folder its pages go in, which is `/reference/<crate>` unless it's set:

  ```yaml
  plugins:
    doc_comments:
      wasm: ./plugins/doc_comments/target/wasm32-wasip2/release/little_doc_comments.wasm
      read: ./src
      crate: my_crate
  ```

  For another crate, a site adds another entry, with another name, and the
  same `wasm`.
- It finds the crate's modules as rustc does, from each `mod`: in a file
  beside the one that declares it, or in that file's folder, in the file a
  `#[path]` names, or inline. Each has a page, `pub` or not, but those with
  `#[cfg(test)]` or `#[doc(hidden)]`: `README.md` in `url`, for the crate,
  `config.md`, for a module, and `site/README.md`, for one that declares
  others.
- It documents each item that's `pub` and has docs, in the module it's
  defined in, and lists each `pub use` by its path, linked to where it
  leads. A type's page has the `pub` functions and constants, with docs, of
  each `impl` for it, wherever it is in the crate, and the traits it
  implements, with an `impl` or a `#[derive]`.
- A page looks like rustdoc's: tables of its module's modules, re-exports,
  and items, then each item under a heading of its name, like
  ``## `Config` ``, and its members under their own, like
  ``### `Config::load` ``, each with its kind as a label, its signature, its
  docs, its fields or variants, the traits it implements, and its source,
  which a reader opens.
- Doc comments are Markdown, as rustdoc reads them: a code block with no
  language is Rust, without the lines it hides; a heading, like
  `# Examples`, is a bold title, as a docstring's section is; and an
  intra-doc link, like ``[`Config::load`]``, is read in the module of what
  it documents, as rustdoc reads it, and leads to its heading, else is code.
- A module it can't find or parse, and a `#[doc]` that isn't a string, like
  an `include_str!`, are warned of, and left out.
- little's manual documents little with it, at `/api/`, as the example on
  the page "Rust API reference", for those who work on little, whom 0032
  left out of the manual. Only `/api/` is in its `nav`, under that page, so
  the manual reads in the order it did, and the pages of little's modules
  are reached from there. Its `little.yml` names the `.wasm` file where
  Cargo builds it, which git ignores.

## Consequences
- A Rust crate gets an API reference in its site, in its theme, and found
  by its search, without Cargo, nightly, or the crate's dependencies, so a
  site builds it offline, in its sandbox, the same each time.
- The plugin is about 1.2 MB, and adds about a quarter of a second to a
  build of little's 18 modules, in 16 files of about 16,000 lines.
- little's doc comments are published, so they're written for readers, and
  their links to each other's items are checked with the manual's.
- A clone of little's repository builds the plugin before the manual:
  `cargo run -- build` stops until it's built, and a workflow that
  publishes the manual will build it first.
- The manual's search finds little's modules beside its pages.
- A plugin for another language starts with the library, adding its kinds
  of item to it.
- Limits, for now:
  - it sees only what's written, so not an item a macro makes, and
    documents the first of two items, or modules, of the same name, like
    one for each platform;
  - it documents only `pub` items, even in a program, which `cargo doc`
    documents in full, and, in a library, a module that isn't `pub` has a
    page, where rustdoc documents what's in it in the module that
    re-exports it;
  - a type's page doesn't list the methods a trait gives it, or those it
    gets from `Deref`, and an `impl` for another crate's type, or for every
    `T`, isn't documented;
  - a type in a signature isn't linked, and a link to the standard library,
    or another crate, is code;
  - the source has no line numbers, and doesn't link to the file in the
    site's repository;
  - the `.wasm` file isn't released, so a site builds it.