# 0047 — Naming API reference plugins

- Status: accepted
- Date: 2026-10-15
- Amends: [0031](./0031-a-plugin-for-pythons-docstrings.md),
  [0044](./0044-a-plugin-for-rusts-doc-comments.md)

## Context
little has two plugins that add an API reference: `docstrings`
([0031](./0031-a-plugin-for-pythons-docstrings.md)), from Python's
docstrings, and `doc_comments`
([0044](./0044-a-plugin-for-rusts-doc-comments.md)), from Rust's doc
comments. Each is named for what it reads, but other languages write both:
Go, Java, Dart, Zig, and C# have doc comments, and Julia and Clojure have
docstrings. So `doc_comments` reads as if it documented any of them, though
it reads only Rust, and 0031 plans a plugin for each language, which needs
a name that says which.

Neither plugin's `.wasm` file is released, and only little's manual names
one, so a new name breaks no site but little's own.

## Options considered
- **What it reads:** `doc_comments` and `docstrings`, as now, which don't
  say which language, and which a plugin for Go, or Julia, would want too.
- **The language and what it reads:** `rust_doc_comments` and
  `python_docstrings`, which say both, but are long.
- **The language alone:** `rust` and `python`, which read as plugins that
  render ```` ```rust ```` blocks.
- **The language and `api`:** `rust_api` and `python_api`, or `api_rust`
  and `api_python`, as the manual titles its pages, "Rust API reference",
  and as mkdocstrings names its handlers by language, like `python`; but no
  language's users know these names.
- **The language's own tool:** whatever it's named, like Doxygen, for C++,
  which follows no pattern; or `<language>doc`, as many languages' tools
  are named already: rustdoc, pydoc, javadoc, JSDoc, godoc, dartdoc, and
  scaladoc. A language's users know it, and it says both the language and
  that it builds docs. But a plugin named for a tool usually runs it, as
  starlight-typedoc runs TypeDoc, and these don't. And pydoc, which comes
  with Python, isn't what most Python sites are built with: that's Sphinx
  or mkdocstrings, and the plugin's pages look like mkdocstrings'.

## Decision
- A plugin that adds an API reference from a language's code is named
  `<language>doc`, as the language's own tool is: `doc_comments` is now
  `rustdoc`, and `docstrings` is `pydoc`. One for JavaScript would be
  `jsdoc`, and one for Go, `godoc`.
- The name is the plugin's folder in `plugins/`, and ends its crate's
  name and its `.wasm` file's:

  | Plugin | Was | Folder | Crate | `.wasm` file |
  | ------ | --- | ------ | ----- | ------------ |
  | `rustdoc` | `doc_comments` | `plugins/rustdoc/` | `little-rustdoc` | `little_rustdoc.wasm` |
  | `pydoc` | `docstrings` | `plugins/pydoc/` | `little-pydoc` | `little_pydoc.wasm` |

  It's also the name the manual gives it in `plugins`, where a site can
  name it as it likes, and which little's warnings name it by:

  ```yaml
  plugins:
    rustdoc:
      wasm: ./plugins/rustdoc/target/wasm32-wasip2/release/little_rustdoc.wasm
      read: ./src
      crate: little
      url: /api
  ```
- Neither plugin runs its language's tool, so its page in the manual, and
  its README, say so in their opening lines: it's named for rustdoc, or
  pydoc, but doesn't run it. The manual writes the plugin's name as code,
  like `rustdoc`, and the tool's as a word, like rustdoc.
- "Docstrings" and "doc comments" still name what the plugins read, and
  the manual's pages keep their titles, "Python API reference" and "Rust
  API reference", which say what a reader is doing, so no URL changes.
- `plugins/api_reference/`, the library they share, keeps its name, as it
  isn't a plugin.
- 0031 and 0044 are kept as they were written, as history.

## Consequences
- A plugin's name says which language it reads, in a word that language's
  users know, and a plugin for another language has a name waiting for it.
- A reader may take `rustdoc` or `pydoc` for the tool it's named for, so
  each plugin's page says first that it isn't.
- A site that uses either plugin builds it again, from its new folder, and
  points its `wasm` at the new file; its entry in `plugins` is its own to
  name, so it can keep it, or rename it, as the manual has. As neither is
  released, that's little's manual, and few others.
- A language whose own tool isn't named `<language>doc`, like C++, whose
  is Doxygen, will be named when its plugin is written.
