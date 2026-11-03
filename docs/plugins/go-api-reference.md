---
title: Go API reference
description: Add a page for each package of a Go module, from their doc comments, with the godoc plugin.
---

# Go API reference

little's repository has a plugin,
[`godoc`](https://github.com/withlittle/little/tree/main/plugins/godoc),
that adds an API reference to a site: a page for each package of a Go
module, built from their doc comments, as `go doc` and
[pkg.go.dev](https://pkg.go.dev) show them. It's named for godoc, Go's
documentation server, but doesn't run it, nor `go doc`, nor Go: it reads
the module's code with [gosyn](https://github.com/chikaku/gosyn)'s parser,
without compiling it, so it needs neither Go nor the module's
dependencies, and a site builds the same each time, offline.

## Getting it

Each version of `godoc` is released in little's repository, as a `.wasm`
file, about 520 KB, which a site names by its URL, with the version in it:

```text
https://github.com/withlittle/little/releases/download/godoc-v0.1.0/little_godoc.wasm
```

little downloads it in the site's first build, and uses that copy from then
on, as for [any plugin](using-plugins.md#naming-a-plugin), so to update it,
change the URL to a later version's. Its releases, among
[little's](https://github.com/withlittle/little/releases), list what changed
in each version, and the version of little's
[hooks](../reference/plugin-hooks.md) it's built for, and each carries a
`THIRD-PARTY-NOTICES.md`, with the licences of the crates it's built from.
To build it yourself, see its
[README](https://github.com/withlittle/little/tree/main/plugins/godoc#building-it).

## Using it

Name it in your site's `little.yml`, with the folder of your module's
`go.mod`, and, if you like, what its page is called, and where its pages
go:

```yaml
# little.yml
plugins:
  godoc:
    wasm: https://github.com/withlittle/little/releases/download/godoc-v0.1.0/little_godoc.wasm
    read: .
    title: API reference
    url: /api
```

- `read` names the folder of the module's `go.mod`, from the site's
  folder: `.`, where the module is the site's repository, as it usually
  is, or a folder of its own, like `./go`. The plugin reads the module's
  path from its `go.mod`, like `example.com/shapes`, for how each package
  is imported, and where links lead.
- `title`, if you set it, is the
  [title](../writing/frontmatter.md#a-pages-title) of the module's page,
  like `API reference`, in place of its package's name, which is still its
  heading.
- `url` is the folder its pages go in, from the source's folder: the last
  element of the module's path, like `/shapes` for `example.com/shapes`, or
  for `example.com/shapes/v2`, unless you set it, like `url: /api`, or
  `url: /` for the source's own.

It has no other settings. One it doesn't have, or no `read`, stops the
build, saying why, as does a `read` with no `go.mod`, or a `go.mod` with
no `module` line.

It documents one module. For each other, like those of a Go workspace,
add another entry under `plugins`, with another name, and the same `wasm`:

```yaml
# little.yml
plugins:
  core_api:
    wasm: https://github.com/withlittle/little/releases/download/godoc-v0.1.0/little_godoc.wasm
    read: ./core
  cli_api:
    wasm: https://github.com/withlittle/little/releases/download/godoc-v0.1.0/little_godoc.wasm
    read: ./cli
```

## Its pages

It reads each `.go` file in `read`, and in the folders in it, as the go
command finds a module's packages, but tests, whose names end in
`_test.go`, files and folders whose names start with `.` or `_`, folders
called `testdata` or `vendor`, and folders with a `go.mod` of their own,
which are other modules. Each folder with `.go` files is a package,
imported by the module's path and the folder's, like
`example.com/shapes/draw`.

For the example above, where the module, `example.com/shapes`, has a
package in its folder, one in `draw/`, and a command in `cmd/shapes/`, it
adds:

- `api/README.md`, at `/api/`, for the package in the module's folder,
  titled `API reference`: how it's imported, its docs, a table of the
  packages in its folders, `cmd` and `draw`, each with the first paragraph
  of its docs, linked to its page, then what it exports;
- `api/draw/README.md`, at `/api/draw/`, for the package in `draw/`, the
  same way;
- `api/cmd/README.md`, for `cmd/`, which has no package of its own,
  labelled `directory`: a table of the packages in it;
- `api/cmd/shapes/README.md`, for the command in `cmd/shapes/`.

A package with nothing documented, no doc comment on it, or on what it
exports, has no page, and nor has a folder with none in it. The module's
own page is always there: where its folder has no package, it's titled
with the module's path, labelled `module`, with a table of its packages.

Each page is built as if it were a Markdown file at that path in the site's
source, so it's in the nav and search, and its links are checked. A file in
the source at the same path, like `docs/api/README.md`, stops the build,
naming both.

## What a page shows

A page looks like pkg.go.dev's, in the site's theme:

- its package's name, labelled `package`; how it's imported, like
  `import "example.com/shapes/draw"`; its docs, from the doc comment before
  `package draw`, in any of its files; and tables of the packages in its
  folders, and of its constants, variables, functions, and types, each
  linked to its page or heading;
- each of those under a heading of its name, in code, like
  ``## `Canvas` ``, labelled `constant`, `variable`, `function`, or `type`,
  in that order, as `go doc` lists them, each in order of name, and
  `deprecated`, where a paragraph of its docs starts `Deprecated: `;
- each one's signature, as it's written: a function's without its body;
  a struct's without the fields it doesn't export, and an interface's
  without such methods, with `// contains filtered or unexported fields`,
  or `methods`, in their place, as `go doc` shows them; and a constant or
  variable on its own, though it's in a group, like
  `const Square Kind = iota`, with its type, if it repeats the line before
  it, as a constant can, like `const Circle Kind`;
- its docs, with a table of a struct's fields that have docs, and their
  types;
- its source, which a reader opens, under
  ``Source code in `draw/canvas.go` ``, with the folder as `read` names it,
  where that shows more than its signature, like a function's body, or the
  fields a struct doesn't export;
- a type's methods, or an interface's, each under a heading, like
  ``### `Canvas.Draw` ``, labelled `method`, and shown as an item is.

It documents what a package exports, by a name that starts with a capital
letter, that has a doc comment: its constants, variables, functions, and
types, their methods, and a struct's fields. A type without one is shown
if a method or field of it has one. Each name in a `const` or `var`
declaration is an item of its own, documented by the comment before its
line, else the one at the end of it, else the declaration's.

A command, `package main`, has a page with only its doc comment, as
`go doc` shows a command's, titled with the name `go install` gives it,
like `shapes`, and labelled `command`. A package in an `internal` folder
is documented too, though other modules can't import it.

## Doc comments

A doc comment, the comments right before a package clause or a
declaration, is read as Go reads it, as
[Go Doc Comments](https://go.dev/doc/comment) describes, not as Markdown:

- A line that starts with `# `, with a blank line before and after it,
  like `# Usage`, is a heading, shown as a bold title, like **Usage:**, so
  a page's table of contents lists only what it documents.
- Indented lines are a code block, with no language, so it isn't
  highlighted.
- Indented lines that start with `-`, `*`, `+`, or `•`, or a number, like
  `1.` or `1)`, are a list.
- A URL, like `https://go.dev`, is a link to it, as is `[Text]`, where a
  line of the comment, like `[Text]: https://go.dev`, gives its URL.
- ``` `` ``` and `''` are curly quotes, “ and ”.
- Anything else, like `*this*` or `<b>`, shows as it's written.

A doc link, a name in brackets, leads to the heading of what it names, or
to its package's page: `[Canvas]`, `[Canvas.Draw]`, `[*Canvas]`, `[draw]`,
`[draw.Canvas]`, `[draw.Canvas.Draw]`, or
`[example.com/shapes/draw.Canvas]`. A package is named by its path, by
the name its file imports it as, or, for its own, by its name. A field, or
a method without docs, leads to its type's heading. A name its package
doesn't declare, like `[Missing]`, isn't a link, and shows as it's
written, as on pkg.go.dev. A link to the standard library, or another
module, like `[io.Reader]`, or to something the reference doesn't
document, is its text, in code.

## Warnings

What it can't read is warned of, and `little build --strict` fails on each:

- a file it can't read or parse, which it leaves out:

  ```text
  warning: plugin `godoc`: can't parse `draw/canvas.go`: unexpected ')' on line 12
  ```

- a folder with files of more than one package, of which it documents the
  one with the most files, as when a `tools.go`, with `//go:build tools`,
  is in another package's folder;
- a module with no `.go` files but tests, or with nothing documented.

## Limits, for now

- It doesn't read build constraints, but `//go:build ignore`: it reads a
  package's files for every platform, like `canvas_linux.go` and
  `canvas_windows.go`, and, of what more than one declares, documents the
  first with a doc comment, in order of their names.
- It doesn't show examples, from `Example` functions in tests, nor
  `BUG(who)` notes, nor what a type gets from a type it embeds: its
  methods, and a struct's fields.
- A function that returns a type, like `NewCanvas`, and a constant of a
  type, like `Square`, are with the package's functions and constants,
  where `go doc` lists them under the type.
- A heading in Go's old style, a line on its own, without `#`, is a
  paragraph; `gofmt` gives it a `#`.
- gosyn doesn't read a method with type parameters, new in Go 1.27, so a
  file with one is warned of, and left out.
- A type in a signature isn't linked to its heading, and a link to the
  standard library, or another module, isn't linked to its docs on
  pkg.go.dev.
- A field's docs are on one line, in its table, a code block's too.
- The source has no line numbers, and doesn't link to the file in the
  site's repository.
