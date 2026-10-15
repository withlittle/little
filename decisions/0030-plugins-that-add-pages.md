# 0030 — Plugins that add pages

- Status: accepted; amended by [0046](./0046-plugin-hooks.md), which adds
  pages, and other files, with the `files` hook, in place of `pages`, and
  warnings with `warn`, so a plugin's page can be a notebook
- Date: 2026-10-08
- Amends: [0013](./0013-pages-that-clash.md),
  [0029](./0029-plugins-that-render-code-blocks.md)

## Context
A plugin renders a fenced code block ([0029](./0029-plugins-that-render-code-blocks.md)),
so an API reference could be a block on each page, naming its module, as
mkdocstrings' `::: mypackage.module` is. But a package of 100 modules
would need 100 pages, each written by hand, and kept in step with the code
as modules come and go. Each would be a file in the docs that shows
nothing on GitHub but its block, which only little understands.

mkdocstrings' users make those pages with a Python script, which
mkdocs-gen-files runs as each build starts, adding its pages to the build
without writing them to the source. Sphinx's autosummary writes them into
the source.

## Options considered
- **How the pages get made:**
  - an author writes them, a block on each;
  - a plugin writes them into the source before each build, as
    autosummary does: files the author didn't write, to keep out of Git,
    which a plugin, which can't change a file
    ([0029](./0029-plugins-that-render-code-blocks.md)), couldn't write;
  - a plugin adds them to the build, as mkdocs-gen-files does, without
    writing them to the source, configured in `little.yml` alone.
- **Where a page goes:** a URL the plugin chooses; or a path, as if it
  were a file in the source, so its URL, its place in the nav, and where
  its links lead from are those of a file at that path, which authors
  already know.
- **What a page is:** HTML, which the theme puts in its template; or
  Markdown, built as a file's is, so its headings, links, math, diagrams,
  and code are little's own, as for a plugin's block.
- **How a plugin says it adds pages:** a new version of the interface,
  which every plugin would have to be built again for; or a second
  interface, in the same package, which only a plugin that adds pages
  exports, so plugins already built keep working.
- **Something a plugin can't add,** like a module it can't read: stop the
  build; or warn, as of a block it can't render, so `little serve` keeps
  going, and `--strict` fails.

## Decision
- `wit/plugin.wit` has a second interface, `pages`, and a second world,
  `plugin-with-pages`, which exports it beside `code-blocks`:

  ```wit
  interface pages {
      record page { path: string, markdown: string }
      record added { pages: list<page>, warnings: list<string> }
      pages: func() -> result<added, string>;
  }

  world plugin-with-pages {
      include plugin;
      export pages;
  }
  ```
- little calls `pages` on each plugin that exports it, once each build,
  after `setup`, before any page is built. An error, or a crash, stops
  the build. A plugin that renders no blocks gives back no languages from
  `setup`.
- Each page is built as if it were a Markdown file at its `path` in the
  source, like `reference/mypackage/README.md`: at that file's URL,
  `/reference/mypackage/`, in the nav, with breadcrumbs, previous and next
  pages, and in search. Its frontmatter is read, its relative links lead
  from its folder, its links are checked, and its code blocks go to
  plugins, which are told it's the file `docs/reference/mypackage/README.md`.
- A path is relative, with `/` between its parts, none of them empty, `.`,
  or `..`, and ends in `.md`; any other stops the build.
- A page that would be the same page as a file in the source, or as
  another plugin's page, stops the build, naming both, as
  [0013](./0013-pages-that-clash.md) has two files do:
  ``./docs/reference/README.md and plugin `docstrings`'s reference/README.md would both be the page at /reference/: rename or move one``.
- A plugin's page has no file in the site's repository, so no
  `page.source_url` or `page.edit_url`.
- A warning about a plugin's page, like a broken link in it, names it as
  ``plugin `docstrings`'s reference/mypackage/README.md``.
- Each of a plugin's warnings is warned of, like
  ``warning: plugin `docstrings`: can't read src/app.py: …``, and
  `little build --strict` fails on one.
- `little serve` calls `pages` again for each build, which a change in a
  folder a plugin reads starts.

## Consequences
- An API reference, or a page for each file in a folder of data, can be
  a plugin, configured in `little.yml`, without a file in the docs for
  each page, so the docs read the same on GitHub.
- An author writes the page above a plugin's, like `reference/README.md`,
  and links to its pages by their paths, like
  `[base](reference/mypackage/base.md)`, as to any page's file
  ([0017](./0017-links-to-pages-files.md)).
- Plugins already built keep working, as do sites that use them.
- Limits, for now:
  - each build asks a plugin for all its pages, even when `little serve`
    rebuilds for a change to one;
  - a plugin's page can't be a notebook;
  - a plugin's page can't name the file it's built from, for
    `page.source_url` and `page.edit_url`.
