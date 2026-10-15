# 0031 — A plugin for Python's docstrings

- Status: accepted; amended by [0046](./0046-plugin-hooks.md), whose
  hooks it exports in place of `plugin-with-pages`: `setup`, and
  `files`, which adds its pages
- Date: 2026-10-08

## Context
An API reference, built from the docstrings in a package's code, is the
first plugin [VISION.md](../VISION.md) plans. Python's docstrings are
where Sphinx's autodoc and mkdocstrings build one from, written in NumPy's,
Google's, or Sphinx's style, and little reads none of them. Now that a
plugin can add pages ([0030](./0030-plugins-that-add-pages.md)), one can
build a page for each module of a package, named in `little.yml` alone.

Other languages write docs in their code too, as Javadoc, JSDoc, TSDoc,
and PHP's docblocks, and will want the same.

## Options considered
- **Languages:** one plugin for every language, or one for each. Each
  language has its own parser, its own styles of docs, and its own
  modules, so a plugin for all of them would be each of them, in one
  large `.wasm` file. One for each, sharing a library for what a page
  shows and how it's written, as Markdown, is as small as its language,
  and a site with Python and TypeScript names two, each reading its own
  folders.
- **Where it lives:** in its own repository, or in little's, beside the
  interface it exports, as Zed keeps its own extensions in its
  repository's `extensions/`. In little's, a change to the interface is
  tested with it in the same pull request, and it can move to its own
  repository later, with its history.
- **How it reads a package:**
  - importing it, as Sphinx's autodoc does, which needs Python, and the
    package's dependencies, in the plugin, so the plugin can't run in
    little's sandbox, with no network;
  - parsing it, as mkdocstrings' griffe does by default, in Rust, with:
    - Ruff's parser, `ruff_python_parser`, which reads every Python
      Ruff does, is published every week, and builds for `wasm32-wasip2`;
    - RustPython's, last released in 2024;
    - tree-sitter's, in C, which needs WASI's C library to build.
- **What it documents:** a setting of its own, or the folders it reads,
  which a site names in `read` already, as a plugin reads nothing else.
- **Styles of docstring:** a setting, or one found for each docstring, as
  griffe's `auto` does, since a package often mixes them.
- **How its pages look:** like mkdocstrings' with Material for MkDocs'
  theme, which Python's users know.

## Decision
- `plugins/docstrings/` is the plugin, a crate, `little-docstrings`, in its
  own Cargo workspace, which builds `little_docstrings.wasm`:

  ```sh
  cargo build --release --target wasm32-wasip2 --manifest-path plugins/docstrings/Cargo.toml
  ```

  It exports `plugin-with-pages`, renders no blocks, and parses Python
  with Ruff's parser.
- A site names it, the folders it documents in `read`, and, in `url`, the
  folder its pages go in, which is `/reference` unless it's set:

  ```yaml
  plugins:
    docstrings:
      wasm: ./plugins/docstrings/target/wasm32-wasip2/release/little_docstrings.wasm
      read: ./mypackage
      url: /reference
  ```
- A folder it reads that holds an `__init__.py` is a package, named by the
  folder; any other holds the packages and modules it documents. Folders
  in a package without an `__init__.py` aren't packages, so it skips them.
- It documents:
  - a module's or a class's functions, classes, methods, properties, and
    attributes, but those with no docstring, and those whose names start
    with `_`, but `__init__`;
  - where a module has `__all__`, only the names it lists;
  - each object in the module it's defined in, not where it's imported.
- Its pages:
  - `reference/mypackage/README.md`, for a package: its docstring, a table
    of its modules and packages, linked to their pages, and what its
    `__init__.py` defines;
  - `reference/mypackage/module.md`, for a module, and so on for the
    packages in a package.

  It leaves `reference/README.md` for the site to write.
- A page looks like mkdocstrings':
  - its module's name, its docstring, and tables of its classes,
    functions, and attributes, each linked to its heading;
  - each object under a heading of its name, in code, with its attributes
    first, then its classes, then its functions, each in order of name,
    like ``## `Solver` `` and, for its methods, ``### `Solver.solve` ``,
    with labels, like `dataclass` and `abstractmethod`;
  - each class's and function's signature, as Python, wrapped at about 60
    characters, with `self` left out, and types by their names, like
    `ndarray` for `np.ndarray`. A class's is its `__init__`'s, or, for a
    dataclass, made from its fields, as Python does;
  - its docstring, and a class's `__init__`'s after it;
  - tables of its parameters, attributes, return values, yielded values,
    and exceptions, with a parameter's type from its annotation, else its
    docstring, and its default value from the signature, or *required*;
  - its source, which a reader opens, under ``Source code in `mypackage/module.py` ``.
- Docstrings are NumPy's, Google's, or Sphinx's style, found for each
  docstring. Their text is Markdown, as in mkdocstrings, and examples
  after `>>>` are Python code. A section the plugin doesn't know, like
  `Notes` in a style without it, is kept, under its title; in NumPy's
  style, its text that's indented is code, as griffe reads it. A title
  reStructuredText underlines, and perhaps overlines, like `requests.api`
  over `~~~~~~~~~~~~`, is bold, as in Markdown it would be a heading, or
  start a block of code that runs to the end of the page.
- Docstrings in NumPy's style that borrow from Google's are read as their
  authors meant them, where griffe and Napoleon misread them: a title may
  have a colon after it, like `Parameters:`; an item may be written like
  `name (type): text`; a value may be given by its type alone, like
  `nx.Graph: A graph.`; and a paragraph that doesn't start with a name,
  like `Returns a dict.`, ends a section of parameters or attributes, and
  is kept as text.
- A module Ruff can't parse is warned of, and left out.

## Consequences
- A Python package gets an API reference without Python, its
  dependencies, or a page in the docs for each module, so a site builds it
  offline, in its sandbox, the same each time.
- The plugin is about 1.6 MB, and adds about a fifth of a second to a
  build of a package of 138 modules.
- A plugin for another language, like TypeScript's, starts with this
  one's model and Markdown, which move to a library they share.
- Limits, for now:
  - a type isn't linked to its class's heading, nor to another module's
    page;
  - an object a package imports from a module whose name starts with `_`
    isn't documented, as the plugin documents an object where it's
    defined;
  - a class's methods don't include those it inherits;
  - reStructuredText in a docstring, like `:class:` and `.. note::`, shows
    as it's written, but for a title, which is bold;
  - the source has no line numbers, and doesn't link to the file in the
    site's repository;
  - the `.wasm` file isn't released, so a site builds it, or downloads it
    from where it's published.
