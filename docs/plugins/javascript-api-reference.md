---
title: JavaScript API reference
description: Add a page for a JavaScript or TypeScript package and each of its modules, from their doc comments, with the jsdoc plugin.
---

# JavaScript API reference

little's repository has a plugin,
[`jsdoc`](https://github.com/withlittle/little/tree/main/plugins/jsdoc),
that adds an API reference to a site: a page for a JavaScript or TypeScript
package, and each of its modules, built from their doc comments,
`/** ... */`, as [JSDoc](https://jsdoc.app) and
[TypeDoc](https://typedoc.org) build them. It's named for JSDoc, but
doesn't run it, nor TypeDoc, nor Node: it reads the package's code with
[oxc](https://oxc.rs)'s parser, without running it, so it needs neither
Node nor the package's dependencies, and a site builds the same each time,
offline. Why it works as it does is in
[ADR 0048](../decisions/0048-a-plugin-for-javascripts-doc-comments.md),
and why it's named so, in
[ADR 0047](../decisions/0047-naming-api-reference-plugins.md).

## Getting it

Each version of `jsdoc` is released in little's repository, as a `.wasm`
file, about 1.2 MB, which a site names by its URL, with the version in it:

```text
https://github.com/withlittle/little/releases/download/jsdoc-v0.1.0/little_jsdoc.wasm
```

little downloads it in the site's first build, and uses that copy from then
on, as for [any plugin](using-plugins.md#naming-a-plugin), so to update it,
change the URL to a later version's. Its releases, among
[little's](https://github.com/withlittle/little/releases), list what changed
in each version, and the version of little's
[hooks](../reference/plugin-hooks.md) it's built for. To build it yourself,
see its
[README](https://github.com/withlittle/little/tree/main/plugins/jsdoc#building-it).

## Using it

Name it in your site's `little.yml`, with the folder your package's code is
in, and the package's name, and, if you like, what its page is called, and
where its pages go:

```yaml
# little.yml
plugins:
  jsdoc:
    wasm: https://github.com/withlittle/little/releases/download/jsdoc-v0.1.0/little_jsdoc.wasm
    read: ./src
    package: my-lib
    title: API reference
    url: /api
```

- `read` names the folder your package's code is in, from the site's
  folder, like `./src` or `./lib`: the code you write, in TypeScript or
  JavaScript, rather than what a build makes of it.
- `package` is the package's name, as npm knows it, like `my-lib` or
  `@my-org/my-lib`. It's the heading of the package's page, and its title
  and folder unless you set `title` and `url`, and a link to a module can
  start with it, like ``{@link module:my-lib/config}``. The plugin needs
  it, as the name is in `package.json`, outside the folder it reads, which
  is all it can see.
- `title`, if you set it, is the
  [title](../writing/frontmatter.md#a-pages-title) of the package's page,
  like `API reference`, in place of the package's name, which is still its
  heading.
- `url` is the folder its pages go in, from the source's folder: the
  package's name, like `/my-lib` or `/@my-org/my-lib`, unless you set it,
  like `url: /api`, or `url: /` for the source's own.

It has no other settings. One it doesn't have, or no `read` or `package`,
stops the build, saying why.

It documents one package. For each other, like those of a monorepo, add
another entry under `plugins`, with another name, and the same `wasm`:

```yaml
# little.yml
plugins:
  core_api:
    wasm: https://github.com/withlittle/little/releases/download/jsdoc-v0.1.0/little_jsdoc.wasm
    read: ./packages/core/src
    package: "@my-org/core"
  cli_api:
    wasm: https://github.com/withlittle/little/releases/download/jsdoc-v0.1.0/little_jsdoc.wasm
    read: ./packages/cli/src
    package: "@my-org/cli"
```

## Its pages

Each `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs`, and `.cjs` file
in `read`, and each `.d.ts`, is a module, but those in `node_modules`, or
whose names start with `.` or `_`. Of two with the same name but for their
extension, like `config.ts` and `config.js`, it reads the TypeScript.

For the example above, where `src/` has `index.ts`, `config.ts`,
`site/index.ts`, and `site/versions.ts`, it adds:

- `api/README.md`, at `/api/`, for the package, titled `API reference`: a
  table of its modules, each with the first paragraph of its docs, linked
  to its page, then what `index.ts` exports;
- `api/config.md`, at `/api/config`, for the module in `src/config.ts`;
- `api/site/README.md`, for `site/index.ts`, the same way, and
  `api/site/versions.md`, for `site/versions.ts`.

A module with nothing documented has no page. A folder with no `index`
has the page of the module beside it of the same name, like `site.ts`, if
there is one, else just a table of its modules.

Each page is built as if it were a Markdown file at that path in the site's
source, so it's in the nav and search, and its links are checked. A file in
the source at the same path, like `docs/api/README.md`, stops the build,
naming both.

## What a page shows

A page looks like those of the [Python](python-api-reference.md) and
[Rust](rust-api-reference.md) API references, in the site's theme:

- its module's name, labelled `package` or `module`, its docs, from a doc
  comment with `@packageDocumentation`, `@module`, `@file`,
  `@fileoverview`, or `@overview`, and tables of the modules in its folder,
  what it re-exports, and each kind of item it exports, each linked to its
  page or heading;
- each item under a heading of its name, in code, like ``## `Config` ``,
  labelled with its kind, like `class`, `interface`, `function`, or
  `type alias`: its classes, then its interfaces, enums, variables,
  functions, and type aliases, each in order of name; and with what its
  tags and TypeScript's modifiers say of it: `default`, for its module's
  default export, `abstract`, `static`, `protected`, `readonly`,
  `optional`, `override`, `async`, and `deprecated`;
- each item's signature, as it's written, without its body: a class's with
  what it `extends` and `implements`, and its constructor; a variable's
  with its type, if it's written, and its value, if it's a literal; and a
  type that `@typedef` or `@callback` defines as TypeScript's `type`. A
  variable that holds a function or a class, like
  `export const load = (path) => ...`, is shown as one;
- its docs, with tables of its parameters, properties, type parameters,
  or an enum's members, and what it returns, yields, or throws, each with
  the type TypeScript's annotation gives it, else JSDoc's tag, and its
  examples, and what `@since` and `@see` say;
- its source, which a reader opens, under
  ``Source code in `src/config.ts` ``, with the folder as `read` names it:
  a class's is its constructor's, if it has one;
- a class's or interface's properties and methods, each under a heading,
  like ``### `Config.load` ``, labelled `property` or `method`, and shown
  as an item is. A getter and a setter of the same name are one property.

It documents what each module exports that has a doc comment, or members
with docs, where it's defined. What a module exports from another, like
`export { load } from './config'`, or `export * from './config'`, is listed
as a re-export, linked to where it leads, rather than documented again. It
leaves out what's `@private`, `@internal`, `@ignore`, `@hidden`, or
`@access private`, TypeScript's `private` members, and `#private` ones.

A CommonJS module, with `require()` and `module.exports`, exports what's
set on it, as TypeScript reads it:

- `exports.load = ...` and `module.exports.load = ...` export `load`;
- `module.exports = ...` is its default export, labelled `default`, or,
  set to an object, like `{ load, save }`, exports each of its properties;
- a name that holds `module.exports`, like `app` in
  `var app = exports = module.exports = {}`, or `res` in
  `module.exports = res`, is its default export, and `app.use = ...`
  exports `use`.

TypeScript's `export = config` is read as `module.exports = config` is. A
script, with no `import`, `export`, or `module.exports`, documents each of
its declarations with a doc comment. A `.d.ts` file documents what it
`declare`s. A function's overloads, in TypeScript, are one item, with each
overload's signature, and the docs of the first with a doc comment.

## Doc comments

A doc comment, `/** ... */`, before what it documents, is Markdown, with
JSDoc's and TSDoc's tags:

- `@param`, with its type in braces, if it's JavaScript, like
  `@param {string} name - What it's called.`; in brackets, if it's
  optional, like `[name]`, or has a default, like `[width=80]`; or a
  property of another, like `options.fill`;
- `@returns`, `@yields`, `@throws`, `@property`, `@template` or
  `@typeParam`, `@type`, `@default` or `@defaultValue`, `@deprecated`,
  `@remarks`, `@since`, and `@see`;
- `@example`, whose code is a code block in its module's language, unless
  it has its own, with a `<caption>`, if it has one;
- `@typedef` and `@callback`, each a type of its own;
- `@abstract`, `@static`, `@protected`, `@readonly`, `@override`, and
  `@async`, which label what they're on.

Other tags, like `@author` or `@api`, aren't shown. A link, `{@link}`,
leads to the heading or the page of what it names: ``{@link Config}``,
``{@link Config | its config}``, ``[its config]{@link Config}``,
``{@link Config#load}``, ``{@link Config.load}``, or
``{@link module:my-lib/config}``, with `{@linkcode}` for its text in code.
A name is read in the module of what it documents: what it declares and
imports, else, if only one thing in the package has that name, that. A
link that leads to nothing it documents, like `{@link Response}`, is its
text, in code.

## Warnings

What it can't read is warned of, and `little build --strict` fails on each:

- a module it can't read or parse, which it leaves out:

  ```text
  warning: plugin `jsdoc`: can't parse `src/config.ts`: Unexpected token on line 12
  ```

- a folder in `read` with no JavaScript or TypeScript files in it;
- a folder with nothing documented, with no doc comment on a module, or on
  what one exports.

## Limits, for now

- It reads only what's written: a type TypeScript infers, like that of a
  `const` set to what a function returns, isn't shown, and types aren't
  checked.
- It doesn't read namespaces, `import x = require()`, an export that
  destructures, like `export const { a, b } = c`, or a constructor
  function's members, set on `this` or its `prototype`, as in
  `View.prototype.render = ...`.
- A doc comment after what it documents, on the same line, isn't read.
- It documents each module's exports, not the package's API, as its
  `package.json` gives it: a module that only re-exports others, as a
  barrel does, has a page that lists them.
- A module beside a folder with an `index`, like `site.ts` and
  `site/index.ts`, is two pages at the same path, if both are documented,
  which stops the build.
- A type in a signature is written as oxc writes it, with its strings in
  `"`, and isn't linked to its heading, and a link to another package
  isn't linked to its docs.
- The source has no line numbers, and doesn't link to the file in the
  site's repository.
