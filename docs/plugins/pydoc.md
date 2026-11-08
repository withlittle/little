---
title: Python docs
description: Add a page for each module of a Python package, from its docstrings, with the pydoc plugin.
---

# Python docs

`pydoc`, one of [little's own plugins](using-plugins.md#littles-own-plugins),
adds an API reference to your site: a
[page](using-plugins.md#pages-and-files) for each module and package of a
Python package, from its docstrings, in NumPy's, Google's, or Sphinx's
style. It's named for Python's own
[pydoc](https://docs.python.org/3/library/pydoc.html), but doesn't run it,
or Python: it reads your code with [Ruff](https://github.com/astral-sh/ruff)'s
parser, so it needs neither Python nor your package's dependencies.

Name it in your site's [`plugins`](using-plugins.md#naming-a-plugin), with
the folder your package is in:

```yaml
# little.yml
plugins:
  pydoc:
    wasm: https://github.com/withlittle/little/releases/download/pydoc-v0.1.0/little_pydoc.wasm
    read: ./mypackage
    title: API reference
    url: /
```

- `read` names the folders it documents, like `./mypackage`, or
  `[./mypackage, ./tools]`. A folder with an `__init__.py` is a package,
  named by the folder. Any other holds the packages and modules it
  documents, like `./src` for `src/mypackage/`.
- `title`, if you set it, is the
  [title](../writing/frontmatter.md#a-pages-title) of the page of the one
  package or module `read` names, like `API reference`, in place of its
  name, which is still its heading.
- `url` is the folder its pages go in, from the source's folder, like
  `/api`. It's the source's own, `/`, unless you set it, and each package
  is in a folder of its name there, like `/mypackage/`.

## Its pages

For the example above, with `mypackage/solvers/base.py`, it adds:

- `mypackage/README.md`, at `/mypackage/`, for the package, titled
  `API reference`. It has the package's docstring, and a table of its
  modules and packages, each with the first paragraph of its docstring,
  linked to its page. Then it has what its `__init__.py` defines.
- `mypackage/solvers/README.md`, for the package in it, the same way.
- `mypackage/solvers/base.md`, at `/mypackage/solvers/base`, for the
  module.

It skips modules and folders whose names start with `_` or `.`, and folders
without an `__init__.py`, which aren't packages. A module it can't read or
parse is [warned of](using-plugins.md#when-a-plugin-fails), and left out.

It writes no page at `url` itself, so `README.md` there, like
`docs/api/README.md` for `url: /api`, is yours to write. Link to each
package's page from it by its path, like `[mypackage](mypackage/README.md)`.

## What a page shows

A page looks like [mkdocstrings](https://mkdocstrings.github.io/)' with
Material for MkDocs' theme. It shows:

- Its module's name, its docstring, and tables of its classes, functions,
  and attributes, each linked to its heading.
- Each of those under a heading of its name, in code, like
  ``## `Solver` ``, or ``### `Solver.solve` `` for a method, with labels,
  like `class`, `dataclass`, `property`, and `abstractmethod`.
- A module's or class's attributes first, then its classes, then its
  functions, each in order of name.
- Each class's and function's signature, as Python, without a method's
  `self` or `cls`, and with each type by its name, like `ndarray` for
  `np.ndarray`. A class's is its `__init__`'s, or, for a dataclass, made
  from its fields, as Python makes it.
- Its docstring, and, for a class, its `__init__`'s after it.
- Tables of its parameters and attributes, and what it returns, yields,
  receives, raises, and warns of. A parameter's type is from its
  annotation, else its docstring, and its default is from its signature,
  else it's *required*.
- Its source, which a reader opens, under
  ``Source code in `mypackage/solvers/base.py` ``.

It documents a module's and a class's functions, classes, methods,
properties, and attributes that have a docstring, or members that do,
unless their names start with `_`.

- Where a module has `__all__`, it documents only the names it lists.
- It documents each object in the module it's defined in, not where it's
  imported, so one a package imports from a module whose name starts with
  `_` isn't documented.
- A class's methods are its own, not those it inherits.

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

- A section other than those its tables show, like `Notes` or `Examples`,
  is kept as text, under its title.
- In NumPy's style, which doesn't indent a section's text, text that's
  indented is code, as in Markdown.
- reStructuredText, like `:class:` or `.. note::`, shows as it's written,
  but for a title it underlines, and perhaps overlines, like `requests.api`
  over `~~~~~~~~~~~~`, which is bold.

A docstring in NumPy's style that borrows from Google's is read as its
author meant it:

- A title may have a colon after it, like `Parameters:`.
- An item may be written like `name (type): text`.
- A value may be given by its type alone, like `nx.Graph: A graph.`.
- A paragraph that doesn't start with a name, like `Returns a dict.`, ends
  a section of parameters or attributes, and is kept as text.
