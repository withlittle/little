---
title: Go docs
description: Add a page for each package of a Go module, from their doc comments, with the godoc plugin.
---

# Go docs

`godoc`, one of [little's own plugins](using-plugins.md#littles-own-plugins),
adds an API reference to your site: a
[page](using-plugins.md#pages-and-files) for each package of a Go module,
from their doc comments, as `go doc` and [pkg.go.dev](https://pkg.go.dev)
show them. It's named for godoc, Go's documentation server, but doesn't run
it, `go doc`, or Go: it reads your code with
[gosyn](https://github.com/chikaku/gosyn)'s parser, without compiling it,
so it needs neither Go nor your module's dependencies.

Name it in your site's [`plugins`](using-plugins.md#naming-a-plugin), with
the folder of your module's `go.mod`:

```yaml
# little.yml
plugins:
  godoc:
    wasm: https://github.com/withlittle/little/releases/download/godoc-v0.1.0/little_godoc.wasm
    read: .
    title: API reference
    url: /api
```

- `read` names the folder of the module's `go.mod`: `.` where the module is
  your site's repository, as it usually is, or a folder of its own, like
  `./go`. The plugin takes the module's path from its `go.mod`, like
  `example.com/shapes`, for how each package is imported, and where links
  lead.
- `title`, if you set it, is the
  [title](../writing/frontmatter.md#a-pages-title) of the module's page,
  like `API reference`, in place of its package's name.
- `url` is the folder its pages go in, from the source's folder, like
  `/api`, or `/` for the source's own. It's the last element of the
  module's path unless you set it, so `/shapes` for `example.com/shapes`,
  or for `example.com/shapes/v2`.

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
command finds a module's packages. It skips:

- Tests, whose names end in `_test.go`.
- Files and folders whose names start with `.` or `_`.
- Folders called `testdata` or `vendor`.
- Folders with a `go.mod` of their own, which are other modules.
- Files with `//go:build ignore`.

It reads the rest whatever their build constraints, so it reads a
package's files for every platform, like `canvas_linux.go` and
`canvas_windows.go`. Of what more than one declares, it documents the first
with a doc comment, in order of their names. A file it can't read or parse
is [warned of](using-plugins.md#when-a-plugin-fails), and left out, as is
one with a method with type parameters, new in Go 1.27, which gosyn doesn't
read.

Each folder with `.go` files is a package, imported by the module's path
and the folder's, like `example.com/shapes/draw`. Where a folder has files
of more than one package, like a `tools.go` with `//go:build tools` beside
another package's, it documents the one with the most files, and warns.

For the first example above, where the module, `example.com/shapes`, has a
package in its folder, one in `draw/`, and a command in `cmd/shapes/`, it
adds:

- `api/README.md`, at `/api/`, for the package in the module's folder,
  titled `API reference`. It has how it's imported, and its docs. Then it
  has a table of the packages in its folders, `cmd` and `draw`, each with
  the first paragraph of its docs, linked to its page. Then it has what it
  exports.
- `api/draw/README.md`, at `/api/draw/`, for the package in `draw/`, the
  same way.
- `api/cmd/README.md`, for `cmd/`, which has no package of its own. It's
  labelled `directory`, and has a table of the packages in it.
- `api/cmd/shapes/README.md`, for the command in `cmd/shapes/`.

A package with nothing documented, with no doc comment on it or on what it
exports, has no page, nor has a folder with no documented package in it.
The module's own page is always there. Where its folder has no package,
it's titled with the module's path, and labelled `module`, with a table of
its packages.

## What a page shows

A page looks like pkg.go.dev's, in your site's theme. It shows:

- Its package's name, labelled `package`, and how it's imported, like
  `import "example.com/shapes/draw"`.
- Its docs, from the doc comment before `package draw`, in any of its
  files.
- Tables of the packages in its folders, and of its constants, variables,
  functions, and types, each linked to its page or heading.
- Each of those under a heading of its name, in code, like
  ``## `Canvas` ``, labelled `constant`, `variable`, `function`, or `type`,
  in that order, each kind in order of name. A function that returns a
  type, like `NewCanvas`, is with the functions, and a constant of a type,
  like `Square`, with the constants, where `go doc` lists them under the
  type.
- A `deprecated` label, where a paragraph of its docs starts
  `Deprecated: `.
- Each one's signature, as it's written:
  - A function's is without its body.
  - A struct's is without the fields it doesn't export, with
    `// contains filtered or unexported fields` in their place, as `go doc`
    shows it.
  - An interface's is without such methods, with
    `// contains filtered or unexported methods` in their place.
  - A constant or variable is on its own, though it's in a group, like
    `const Square Kind = iota`.
  - A constant that repeats the line before it, as a constant can, has its
    type, like `const Circle Kind`.
- Its docs, with a table of a struct's fields that have docs, and their
  types.
- Its source, which a reader opens, under
  ``Source code in `draw/canvas.go` ``, where that shows more than its
  signature, like a function's body, or the fields a struct doesn't export.
- A type's methods, or an interface's, each under a heading, like
  ``### `Canvas.Draw` ``, labelled `method`, and shown as an item is.

It documents what a package exports, by a name that starts with a capital
letter, and has a doc comment: its constants, variables, functions, and
types, their methods, and a struct's fields.

- A type without a doc comment is shown if a method or field of it has
  one.
- Each name in a `const` or `var` declaration is an item of its own,
  documented by the comment before its line, else the one at the end of it,
  else the declaration's.

A command, `package main`, has a page with only its doc comment, as
`go doc` shows a command's, titled with the name `go install` gives it,
like `shapes`, and labelled `command`. A package in an `internal` folder is
documented too, though other modules can't import it.

## Doc comments

A doc comment is the comments right before a package clause or a
declaration. It's read as Go reads it, as
[Go Doc Comments](https://go.dev/doc/comment) describes, not as Markdown:

- A line that starts with `# `, with a blank line before and after it,
  like `# Usage`, is a heading, shown as a bold title, like **Usage:**, so
  a page's table of contents lists only what it documents. A heading in
  Go's old style, a line on its own without `#`, is a paragraph, until
  `gofmt` gives it a `#`.
- Indented lines are a code block, with no language, so it isn't
  highlighted.
- Indented lines that start with `-`, `*`, `+`, or `•`, or a number, like
  `1.` or `1)`, are a list.
- A URL, like `https://go.dev`, is a link to it, as is `[Text]`, where a
  line of the comment, like `[Text]: https://go.dev`, gives its URL.
- ``` `` ``` and `''` are curly quotes, “ and ”.
- Anything else, like `*this*` or `<b>`, shows as it's written.

A doc link is a name in brackets. It leads to the heading of what it names,
or to its package's page: `[Canvas]`, `[Canvas.Draw]`, `[*Canvas]`,
`[draw]`, `[draw.Canvas]`, `[draw.Canvas.Draw]`, or
`[example.com/shapes/draw.Canvas]`.

- A package is named by its path, or by the name its file imports it as.
  The doc comment's own package is named by its name.
- A field, or a method without docs, leads to its type's heading.
- A name its package doesn't declare, like `[Missing]`, isn't a link, and
  shows as it's written, as on pkg.go.dev.
- A link to the standard library, or another module, like `[io.Reader]`,
  is its text, in code, as is a link to something the reference doesn't
  document.
