---
title: Python API reference
description: Add a page for each module of a Python package, from its docstrings, with the docstrings plugin.
---

# Python API reference

little's repository has a plugin,
[`docstrings`](https://github.com/withlittle/little/tree/main/plugins/docstrings),
that adds an API reference to a site: a page for each module and package of
a Python package, built from its docstrings, in NumPy's, Google's, or
Sphinx's style, as [mkdocstrings](https://mkdocstrings.github.io/) builds
them. It reads the package's code with
[Ruff](https://github.com/astral-sh/ruff)'s parser, without running it, so it
needs neither Python nor the package's dependencies, and a site builds the
same each time, offline. Why it works as it does is in
[ADR 0031](https://github.com/withlittle/little/blob/main/decisions/0031-a-plugin-for-pythons-docstrings.md).

## Building it

Its `.wasm` file isn't released yet, so build it, in a clone of little's
repository:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/docstrings/Cargo.toml
```

That builds
`plugins/docstrings/target/wasm32-wasip2/release/little_docstrings.wasm`,
about 1.6 MB, which a site can name where it is, or copy into its folder.

## Using it

Name it in your site's `little.yml`, with the folder your package is in:

```yaml
# little.yml
plugins:
  docstrings:
    wasm: ./little_docstrings.wasm
    read: ./mypackage
```

- `read` names the folders it documents, from the site's folder, like
  `./mypackage`, or `[./mypackage, ./tools]`. A folder with an
  `__init__.py` is a package, named by the folder. Any other holds the
  packages and modules it documents, like `./src` for `src/mypackage/`.
- `url` is the folder its pages go in, from the source's folder:
  `/reference` unless you set it, like `url: /api`, or `url: /` for the
  source's own.

It has no other settings. One it doesn't have, or no `read`, stops the
build, saying why.

## Its pages

For `read: ./mypackage`, with `mypackage/solvers/base.py`, it adds:

- `reference/mypackage/README.md`, at `/reference/mypackage/`, for the
  package: its docstring, a table of its modules and packages, each with the
  first paragraph of its docstring, linked to its page, and what its
  `__init__.py` defines;
- `reference/mypackage/solvers/README.md`, for the package in it, the same
  way;
- `reference/mypackage/solvers/base.md`, at
  `/reference/mypackage/solvers/base`, for the module.

Each is built as if it were a Markdown file at that path in the site's
source, so it's in the nav and search, and its links are checked. A file in
the source at the same path stops the build, naming both, but
`docs/reference/README.md` is yours to write, if you like, linking to each
package's page by its path, like `[mypackage](mypackage/README.md)`. A site
with [a nav of its own](../writing/navigation.md#writing-the-nav-yourself)
lists the pages it wants.

It skips modules and folders whose names start with `_` or `.`, and folders
without an `__init__.py`, which aren't packages.

## What a page shows

A page looks like mkdocstrings' with Material for MkDocs' theme:

- its module's name, its docstring, and tables of its classes, functions,
  and attributes, each linked to its heading;
- each of those under a heading of its name, in code, like
  ``## `Solver` ``, and, for a method, ``### `Solver.solve` ``, with labels,
  like `class`, `dataclass`, `property`, and `abstractmethod`: a module's or
  class's attributes first, then its classes, then its functions, each in
  order of name;
- each class's and function's signature, as Python, wrapped at about 60
  characters, with `self` left out, and types by their names, like
  `ndarray` for `np.ndarray`. A class's is its `__init__`'s, or, for a
  dataclass, made from its fields, as Python does;
- its docstring, and, for a class, its `__init__`'s after it;
- tables of its parameters, attributes, return values, yielded values, and
  exceptions. A parameter's type is from its annotation, else its docstring,
  and its default from its signature, or *required*;
- its source, which a reader opens, under
  ``Source code in `mypackage/solvers/base.py` ``.

It documents a module's and a class's functions, classes, methods,
properties, and attributes, but not those without a docstring, or whose
names start with `_`, but `__init__`. Where a module has `__all__`, it
documents only the names it lists. It documents each object in the module
it's defined in, not where it's imported.

## Styles of docstring

It finds the style of each docstring for itself, as a package often mixes
them:

- NumPy's, when a section's title is underlined with dashes, like
  `Parameters` over `----------`;
- Sphinx's, when a line starts with a field, like `:param x:` or
  `:returns:`;
- Google's, otherwise, with sections like `Args:` over indented items.

A docstring's text is Markdown, as in mkdocstrings, and an example after
`>>>` is Python code. A section of anything but parameters, attributes,
values, and exceptions, like `Notes` or `Examples`, is kept as text, under
its title; in NumPy's style, which doesn't indent a section's text, text
that's indented is code, as in Markdown. A title reStructuredText
underlines, and perhaps overlines, like `requests.api` over `~~~~~~~~~~~~`,
is bold, as in Markdown it would be a heading, or start a block of code that
runs to the end of the page.

A docstring in NumPy's style that borrows from Google's is read as its
author meant it, where griffe and Napoleon misread it: a title may have a
colon after it, like `Parameters:`; an item may be written like
`name (type): text`; a value may be given by its type alone, like
`nx.Graph: A graph.`; and a paragraph that doesn't start with a name, like
`Returns a dict.`, ends a section of parameters or attributes, and is kept
as text.

## Warnings

A module it can't read or parse is warned of, and left out, as is a folder
in `read` with no package or module in it, and `little build --strict`
fails on each:

```text
warning: plugin `docstrings`: can't parse `mypackage/broken.py`: Expected a parameter or the end of the parameter list on line 3
```

## Limits, for now

- It reads only Python; [`doc_comments`](rust-api-reference.md) reads Rust.
- A type isn't linked to its class's heading, nor to another module's page.
- An object a package imports from a module whose name starts with `_`
  isn't documented, as the plugin documents an object where it's defined.
- A class's methods don't include those it inherits.
- reStructuredText in a docstring, like `:class:` and `.. note::`, shows as
  it's written, but for a title, which is bold.
- The source has no line numbers, and doesn't link to the file in the
  site's repository.
- Its `.wasm` file isn't released, so you build it.
