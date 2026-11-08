---
title: Rust docs
description: Add a page for a Rust crate and each of its modules, from their doc comments, with the rustdoc plugin.
---

# Rust docs

`rustdoc`, one of
[little's own plugins](using-plugins.md#littles-own-plugins), adds an API
reference to your site: a [page](using-plugins.md#pages-and-files) for a
Rust crate and each of its modules, from their doc comments. It's named for
Rust's own [rustdoc](https://doc.rust-lang.org/rustdoc/), but doesn't run
it, or Cargo: it reads your code with [syn](https://github.com/dtolnay/syn)'s
parser, without compiling it, so it needs neither Rust nor your crate's
dependencies.

Name it in your site's [`plugins`](using-plugins.md#naming-a-plugin), with
the folder your crate's code is in, and the crate's name:

```yaml
# little.yml
plugins:
  rustdoc:
    wasm: https://github.com/withlittle/little/releases/download/rustdoc-v0.1.0/little_rustdoc.wasm
    read: ./src
    crate: my_crate
    title: API reference
    url: /api
```

- `read` names the folder of the crate's `lib.rs` or `main.rs`, like
  `./src`. Where it has both, it documents `lib.rs`, the crate's library.
- `crate` is the crate's name, as its code names it, like `my_crate` for a
  package called `my-crate`. It's the heading of the crate's page, and its
  title and folder too, unless you set `title` and `url`. A link to a path
  that starts with it, like ``[`my_crate::Config`]``, leads where
  `crate::Config` does.
- `title`, if you set it, is the
  [title](../writing/frontmatter.md#a-pages-title) of the crate's page,
  like `API reference`, in place of its name.
- `url` is the folder its pages go in, from the source's folder, like
  `/api`, or `/` for the source's own. It's the crate's name, like
  `/my_crate`, unless you set it.

It documents one crate. For each other, like those of a workspace, add
another entry under `plugins`, with another name, and the same `wasm`:

```yaml
# little.yml
plugins:
  core_api:
    wasm: https://github.com/withlittle/little/releases/download/rustdoc-v0.1.0/little_rustdoc.wasm
    read: ./core/src
    crate: my_core
  cli_api:
    wasm: https://github.com/withlittle/little/releases/download/rustdoc-v0.1.0/little_rustdoc.wasm
    read: ./cli/src
    crate: my_cli
```

## Its pages

For the first example above, where `src/lib.rs` declares `mod config;` and
`mod site;`, and `src/site.rs` declares `mod versions;`, it adds:

- `api/README.md`, at `/api/`, for the crate, titled `API reference`. It
  has the crate's docs, and a table of its modules, each with the first
  paragraph of its docs, linked to its page. Then it has a table of what it
  re-exports, and what `lib.rs` defines.
- `api/config.md`, at `/api/config`, for the module in `src/config.rs`.
- `api/site/README.md`, for `site`, which declares a module, the same way.
- `api/site/versions.md`, for that module, in `src/site/versions.rs`.

It finds each module as rustc does, from the `mod` that declares it, in
`config.rs`, `config/mod.rs`, the file a `#[path]` names, or inline, so a
file no `mod` declares, like one in `src/bin/`, isn't read. Every module it
finds has a page, `pub` or not, as `cargo doc` gives a program's, but for
one with `#[cfg(test)]` or `#[doc(hidden)]`. A module whose file it can't
find, read, or parse is [warned of](using-plugins.md#when-a-plugin-fails),
and left out, with the modules it declares.

## What a page shows

A page looks like rustdoc's, in your site's theme. It shows:

- Its module's name, labelled `crate` or `module`, and its docs, from its
  `//!` comments and those on its `mod`.
- Tables of the modules it declares, what it re-exports, and each kind of
  item it defines, each linked to its page or heading.
- Each item under a heading of its name, in code, like ``## `Config` ``,
  labelled with its kind, like `struct`, `trait`, or `type alias`.
- Its macros first, then its structs, enums, constants, statics, traits,
  functions, type aliases, and unions, as rustdoc orders them, each kind in
  order of name.
- Each item's signature, as Rust, laid out as rustfmt would:
  - A function's is without its body.
  - A struct's or union's has its `pub` fields, and `/* private fields */`
    for the rest.
  - A constant's or static's has its value, if it's a literal or a path,
    else `_`.
  - A trait's has `{ ... }` for each body it gives.
  - A macro's has the pattern of each of its rules.
- Its docs, then a table of a struct's or union's `pub` fields, with their
  types, or of an enum's variants, each with its docs.
- The traits it implements, with an `impl` or a `#[derive]`, by name, like
  `Clone`, `Debug`, `Display`, each linked to its heading, if it has one.
- Its source, which a reader opens, under
  ``Source code in `src/config.rs` ``.
- Its `pub` methods, associated functions, and constants, from each of its
  own `impl`s, like `impl Config`, wherever it is in the crate. Each is
  under a heading, like ``### `Config::load` ``, labelled `method` for a
  function that takes `self`, `function` for one that doesn't, or
  `associated constant`, and shown as an item is. A trait's are its own,
  with its associated types.

It documents a module's items that are `pub`, not `pub(crate)`, and have
docs: its structs, enums, unions, traits, functions, type aliases,
constants, and statics. It documents what's written, so an item a macro
makes, like a `bitflags!` type, isn't documented, and of two items with one
name under `#[cfg]`, like one for each platform, it documents the first.

- A `macro_rules!` with `#[macro_export]` is on the crate's page, where
  it's exported, wherever it's written.
- A `pub use` is listed by its path, as it's written, like
  `config::Config`, `config::Config as Settings`, or `crate::site::*`, and
  linked to where it leads, with the first paragraph of its docs, rather
  than documented again.
- It leaves out an item, field, variant, or `impl` with `#[doc(hidden)]`,
  and fields, variants, and members without docs.

## Doc comments

Doc comments are Markdown, as rustdoc reads them: `///` and `//!`,
`/** */` and `/*! */`, and `#[doc = "..."]`.

- A code block with no language is Rust, as is one with only rustdoc's
  words for how it's tested, like `no_run` or `should_panic`.
- In a Rust block, a line that's `#`, or starts with `# `, is left out, as
  rustdoc hides it, and one that starts with `##` shows with `#`.
- A heading, like `# Examples` or `# Errors`, is a bold title, like
  **Examples:**, so a page's table of contents lists only what it
  documents.
- A link to a path leads to the heading of what it names, or to its
  module's page. It's written as rustdoc's
  [intra-doc links](https://doc.rust-lang.org/rustdoc/write-documentation/linking-to-items-by-name.html)
  are: ``[`Config`]``, ``[its config](Config)``, ``[`Config::load`]``,
  ``[`crate::site`]``, ``[`super::render`]``, or ``[`Self::new`]``. It may
  give a kind, like ``[`struct@Config`]``, and have `()` after a function's
  name, or `!` after a macro's.
- A path is read in the module of what it documents, through its `use`s,
  as rustc reads it there.
- A link to a field or a variant leads to its type's heading.
- A link to a path that leads to nothing it documents, like ``[`Vec`]``,
  in the standard library, is its text, in code if that's the path.
