---
title: Rust API reference
description: Add a page for a Rust crate and each of its modules, from their doc comments, with the doc_comments plugin.
---

# Rust API reference

little's repository has a plugin,
[`doc_comments`](https://github.com/withlittle/little/tree/main/plugins/doc_comments),
that adds an API reference to a site: a page for a Rust crate and each of its
modules, built from their doc comments, as
[rustdoc](https://doc.rust-lang.org/rustdoc/) builds them. It reads the
crate's code with [syn](https://github.com/dtolnay/syn)'s parser, without
compiling it, so it needs neither Cargo nor the crate's dependencies, and a
site builds the same each time, offline. Why it works as it does is in
[ADR 0044](https://github.com/withlittle/little/blob/main/decisions/0044-a-plugin-for-rusts-doc-comments.md).

This manual uses it on little's own code: [little's API](../api/README.md)
is a page for each of little's modules, for those who work on little.

## Building it

Its `.wasm` file isn't released yet, so build it, in a clone of little's
repository:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/doc_comments/Cargo.toml
```

That builds
`plugins/doc_comments/target/wasm32-wasip2/release/little_doc_comments.wasm`,
about 1.2 MB, which a site can name where it is, or copy into its folder.

## Using it

Name it in your site's `little.yml`, with the folder your crate's code is in,
and the crate's name:

```yaml
# little.yml
plugins:
  doc_comments:
    wasm: ./little_doc_comments.wasm
    read: ./src
    crate: my_crate
```

- `read` names the folder the crate's `lib.rs` or `main.rs` is in, from the
  site's folder, like `./src`. Where it has both, it documents `lib.rs`, the
  crate's library.
- `crate` is the crate's name, as its code names it, like `my_crate` for a
  package called `my-crate`. It's the title of the crate's page, and a link
  to a path that starts with it, like ``[`my_crate::Config`]``, leads where
  `crate::Config` does.
- `url` is the folder its pages go in, from the source's folder:
  `/reference/my_crate` unless you set it, like `url: /api`, or `url: /` for
  the source's own.

It has no other settings. One it doesn't have, or no `read` or `crate`,
stops the build, saying why.

It documents one crate. For each other, like those of a workspace, add
another entry under `plugins`, with another name, and the same `wasm`:

```yaml
# little.yml
plugins:
  core_api:
    wasm: ./little_doc_comments.wasm
    read: ./core/src
    crate: my_core
  cli_api:
    wasm: ./little_doc_comments.wasm
    read: ./cli/src
    crate: my_cli
```

## Its pages

For `crate: my_crate`, where `src/lib.rs` declares `mod config;` and
`mod site;`, and `src/site.rs` declares `mod versions;`, it adds:

- `reference/my_crate/README.md`, at `/reference/my_crate/`, for the crate:
  its docs, a table of its modules, each with the first paragraph of its
  docs, linked to its page, a table of what it re-exports, and what
  `lib.rs` defines;
- `reference/my_crate/config.md`, at `/reference/my_crate/config`, for the
  module in `src/config.rs`;
- `reference/my_crate/site/README.md`, for `site`, which declares a module,
  the same way, and `reference/my_crate/site/versions.md`, for that module,
  in `src/site/versions.rs`.

Each is built as if it were a Markdown file at that path in the site's
source, so it's in the nav and search, and its links are checked. A file in
the source at the same path stops the build, naming both, but
`docs/reference/README.md` is yours to write, if you like, linking to each
crate's page by its path, like `[my_crate](my_crate/README.md)`. A site with
[a nav of its own](../writing/navigation.md#writing-the-nav-yourself) lists
the pages it wants: this manual lists only little's crate's page, which
links to the rest.

It finds each module as rustc does, from the `mod` that declares it:

- in `config.rs` or `config/mod.rs`, in the folder of the `lib.rs`,
  `main.rs`, or `mod.rs` that declares it, or, for one that `site.rs`
  declares, in `site/`, like `site/versions.rs`;
- in the file a `#[path]` names;
- or inline, in `mod config { ... }`.

Every module it finds has a page, `pub` or not, as `cargo doc` gives a
program's, but for one with `#[cfg(test)]` or `#[doc(hidden)]`. A file no
`mod` declares, like one in `src/bin/`, isn't read.

## What a page shows

A page looks like rustdoc's, in the site's theme:

- its module's name, labelled `crate` or `module`, its docs, from its `//!`
  comments and those on its `mod`, and tables of the modules it declares,
  what it re-exports, and each kind of item it defines, each linked to its
  page or heading;
- each item under a heading of its name, in code, like ``## `Config` ``,
  labelled with its kind, like `struct`, `trait`, or `type alias`: its
  macros, then its structs, enums, constants, statics, traits, functions,
  type aliases, and unions, as rustdoc orders them, each in order of name;
- each item's signature, as Rust, laid out by
  [prettyplease](https://github.com/dtolnay/prettyplease) as rustfmt would:
  a function's without its body; a struct's or union's with its `pub`
  fields, and `/* private fields */` for the rest; a constant's or static's
  with its value, if it's a literal or a path, else `_`; a trait's with
  `{ ... }` for each body it gives; and a macro's with the pattern of each
  of its rules;
- its docs, and a table of a struct's or union's `pub` fields, with their
  types, or of an enum's variants, each with its docs;
- the traits it implements, with an `impl` or a `#[derive]`, by name, like
  `Clone`, `Debug`, `Display`, each linked to its heading, if it has one;
- its source, which a reader opens, under
  ``Source code in `src/config.rs` ``, with the folder as `read` names it;
- its methods, associated functions, and constants, from each `impl` for it
  in the crate, wherever it is, or, for a trait, its own, and its associated
  types, each under a heading, like ``### `Config::load` ``, labelled
  `method`, for a function that takes `self`, `function`, or
  `associated constant`, and shown as an item is.

It documents a module's items that are `pub` and have docs: its structs,
enums, unions, traits, functions, type aliases, constants, and statics. A
`macro_rules!` with `#[macro_export]` is on the crate's page, where it's
exported, wherever it's written. A `pub use` is listed by its path, as it's
written, like `config::Config`, `config::Config as Settings`, or
`crate::site::*`, linked to where it leads, with the first paragraph of its
docs, rather than documented again. It leaves out an item, field, variant,
or `impl` with `#[doc(hidden)]`, and fields, variants, and members without
docs.

## Doc comments

Doc comments, `///` and `//!`, `/** */` and `/*! */`, and
`#[doc = "..."]`, are Markdown, as rustdoc reads them:

- A code block with no language, or with only rustdoc's words for how it's
  tested, like `no_run` or `should_panic`, is Rust, without the lines that
  start with `# `, which rustdoc hides; a line that starts with `##` shows
  with `#`.
- A heading, like `# Examples` or `# Errors`, is a bold title, like
  **Examples:**, so a page's table of contents lists only what it
  documents.
- A link to a path, as rustdoc's
  [intra-doc links](https://doc.rust-lang.org/rustdoc/write-documentation/linking-to-items-by-name.html)
  are written, leads to the heading of what it names, or to its module's
  page: ``[`Config`]``, ``[its config](Config)``, ``[`Config::load`]``,
  ``[`crate::site`]``, ``[`super::render`]``, or ``[`Self::new`]``, with a
  kind or not, like ``[`struct@Config`]``, and `()` after a function's name,
  or `!` after a macro's. A path is read in the module of what it documents,
  through its `use`s, as rustc reads it there. A field or a variant leads
  to its type's heading.
- A link to a path that leads to nothing it documents, like ``[`Vec`]``, in
  the standard library, is its text, in code if that's the path.

## Warnings

What it can't read is warned of, and `little build --strict` fails on each:

- a module whose file it can't find, read, or parse, which it leaves out,
  with the modules it declares:

  ```text
  warning: plugin `doc_comments`: can't parse `src/config.rs`: expected `:` on line 12
  ```

- a `#[doc]` that isn't a string, like
  `#[doc = include_str!("../README.md")]`, which it leaves out;
- a folder in `read` with no `lib.rs` or `main.rs` in it.

## Limits, for now

- It reads only Rust, and only what's written: an item a macro makes, like
  a `bitflags!` type, isn't documented.
- Where a module has two items, or modules, of the same name, like one for
  each platform, under `#[cfg]`, it documents the first.
- An item that isn't `pub`, like one that's `pub(crate)`, isn't
  documented, even in a program, which `cargo doc` documents in full.
- In a library, a module that isn't `pub` has a page, which rustdoc leaves
  out, and a `pub use` of what's in it links there, where rustdoc documents
  it again, in the module that re-exports it.
- A type's page doesn't list the methods a trait gives it, or those it gets
  from `Deref`, and an `impl` for another crate's type, or for every `T`,
  isn't documented.
- A type in a signature isn't linked to its heading, and a link to the
  standard library, or another crate, isn't linked to its docs.
- The source has no line numbers, and doesn't link to the file in the
  site's repository.
- Its `.wasm` file isn't released, so you build it.