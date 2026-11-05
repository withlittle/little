---
title: JavaScript API reference
description: Add a page for a JavaScript or TypeScript package and each of its modules, from their doc comments, with the jsdoc plugin.
---

# JavaScript API reference

[`jsdoc`](https://github.com/withlittle/little/tree/main/plugins/jsdoc), a
plugin in little's repository, adds an API reference to a site. It adds a
page for a JavaScript or TypeScript package, and each of its modules, from
their doc comments, `/** ... */`, as [JSDoc](https://jsdoc.app) and
[TypeDoc](https://typedoc.org) build them.

It's named for JSDoc, but doesn't run it, nor TypeDoc, nor Node. It reads
the package's code with [oxc](https://oxc.rs)'s parser, without running it.
So it needs neither Node nor the package's dependencies, and a site builds
the same each time, offline.

## Using it

Name it in your site's `little.yml`, with the folder your package's code is
in, and the package's name. If you like, set what its page is called, and
where its pages go:

```yaml
# little.yml
plugins:
  jsdoc:
    wasm: https://github.com/withlittle/little/releases/download/jsdoc-v0.1.1/little_jsdoc.wasm
    read: ./src
    package: my-lib
    title: API reference
    url: /api
```

- `read` names the folder your package's code is in, from the site's
  folder, like `./src` or `./lib`. That's the code you write, in
  TypeScript or JavaScript, rather than what a build makes of it.
- `package` is the package's name, as npm knows it, like `my-lib` or
  `@my-org/my-lib`. It's the heading of the package's page. It's its title
  and folder too, unless you set `title` and `url`.
- A link to a module can start with the package's name, like
  ``{@link module:my-lib/config}``.
- The plugin needs `package`, as the name is in `package.json`. That's
  outside the folder it reads, which is all it can see.
- `title`, if you set it, is the
  [title](../writing/frontmatter.md#a-pages-title) of the package's page,
  like `API reference`, in place of the package's name. The name is still
  its heading.
- `url` is the folder its pages go in, from the source's folder. It's the
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
    wasm: https://github.com/withlittle/little/releases/download/jsdoc-v0.1.1/little_jsdoc.wasm
    read: ./packages/core/src
    package: "@my-org/core"
  cli_api:
    wasm: https://github.com/withlittle/little/releases/download/jsdoc-v0.1.1/little_jsdoc.wasm
    read: ./packages/cli/src
    package: "@my-org/cli"
```

## Getting it

Each version of `jsdoc` is released in little's repository, as a `.wasm`
file, about 1.2 MB. A site names it by its URL, with the version in it, like
`jsdoc-v0.1.1` above.

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
  [README](https://github.com/withlittle/little/tree/main/plugins/jsdoc#building-it).

## Its pages

Each `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs`, and `.cjs` file
in `read` is a module, and so is each `.d.ts`. But it skips those in
`node_modules`, and those whose names start with `.` or `_`. Of two with
the same name but for their extension, like `config.ts` and `config.js`, it
reads the TypeScript.

Take the first example in [Using it](#using-it). There, `src/` has
`index.ts`, `config.ts`, `site/index.ts`, and `site/versions.ts`. So it
adds:

- `api/README.md`, at `/api/`, for the package, titled `API reference`. It
  has a table of its modules, each with the first paragraph of its docs,
  linked to its page. Then it has what `index.ts` exports.
- `api/config.md`, at `/api/config`, for the module in `src/config.ts`.
- `api/site/README.md`, for `site/index.ts`, the same way.
- `api/site/versions.md`, for `site/versions.ts`.

A module with nothing documented has no page. A folder with no `index` has
the page of the module beside it of the same name, like `site.ts`, if there
is one. Else it has just a table of its modules.

Each page is built as if it were a Markdown file at that path in the site's
source. So it's in the nav and search, and its links are checked. A file in
the source at the same path, like `docs/api/README.md`, stops the build,
naming both.

## What a page shows

A page looks like those of the [Python](python-api-reference.md) and
[Rust](rust-api-reference.md) API references, in the site's theme. It
shows:

- Its module's name, labelled `package` or `module`.
- Its docs, from a doc comment with `@packageDocumentation`, `@module`,
  `@file`, `@fileoverview`, or `@overview`.
- Tables of the modules in its folder, what it re-exports, and each kind of
  item it exports, each linked to its page or heading.
- Each item under a heading of its name, in code, like ``## `Config` ``,
  labelled with its kind, like `class`, `interface`, `function`, or
  `type alias`.
- Its classes first, then its interfaces, enums, variables, functions, and
  type aliases. Each kind is in order of name.
- Each item's labels for what its tags and TypeScript's modifiers say of
  it: `default`, for its module's default export, `abstract`, `static`,
  `protected`, `readonly`, `optional`, `override`, `async`, and
  `deprecated`.
- Each item's signature, as it's written, without its body:
  - A class's has what it `extends` and `implements`, and its constructor.
  - A variable's has its type, if it's written, and its value, if it's a
    literal.
  - A type that `@typedef` or `@callback` defines is shown as TypeScript's
    `type`.
  - A variable that holds a function or a class, like
    `export const load = (path) => ...`, is shown as one.
- Its docs, with tables of its parameters, properties, type parameters, or
  an enum's members, and what it returns, yields, or throws. Each has the
  type TypeScript's annotation gives it, else JSDoc's tag.
- Its examples, and what `@since` and `@see` say.
- Its source, which a reader opens, under
  ``Source code in `src/config.ts` ``, with the folder as `read` names it.
  A class's is its constructor's, if it has one.
- A class's or interface's properties and methods, each under a heading,
  like ``### `Config.load` ``. Each is labelled `property` or `method`, and
  shown as an item is. A getter and a setter of the same name are one
  property.

It documents what each module exports that has a doc comment, or members
with docs, where it's defined.

- What a module exports from another, like
  `export { load } from './config'`, or `export * from './config'`, is
  listed as a re-export. It's linked to where it leads, rather than
  documented again.
- It leaves out what's `@private`, `@internal`, `@ignore`, `@hidden`, or
  `@access private`. It leaves out TypeScript's `private` members, and
  `#private` ones, too.

A CommonJS module, with `require()` and `module.exports`, exports what's
set on it, as TypeScript reads it:

- `exports.load = ...` and `module.exports.load = ...` export `load`.
- `module.exports = ...` is its default export, labelled `default`. But set
  to an object, like `{ load, save }`, it exports each of its properties.
- A name that holds `module.exports` is its default export, like `app` in
  `var app = exports = module.exports = {}`, or `res` in
  `module.exports = res`. Then `app.use = ...` exports `use`.

TypeScript's `export = config` is read as `module.exports = config` is. A
script, with no `import`, `export`, or `module.exports`, documents each of
its declarations with a doc comment. A `.d.ts` file documents what it
`declare`s.

A function's overloads, in TypeScript, are one item, with each overload's
signature. It has the docs of the first with a doc comment.

## Doc comments

A doc comment, `/** ... */`, goes before what it documents. It's Markdown,
with JSDoc's and TSDoc's tags:

- `@param`, like `@param {string} name - What it's called.` Its type is in
  braces, if it's JavaScript. Its name is in brackets if it's optional,
  like `[name]`, or has a default, like `[width=80]`. Or it's a property of
  another, like `options.fill`.
- `@returns`, `@yields`, `@throws`, `@property`, `@template` or
  `@typeParam`, `@type`, `@default` or `@defaultValue`, `@deprecated`,
  `@remarks`, `@since`, and `@see`.
- `@example`. Its code is a code block in its module's language, unless it
  has its own. It's shown with its `<caption>`, if it has one.
- `@typedef` and `@callback`, each a type of its own.
- `@abstract`, `@static`, `@protected`, `@readonly`, `@override`, and
  `@async`, which label what they're on.

Other tags, like `@author` or `@api`, aren't shown.

A link, `{@link}`, leads to the heading or the page of what it names. It's
written ``{@link Config}``, ``{@link Config | its config}``,
``[its config]{@link Config}``, ``{@link Config#load}``,
``{@link Config.load}``, or ``{@link module:my-lib/config}``. Write
`{@linkcode}` for its text in code.

- A name is read in the module of what it documents, from what it declares
  and imports. Else, if only one thing in the package has that name, it's
  that.
- A link that leads to nothing it documents, like `{@link Response}`, is
  its text, in code.

## Warnings

What it can't read is warned of, and `little build --strict` fails on each:

- A module it can't read or parse, which it leaves out:

  ```text
  warning: plugin `jsdoc`: can't parse `src/config.ts`: Unexpected token on line 12
  ```

- A folder in `read` with no JavaScript or TypeScript files in it.
- A folder with nothing documented, with no doc comment on a module, or on
  what one exports.

## Limits, for now

- It reads only what's written. So a type TypeScript infers, like that of
  a `const` set to what a function returns, isn't shown. Types aren't
  checked.
- It doesn't read namespaces, or `import x = require()`.
- It doesn't read an export that destructures, like
  `export const { a, b } = c`.
- It doesn't read a constructor function's members, set on `this` or its
  `prototype`, as in `View.prototype.render = ...`.
- A doc comment after what it documents, on the same line, isn't read.
- It documents each module's exports, not the package's API, as its
  `package.json` gives it. So a module that only re-exports others, as a
  barrel does, has a page that lists them.
- A module beside a folder with an `index`, like `site.ts` and
  `site/index.ts`, is two pages at the same path, if both are documented.
  That stops the build.
- A type in a signature is written as oxc writes it, with its strings in
  `"`. It isn't linked to its heading.
- A link to another package isn't linked to its docs.
- The source has no line numbers, and doesn't link to the file in the
  site's repository.
