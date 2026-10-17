# 0048 — A plugin for JavaScript's doc comments

- Status: accepted; amended by
  [0049](./0049-an-api-references-url-and-title.md), which puts its pages
  in `/<package>`, not `/reference/<package>`, unless `url` is set, and
  titles the package's page with `title`
- Date: 2026-10-16

## Context
[0031](./0031-a-plugin-for-pythons-docstrings.md) and
[0044](./0044-a-plugin-for-rusts-doc-comments.md) added plugins that build
an API reference from Python's docstrings and Rust's doc comments, sharing
a library for what a page shows, and
[0047](./0047-naming-api-reference-plugins.md) named the next, for
JavaScript, `jsdoc`. JavaScript's and TypeScript's docs are doc comments,
`/** ... */`, before what they document, with JSDoc's tags, like `@param`
and `@returns`, or TSDoc's, which editors show as a reader types. JSDoc and
TypeDoc build them into a site of their own, in their own theme, apart from
a project's other docs, and both run on Node, which little never needs.

A package's code is JavaScript, with its types in JSDoc's tags, like
`@param {string} name`, or TypeScript, with its types in its code, or both,
with `.d.ts` files that declare the types of what `.js` files define. Its
modules are ES modules, with `import` and `export`; CommonJS, with
`require()`, `exports.name =`, and `module.exports =`, as Express's are; or
scripts, with neither.

## Options considered
- **How it reads code:**
  - running TypeDoc or JSDoc, which know every type, but need Node, and the
    package's dependencies, neither of which is in the sandbox, so a site
    would run a step of its own before `little build`;
  - parsing it, in Rust, built for `wasm32-wasip2`, with swc's parser,
    `swc_ecma_parser`, Biome's, `biome_js_parser`, or oxc's, `oxc_parser`,
    each of which reads JavaScript, TypeScript, and JSX, but sees only
    what's written: not a type that's inferred, nor what a name means in
    another package.
- **What it documents:** a package's public API, from the `exports` of its
  `package.json`, which needs each re-export followed to where it's
  defined, often in a build's output; or what each module exports, where
  it's defined, as 0031 and 0044 do, with each re-export linked to where it
  leads.
- **The package's name:** from its `package.json`, which `read: ./src`
  doesn't reach, while reading `.` would read `node_modules/`; or a
  setting, as rustdoc's `crate` is.
- **Which modules:** ES modules alone, or CommonJS too, as JSDoc and
  TypeScript read it, which many packages on npm still are.

## Decision
- `plugins/jsdoc/` is the plugin, a crate, `little-jsdoc`, in its own Cargo
  workspace, which builds `little_jsdoc.wasm`:

  ```sh
  cargo build --release --target wasm32-wasip2 --manifest-path plugins/jsdoc/Cargo.toml
  ```

  It exports `setup`, and `files`, which adds its pages. It parses code
  with oxc's parser, which tells which comments are JSDoc's, and the node
  each is before, and writes a type back as code with oxc's codegen. It
  doesn't run JSDoc, TypeDoc, or Node.
- The library, `plugins/api_reference/`, gets JavaScript's kinds of item,
  `interface`, `variable`, and `property`, beside Python's and Rust's; the
  sections that `@throws`, `@property`, and `@template` make, and an
  enum's members; and calls a class's or interface's functions its
  methods. pydoc's and rustdoc's pages are as they were.
- A site names it, with the one folder of the package's code in `read`,
  the package's name on npm, scoped or not, in `package`, and, in `url`,
  the folder its pages go in, which is `/reference/<package>` unless it's
  set:

  ```yaml
  plugins:
    jsdoc:
      wasm: ./plugins/jsdoc/target/wasm32-wasip2/release/little_jsdoc.wasm
      read: ./src
      package: my-lib
  ```

  For another package, a site adds another entry, with another name, and
  the same `wasm`.
- Each `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs`, and `.cjs`
  file in the folder is a module, and each `.d.ts`, but those in
  `node_modules`, or whose names start with `.` or `_`. Of two with the
  same name but for their extension, like `index.ts` and `index.js`, it
  reads the TypeScript. Each module with something documented has a page,
  at its path, with `index` as its folder's `README.md`, and a folder with
  no `index` has the page of the module beside it, of the same name, if
  it has one. The package's page, `README.md` in `url`, lists its modules,
  and shows the `index` in `read`.
- It documents what each module exports, where it's defined, if it has a
  doc comment, or members with docs: its classes, interfaces, enums,
  variables, functions, and type aliases, and the types its `@typedef`s
  and `@callback`s define. It lists what a module exports from another,
  linked to where it leads. In CommonJS, a module exports what
  `exports.name`, `module.exports.name`, or `name.x` is set to, where
  `name` holds `module.exports`, like `app` in
  `var app = exports = module.exports = {}`, and `module.exports` is its
  default, as TypeScript reads it. A script, with neither, documents what
  it declares, and a `.d.ts` file what it `declare`s.
- A page looks like pydoc's and rustdoc's: tables of its module's modules,
  re-exports, and items, then each item under a heading of its name, like
  ``## `Config` ``, and its members under their own, like
  ``### `Config.load` ``, each labelled with its kind, and with what
  JSDoc's tags or TypeScript's modifiers say of it, like `static`,
  `readonly`, or `deprecated`. Each has its signature, as it's written,
  without its body; its docs, and their sections, like Parameters and
  Returns, with the types TypeScript's annotations give, else JSDoc's
  tags; and its source, which a reader opens.
- Doc comments are Markdown, with JSDoc's and TSDoc's tags. A `{@link}`
  leads to the heading or the page of what it names, read in the module of
  what it documents, through its imports, else it's code. What's
  `@private`, `@internal`, `@ignore`, or `@hidden`, or TypeScript's
  `private`, or `#private`, is left out.
- A module it can't read or parse is warned of, and left out, as is a
  folder with nothing to read, or nothing documented.

## Consequences
- A JavaScript or TypeScript package gets an API reference in its site, in
  its theme, and found by its search, without Node, or the package's
  dependencies, so a site builds it offline, in its sandbox, the same each
  time.
- The plugin is about 1.2 MB. A build of Express's site, with a page for
  each of its 6 modules, takes about 1.3 seconds in all.
- little has no JavaScript but its theme's script, which isn't an API, so
  the manual's page "JavaScript API reference" can't use what it
  documents, as the page on Python's can't.
- Limits, for now:
  - it sees only what's written, so a type that's inferred, like that of
    a `const` set to a call, isn't shown, and types aren't checked;
  - it doesn't read namespaces, `import x = require()`, exports that
    destructure, a constructor function's members, set on `this` or its
    `prototype`, or a doc comment after what it documents, on its line;
  - a module that only re-exports others, as a barrel does, has a page
    listing them;
  - a module beside a folder with an `index`, like `foo.ts` and
    `foo/index.ts`, is two pages at the same path, if both are documented,
    which stops the build;
  - the codegen writes a type its own way, with double quotes, say;
  - the source has no line numbers, and doesn't link to the file in the
    site's repository;
  - the `.wasm` file isn't released, so a site builds it.
