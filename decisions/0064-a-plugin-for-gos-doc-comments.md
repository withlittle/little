# 0064 — A plugin for Go's doc comments

- Status: accepted
- Date: 2026-11-03
- Amends: [0051](./0051-semantic-versioning-and-conventional-commits.md)

## Context
[0031](./0031-a-plugin-for-pythons-docstrings.md),
[0044](./0044-a-plugin-for-rusts-doc-comments.md), and
[0048](./0048-a-plugin-for-javascripts-doc-comments.md) added plugins that
build an API reference from Python's docstrings, Rust's doc comments, and
JavaScript's, sharing a library for what a page shows, and
[0047](./0047-naming-api-reference-plugins.md) named the one for Go
`godoc`. Go's docs are doc comments, the `//` lines right before a package
clause or a declaration, with no tags, in a markup of Go's own,
[Go Doc Comments](https://go.dev/doc/comment): paragraphs, `#` headings,
indented code and lists, links, and doc links to Go's code, like
`[io.Reader]`. `go doc` shows them in a terminal, and
[pkg.go.dev](https://pkg.go.dev) on a site of its own, apart from a
project's other docs, each with Go's go/doc, which only Go runs, and each
shows every name a package exports, documented or not.

A module is a folder with a `go.mod`, which names its path, like
`example.com/shapes`. Each folder in it with `.go` files is a package,
imported by the module's path and the folder's, like
`example.com/shapes/draw`, and documented as a whole: what all its files
declare, put together, each with the comment go/parser finds before it,
else at the end of its line. A package exports what's named with a capital
letter, and `package main` is a command, a program, not a library. A
package's files can be for different builds, like `canvas_linux.go` and
`canvas_windows.go`, which the go command chooses between by their names
and their `//go:build` lines.

## Options considered
- **How it reads code:**
  - running `go doc`, or a program built on go/doc, which read Go exactly,
    but need Go, which isn't in the sandbox, so a site would run a step of
    its own before `little build`;
  - parsing it, in Rust, built for `wasm32-wasip2`: with tree-sitter's
    grammar, `tree-sitter-go`, which is C, so needs a C compiler for
    WebAssembly, and gives a tree of tokens, not of declarations; with
    goscript's parser, `go-parser`, last released in 2023; with gosyn, a
    parser in Rust alone, under MIT, kept up, which reads Go, generics
    too, into a tree like go/ast's; or with a parser of little's own.
- **What it documents:** every name a package exports, as go/doc does; or
  only what has a doc comment, or members with docs, as 0031, 0044, and
  0048 do, so a page shows what its authors wrote.
- **The module's path:** from its `go.mod`, which is in the module's
  folder, the one `read` names; or a setting, as rustdoc's `crate` and
  jsdoc's `package` are.
- **Its pages:** one for each package, as pkg.go.dev has, since a package,
  not a file, is what's imported; or one for each file, as Python's and
  JavaScript's modules are.

## Decision
- `plugins/godoc/` is the plugin, a crate, `little-godoc`, in its own Cargo
  workspace, which builds `little_godoc.wasm`:

  ```sh
  cargo build --release --target wasm32-wasip2 --manifest-path plugins/godoc/Cargo.toml
  ```

  It exports `setup`, and `files`, which adds its pages. It parses Go with
  gosyn, and doesn't run Go.
- gosyn's tree has doc comments, but not line comments, at the end of a
  line, which it takes as the doc comment of what follows, so the plugin
  steps through gosyn's tokens as go/parser does, with the semicolons Go's
  scanner inserts at the ends of lines, to find each declaration's and
  field's doc comment and line comment. gosyn's line for an error is wrong
  once it's backed up over a line and read it again, so the plugin names
  the line that the shortest start of the file with the same error ends
  on.
- A site names it, with the folder of the module's `go.mod` in `read`,
  and, in `url`, the folder its pages go in, which is `/<name>` unless it's
  set, as [0049](./0049-an-api-references-url-and-title.md) sets for
  rustdoc's and jsdoc's, where `<name>` is the last element of the
  module's path, or, for a major version, like `example.com/shapes/v2`,
  the one before it, as `go install` names a command:

  ```yaml
  plugins:
    godoc:
      wasm: ./plugins/godoc/target/wasm32-wasip2/release/little_godoc.wasm
      read: .
  ```

  It reads the module's path from the `module` line of its `go.mod`, and
  `title` titles the module's page, as 0049 sets. For another module, a
  site adds another entry, with another name, and the same `wasm`.
- It reads each `.go` file as the go command finds a module's packages,
  but tests, files and folders whose names start with `.` or `_`, folders
  called `testdata` or `vendor`, folders with a `go.mod` of their own, and
  files with `//go:build ignore`; it doesn't read other build constraints.
  Each package with something documented has a page, at its folder's
  `README.md` in `url`, so its URL follows its import path, as
  pkg.go.dev's does, and each folder above one has a page too, labelled
  `directory`, listing the packages in it, if it has none of its own. The
  module's page is always there, labelled `module`, and titled with its
  path, if its folder has no package. Of a folder with files of more than
  one package, like a `tools.go`, it documents the one with the most, and
  warns of the others.
- It documents what a package exports, with a doc comment, or with methods
  or fields with docs, as 0031, 0044, and 0048 do: its constants,
  variables, functions, and types, their methods, and a struct's fields,
  put together from all its files, as go/doc puts them; of what more than
  one file declares, for different builds, the first with a doc comment,
  by the files' names. Each name in a `const` or `var` is an item of its
  own, with its line's doc comment, else its line comment, else the
  declaration's. A command has a page with its doc comment alone, titled
  with the name `go install` gives it, and labelled `command`, as `go doc`
  shows a command's.
- The library, `plugins/api_reference/`, gets Go's kinds, `type`, for every
  kind of type, listed under Types, after Functions, as `go doc` lists
  them, each with its methods, and `directory`. pydoc's, rustdoc's, and
  jsdoc's pages are as they were.
- A page looks like theirs: its package's import line, its docs, and tables
  of the packages in its folders, and of what it exports, then each item
  under a heading of its name, like ``## `Canvas` ``, and its methods under
  their own, like ``### `Canvas.Draw` ``, each with its signature, as it's
  written, but without what it doesn't export, as `go doc` writes it, its
  docs, a table of a struct's fields with docs, and its source.
- Doc comments are read as go/doc/comment reads them, not as Markdown, and
  written as Markdown that shows what pkg.go.dev would, with their text
  escaped, and each heading as a bold title, so a page's table of
  contents lists only what it documents. A doc link leads to the heading or
  page of what it names, in the module, and a field's, or an undocumented
  method's, to its type's; one to the standard library, another module, or
  what isn't documented, is code, and one to a name its package doesn't
  declare stays as it's written, as on pkg.go.dev.
- A file it can't read or parse is warned of, and left out, as is a module
  with nothing to read, or nothing documented. No `read`, more than one
  folder in it, one with no `go.mod`, or a `go.mod` with no `module` line,
  stops the build.
- `godoc` is a scope for commits
  ([0051](./0051-semantic-versioning-and-conventional-commits.md)), and
  its release's notes list the commits that changed its folder or
  `plugins/api_reference/`, as pydoc's, rustdoc's, and jsdoc's do
  ([0052](./0052-releasing-plugins-from-tags.md)).

## Consequences
- A Go module gets an API reference in its site, in its theme, and found
  by its search, without Go, or the module's dependencies, so a site
  builds it offline, in its sandbox, the same each time.
- The plugin is about 520 KB. It adds gosyn, and gosyn's own crates, one of
  which, `unic-ucd-category`, the plugin uses too, for the Unicode
  categories Go's rules for names and doc links read.
- How go/parser finds comments, and how go/doc/comment reads them, are the
  plugin's to keep up with Go, as they change, and gosyn is another
  project's to keep up with Go's syntax: it doesn't read Go 1.27's methods
  with type parameters, so a file with one is warned of, and left out.
- little has no Go, so the manual's page "Go API reference" can't use what
  it documents, as the pages on Python's and JavaScript's can't.
- Limits, for now:
  - build constraints aren't read, but `ignore`, so a package's files for
    every build are read together;
  - it doesn't show examples, `BUG` notes, or the methods and fields a
    type gets from one it embeds;
  - functions that return a type, and constants of one, are listed with
    the package's, not under the type, as `go doc` lists them;
  - a heading in Go's old style, without `#`, is a paragraph;
  - a type in a signature isn't linked, and nor is a doc link outside the
    module, to pkg.go.dev;
  - the source has no line numbers, and doesn't link to the file in the
    site's repository.
