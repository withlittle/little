---
title: Python API reference
description: Add a page for each module of a Python package, from its docstrings, with the pydoc plugin.
---

# Python API reference

[`pydoc`](https://github.com/withlittle/little/tree/main/plugins/pydoc), a
plugin in little's repository, adds an API reference to a site. It adds a
page for each module and package of a Python package, from its docstrings,
in NumPy's, Google's, or Sphinx's style, as
[mkdocstrings](https://mkdocstrings.github.io/) builds them.

It's named for Python's own
[pydoc](https://docs.python.org/3/library/pydoc.html), but doesn't run it.
It reads the package's code with [Ruff](https://github.com/astral-sh/ruff)'s
parser, without running it. So it needs neither Python nor the package's
dependencies, and a site builds the same each time, offline.

## Using it

Name it in your site's `little.yml`, with the folder your package is in.
If you like, set what its page is called, and where its pages go:

```yaml
# little.yml
plugins:
  pydoc:
    wasm: https://github.com/withlittle/little/releases/download/pydoc-v0.1.0/little_pydoc.wasm
    read: ./mypackage
    title: API reference
    url: /
```

- `read` names the folders it documents, from the site's folder, like
  `./mypackage`, or `[./mypackage, ./tools]`.
- A folder in `read` with an `__init__.py` is a package, named by the
  folder. Any other holds the packages and modules it documents, like
  `./src` for `src/mypackage/`.
- `title`, if you set it, is the
  [title](../writing/frontmatter.md#a-pages-title) of the package's page,
  like `API reference`, in place of the package's name. The name is still
  its heading.
- `title` is the title of one package's or module's page. So with it,
  `read` holds only that one, like `./mypackage`, or `./src` for
  `src/mypackage/` alone. More stops the build.
- `url` is the folder its pages go in, from the source's folder. It's `/`,
  the source's own, unless you set it, like `url: /api`. Each package is in
  a folder of its name there, like `/mypackage/`.

It has no other settings. One it doesn't have, or no `read`, stops the
build, saying why.

## Getting it

Each version of `pydoc` is released in little's repository, as a `.wasm`
file, about 1.6 MB. A site names it by its URL, with the version in it, like
`pydoc-v0.1.0` above.

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
  [README](https://github.com/withlittle/little/tree/main/plugins/pydoc#building-it).

## Its pages

For the example in [Using it](#using-it), with
`mypackage/solvers/base.py`, it adds:

- `mypackage/README.md`, at `/mypackage/`, for the package, titled
  `API reference`. It has the package's docstring, and a table of its
  modules and packages, each with the first paragraph of its docstring,
  linked to its page. Then it has what its `__init__.py` defines.
- `mypackage/solvers/README.md`, for the package in it, the same way.
- `mypackage/solvers/base.md`, at `/mypackage/solvers/base`, for the
  module.

Each is built as if it were a Markdown file at that path in the site's
source. So it's in the nav and search, and its links are checked.

- A file in the source at the same path stops the build, naming both.
- But `README.md` in `url`, like `docs/api/README.md` for `url: /api`, is
  yours to write, if you like. Link to each package's page by its path,
  like `[mypackage](mypackage/README.md)`.
- A site with
  [a nav of its own](../writing/navigation.md#writing-the-nav-yourself)
  lists the pages it wants.
- It skips modules and folders whose names start with `_` or `.`. It skips
  folders without an `__init__.py` too, which aren't packages.

## What a page shows

A page looks like mkdocstrings' with Material for MkDocs' theme. It shows:

- Its module's name, its docstring, and tables of its classes, functions,
  and attributes, each linked to its heading.
- Each of those under a heading of its name, in code, like
  ``## `Solver` ``, and, for a method, ``### `Solver.solve` ``. Each has
  labels, like `class`, `dataclass`, `property`, and `abstractmethod`.
- A module's or class's attributes first, then its classes, then its
  functions, each in order of name.
- Each class's and function's signature, as Python, wrapped at about 60
  characters. `self` is left out, and types are shown by their names, like
  `ndarray` for `np.ndarray`.
- A class's signature is its `__init__`'s. For a dataclass, it's made from
  its fields, as Python does.
- Its docstring, and, for a class, its `__init__`'s after it.
- Tables of its parameters, attributes, return values, yielded values, and
  exceptions. A parameter's type is from its annotation, else its
  docstring. Its default is from its signature, or *required*.
- Its source, which a reader opens, under
  ``Source code in `mypackage/solvers/base.py` ``.

It documents a module's and a class's functions, classes, methods,
properties, and attributes.

- It leaves out those without a docstring, or whose names start with `_`,
  except `__init__`.
- Where a module has `__all__`, it documents only the names it lists.
- It documents each object in the module it's defined in, not where it's
  imported.

## Styles of docstring

It finds the style of each docstring for itself, as a package often mixes
them:

- NumPy's, when a section's title is underlined with dashes, like
  `Parameters` over `----------`.
- Sphinx's, when a line starts with a field, like `:param x:` or
  `:returns:`.
- Google's, otherwise, with sections like `Args:` over indented items.

A docstring's text is Markdown, as in mkdocstrings, and an example after
`>>>` is Python code.

- A section of anything but parameters, attributes, values, and
  exceptions, like `Notes` or `Examples`, is kept as text, under its title.
- In NumPy's style, which doesn't indent a section's text, text that's
  indented is code, as in Markdown.
- A title reStructuredText underlines, and perhaps overlines, like
  `requests.api` over `~~~~~~~~~~~~`, is bold. In Markdown, it would be a
  heading, or start a block of code that runs to the end of the page.

A docstring in NumPy's style that borrows from Google's is read as its
author meant it, where griffe and Napoleon misread it:

- A title may have a colon after it, like `Parameters:`.
- An item may be written like `name (type): text`.
- A value may be given by its type alone, like `nx.Graph: A graph.`.
- A paragraph that doesn't start with a name, like `Returns a dict.`, ends
  a section of parameters or attributes, and is kept as text.

## Warnings

A module it can't read or parse is warned of, and left out. So is a folder
in `read` with no package or module in it. `little build --strict` fails on
each:

```text
warning: plugin `pydoc`: can't parse `mypackage/broken.py`: Expected a parameter or the end of the parameter list on line 3
```

## Limits, for now

- It reads only Python. [`rustdoc`](rust-api-reference.md) reads Rust,
  [`jsdoc`](javascript-api-reference.md) reads JavaScript and TypeScript,
  and [`godoc`](go-api-reference.md) reads Go.
- A type isn't linked to its class's heading, nor to another module's page.
- An object a package imports from a module whose name starts with `_`
  isn't documented, as the plugin documents an object where it's defined.
- A class's methods don't include those it inherits.
- reStructuredText in a docstring, like `:class:` and `.. note::`, shows as
  it's written, except a title, which is bold.
- The source has no line numbers, and doesn't link to the file in the
  site's repository.
