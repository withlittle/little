---
title: JavaScript docs
description: Add a page for a JavaScript or TypeScript package and each of its modules, from their doc comments, with the jsdoc plugin.
---

# JavaScript docs

`jsdoc`, one of [little's own plugins](using-plugins.md#littles-own-plugins),
adds an API reference to your site: a
[page](using-plugins.md#pages-and-files) for a JavaScript or TypeScript
package and each of its modules, from their doc comments, `/** ... */`. It's
named for [JSDoc](https://jsdoc.app), but doesn't run it,
[TypeDoc](https://typedoc.org), or Node: it reads your code with
[oxc](https://oxc.rs)'s parser, so it needs neither Node nor your
package's dependencies.

Name it in your site's [`plugins`](using-plugins.md#naming-a-plugin), with
the folder your package's code is in, and the package's name:

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

- `read` names the folder of the code you write, in TypeScript or
  JavaScript, like `./src` or `./lib`, rather than what a build makes of
  it.
- `package` is the package's name, as npm knows it, like `my-lib` or
  `@my-org/my-lib`. It's the heading of the package's page, and its title
  and folder too, unless you set `title` and `url`. A link to a module can
  start with it, like ``{@link module:my-lib/config}``.
- `title`, if you set it, is the
  [title](../writing/frontmatter.md#a-pages-title) of the package's page,
  like `API reference`, in place of its name.
- `url` is the folder its pages go in, from the source's folder, like
  `/api`, or `/` for the source's own. It's the package's name, like
  `/my-lib` or `/@my-org/my-lib`, unless you set it.

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

## Its pages

Each `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs`, and `.cjs` file
in `read` is a module, as is each `.d.ts`, `.d.mts`, and `.d.cts`, but for
those in `node_modules`, or whose names start with `.` or `_`. Of two with
the same name but for their extension, like `config.ts` and `config.js`, it
reads the TypeScript. A module it can't read or parse is
[warned of](using-plugins.md#when-a-plugin-fails), and left out.

For the first example above, where `src/` has `index.ts`, `config.ts`,
`site/index.ts`, and `site/versions.ts`, it adds:

- `api/README.md`, at `/api/`, for the package, titled `API reference`. It
  has a table of its modules, each with the first paragraph of its docs,
  linked to its page. Then it has what `index.ts` exports.
- `api/config.md`, at `/api/config`, for the module in `src/config.ts`.
- `api/site/README.md`, for `site/index.ts`, the same way.
- `api/site/versions.md`, for `site/versions.ts`.

A module with nothing documented has no page. A folder with no `index` has
the page of the module beside it of the same name, like `site.ts`, if there
is one, else just a table of its modules.

## What a page shows

A page looks like those of [`pydoc`](pydoc.md) and [`rustdoc`](rustdoc.md),
in your site's theme. It shows:

- Its module's name, labelled `package` or `module`.
- Its docs, from a doc comment with `@packageDocumentation`, `@module`,
  `@file`, `@fileoverview`, or `@overview`.
- Tables of the modules in its folder, what it re-exports, and each kind of
  item it exports, each linked to its page or heading.
- Each item under a heading of its name, in code, like ``## `Config` ``,
  labelled with its kind, like `class`, `interface`, `function`, or
  `type alias`.
- Its classes first, then its interfaces, enums, variables, functions, and
  type aliases, each kind in order of name.
- Each item's labels for what its tags and TypeScript's modifiers say of
  it: `default`, for its module's default export, `abstract`, `static`,
  `protected`, `readonly`, `optional`, `override`, `async`, and
  `deprecated`.
- Each item's signature, without its body, and with only the types its
  code writes, not those TypeScript would infer:
  - A class's has what it `extends` and `implements`, and its constructor.
  - A variable's has its type, if it's written, and its value, if it's a
    literal.
  - A type that `@typedef` or `@callback` defines is shown as TypeScript's
    `type`.
  - A variable that holds a function or a class, like
    `export const load = (path) => ...`, is shown as one.
- Its docs, with tables of its parameters, properties, type parameters, or
  an enum's members, and what it returns, yields, or throws, each with the
  type TypeScript's annotation gives it, else JSDoc's tag.
- Its examples, and what `@since` and `@see` say.
- Its source, which a reader opens, under
  ``Source code in `src/config.ts` ``. A class's is its constructor's, if
  it has one.
- A class's or interface's properties and methods, each under a heading,
  like ``### `Config.load` ``, labelled `property` or `method`, and shown
  as an item is. A getter and a setter of the same name are one property.

It documents what each module exports that has a doc comment, or members
with docs, where it's defined.

- What a module exports from another, like
  `export { load } from './config'`, or `export * from './config'`, is
  listed as a re-export, linked to where it leads, rather than documented
  again.
- It leaves out what's `@private`, `@internal`, `@ignore`, `@hidden`, or
  `@access private`, and TypeScript's `private` members, and `#private`
  ones.

A CommonJS module, with `require()` and `module.exports`, exports what's
set on it, as TypeScript reads it:

- `exports.load = ...` and `module.exports.load = ...` export `load`.
- `module.exports = ...` is its default export, labelled `default`, but set
  to an object, like `{ load, save }`, it exports each of its properties.
- A name that holds `module.exports` is its default export, like `app` in
  `var app = exports = module.exports = {}`, or `res` in
  `module.exports = res`, so `app.use = ...` exports `use`.

TypeScript's `export = config` is read as `module.exports = config` is. A
script, with no `import`, `export`, or `module.exports`, documents each of
its declarations with a doc comment. A `.d.ts` file documents what it
`declare`s. A function's overloads, in TypeScript, are one item, with each
overload's signature, and the docs of the first with a doc comment.

## Doc comments

A doc comment, `/** ... */`, goes before what it documents. It's Markdown,
with JSDoc's and TSDoc's tags:

- `@param`, like `@param {string} name - What it's called.` Its type is in
  braces, if it's JavaScript. Its name is in brackets if it's optional,
  like `[name]`, or has a default, like `[width=80]`, and may be a property
  of another, like `options.fill`.
- `@returns`, `@yields`, `@throws`, `@property`, `@template` or
  `@typeParam`, `@type`, `@default` or `@defaultValue`, `@deprecated`,
  `@remarks`, `@since`, and `@see`.
- `@example`, whose code is a code block in its module's language, unless
  it has its own, shown with its `<caption>`, if it has one.
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
