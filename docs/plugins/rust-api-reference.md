---
title: Rust API reference
description: Add a page for a Rust crate and each of its modules, from their doc comments, with the rustdoc plugin.
---

# Rust API reference

[`rustdoc`](https://github.com/withlittle/little/tree/main/plugins/rustdoc),
a plugin in little's repository, adds an API reference to a site. It adds a
page for a Rust crate and each of its modules, from their doc comments, as
Rust's own [rustdoc](https://doc.rust-lang.org/rustdoc/) builds them.

It's named for rustdoc, but doesn't run it. It reads the crate's code with
[syn](https://github.com/dtolnay/syn)'s parser, without compiling it. So it
needs neither Cargo nor the crate's dependencies, and a site builds the same
each time, offline.

## Using it

Name it in your site's `little.yml`, with the folder your crate's code is
in, and the crate's name. If you like, set what its page is called, and
where its pages go:

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

- `read` names the folder the crate's `lib.rs` or `main.rs` is in, from the
  site's folder, like `./src`. Where it has both, it documents `lib.rs`,
  the crate's library.
- `crate` is the crate's name, as its code names it, like `my_crate` for a
  package called `my-crate`. It's the heading of the crate's page. It's
  its title and folder too, unless you set `title` and `url`.
- A link to a path that starts with the crate's name, like
  ``[`my_crate::Config`]``, leads where `crate::Config` does.
- The plugin needs `crate`, as the name is in `Cargo.toml`. That's outside
  the folder it reads, which is all it can see.
- `title`, if you set it, is the
  [title](../writing/frontmatter.md#a-pages-title) of the crate's page,
  like `API reference`, in place of the crate's name. The name is still its
  heading.
- `url` is the folder its pages go in, from the source's folder. It's the
  crate's name, like `/my_crate`, unless you set it, like `url: /api`, or
  `url: /` for the source's own.

It has no other settings. One it doesn't have, or no `read` or `crate`,
stops the build, saying why.

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

## Getting it

Each version of `rustdoc` is released in little's repository, as a `.wasm`
file, about 1.2 MB. A site names it by its URL, with the version in it, like
`rustdoc-v0.1.0` above.

little downloads it in the site's first build, and uses that copy from then
on, as for [any plugin](using-plugins.md#naming-a-plugin). So to update it,
change the URL to a later version's.

- Its releases, among
  [little's](https://github.com/withlittle/little/releases), list what
  changed in each version, and the version of little's
  [hooks](../reference/plugin-hooks.md) it's built for.
- Each release carries a `THIRD-PARTY-NOTICES.md`, with the licences of
  the crates it's built from.
- To build it yourself, see its
  [README](https://github.com/withlittle/little/tree/main/plugins/rustdoc#building-it).

## Its pages

Take the first example in [Using it](#using-it). There, `src/lib.rs`
declares `mod config;` and `mod site;`, and `src/site.rs` declares
`mod versions;`. So it adds:

- `api/README.md`, at `/api/`, for the crate, titled `API reference`. It
  has the crate's docs, and a table of its modules, each with the first
  paragraph of its docs, linked to its page. Then it has a table of what it
  re-exports, and what `lib.rs` defines.
- `api/config.md`, at `/api/config`, for the module in `src/config.rs`.
- `api/site/README.md`, for `site`, which declares a module, the same way.
- `api/site/versions.md`, for that module, in `src/site/versions.rs`.

Each is built as if it were a Markdown file at that path in the site's
source. So it's in the nav and search, and its links are checked.

- A file in the source at the same path, like `docs/api/README.md`, stops
  the build, naming both.
- A site with
  [a nav of its own](../writing/navigation.md#writing-the-nav-yourself)
  lists the pages it wants. It can list only the crate's page, which links
  to the rest.

It finds each module as rustc does, from the `mod` that declares it:

- In `config.rs` or `config/mod.rs`, in the folder of the `lib.rs`,
  `main.rs`, or `mod.rs` that declares it. A module that `site.rs`
  declares is in `site/`, like `site/versions.rs`.
- In the file a `#[path]` names.
- Or inline, in `mod config { ... }`.

Every module it finds has a page, `pub` or not, as `cargo doc` gives a
program's. But one with `#[cfg(test)]` or `#[doc(hidden)]` has none. A file
no `mod` declares, like one in `src/bin/`, isn't read.

## What a page shows

A page looks like rustdoc's, in the site's theme. It shows:

- Its module's name, labelled `crate` or `module`. Then its docs, from its
  `//!` comments and those on its `mod`.
- Tables of the modules it declares, what it re-exports, and each kind of
  item it defines, each linked to its page or heading.
- Each item under a heading of its name, in code, like ``## `Config` ``,
  labelled with its kind, like `struct`, `trait`, or `type alias`.
- Its macros first, then its structs, enums, constants, statics, traits,
  functions, type aliases, and unions, as rustdoc orders them. Each kind is
  in order of name.
- Each item's signature, as Rust, laid out by
  [prettyplease](https://github.com/dtolnay/prettyplease) as rustfmt would:
  - A function's is without its body.
  - A struct's or union's has its `pub` fields, and `/* private fields */`
    for the rest.
  - A constant's or static's has its value, if it's a literal or a path,
    else `_`.
  - A trait's has `{ ... }` for each body it gives.
  - A macro's has the pattern of each of its rules.
- Its docs. Then a table of a struct's or union's `pub` fields, with their
  types, or of an enum's variants, each with its docs.
- The traits it implements, with an `impl` or a `#[derive]`, by name, like
  `Clone`, `Debug`, `Display`. Each is linked to its heading, if it has
  one.
- Its source, which a reader opens, under
  ``Source code in `src/config.rs` ``, with the folder as `read` names it.
- Its methods, associated functions, and constants, from each `impl` for it
  in the crate, wherever it is. A trait's are its own, with its associated
  types.
- Each of those under a heading, like ``### `Config::load` ``, and shown as
  an item is. It's labelled `method` for a function that takes `self`,
  `function` for one that doesn't, or `associated constant`.

It documents a module's items that are `pub` and have docs: its structs,
enums, unions, traits, functions, type aliases, constants, and statics.

- A `macro_rules!` with `#[macro_export]` is on the crate's page, where
  it's exported, wherever it's written.
- A `pub use` is listed by its path, as it's written, like
  `config::Config`, `config::Config as Settings`, or `crate::site::*`. It's
  linked to where it leads, with the first paragraph of its docs, rather
  than documented again.
- It leaves out an item, field, variant, or `impl` with `#[doc(hidden)]`.
  It leaves out fields, variants, and members without docs too.

## Doc comments

Doc comments are Markdown, as rustdoc reads them. That's `///` and `//!`,
`/** */` and `/*! */`, and `#[doc = "..."]`.

- A code block with no language is Rust. So is one with only rustdoc's
  words for how it's tested, like `no_run` or `should_panic`.
- In a Rust block, the lines that start with `# ` are left out, as rustdoc
  hides them. A line that starts with `##` shows with `#`.
- A heading, like `# Examples` or `# Errors`, is a bold title, like
  **Examples:**. So a page's table of contents lists only what it
  documents.
- A link to a path leads to the heading of what it names, or to its
  module's page. It's written as rustdoc's
  [intra-doc links](https://doc.rust-lang.org/rustdoc/write-documentation/linking-to-items-by-name.html)
  are: ``[`Config`]``, ``[its config](Config)``, ``[`Config::load`]``,
  ``[`crate::site`]``, ``[`super::render`]``, or ``[`Self::new`]``.
- A link may give a kind or not, like ``[`struct@Config`]``. It may have
  `()` after a function's name, or `!` after a macro's.
- A path is read in the module of what it documents, through its `use`s,
  as rustc reads it there.
- A link to a field or a variant leads to its type's heading.
- A link to a path that leads to nothing it documents, like ``[`Vec`]``,
  in the standard library, is its text, in code if that's the path.

## Warnings

What it can't read is warned of, and `little build --strict` fails on each:

- A module whose file it can't find, read, or parse. It leaves it out, with
  the modules it declares:

  ```text
  warning: plugin `rustdoc`: can't parse `src/config.rs`: expected `:` on line 12
  ```

- A `#[doc]` that isn't a string, like
  `#[doc = include_str!("../README.md")]`, which it leaves out.
- A folder in `read` with no `lib.rs` or `main.rs` in it.

## Limits, for now

- It reads only Rust, and only what's written. So an item a macro makes,
  like a `bitflags!` type, isn't documented.
- Where a module has two items, or modules, of the same name under
  `#[cfg]`, like one for each platform, it documents the first.
- An item that isn't `pub`, like one that's `pub(crate)`, isn't
  documented. That's so even in a program, which `cargo doc` documents in
  full.
- In a library, a module that isn't `pub` has a page, which rustdoc leaves
  out. A `pub use` of what's in it links there. Rust's rustdoc documents it
  again instead, in the module that re-exports it.
- A type's page doesn't list the methods a trait gives it, or those it gets
  from `Deref`.
- An `impl` for another crate's type, or for every `T`, isn't documented.
- A type in a signature isn't linked to its heading. A link to the standard
  library, or another crate, isn't linked to its docs.
- The source has no line numbers, and doesn't link to the file in the
  site's repository.
