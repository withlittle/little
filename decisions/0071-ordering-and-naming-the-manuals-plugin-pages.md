# 0071 — Ordering and naming the manual's plugin pages

- Status: accepted
- Date: 2026-11-07
- Amends: [0047](./0047-naming-api-reference-plugins.md),
  [0060](./0060-other-formats-in-plugins.md)

## Context
The manual's Plugins section lists its pages in this order:

1. "Using plugins".
2. "Jupyter notebooks", "AsciiDoc", "Org mode", and "reStructuredText",
   on the plugins that read a format.
3. "Python API reference", "Rust API reference", "JavaScript API
   reference", "Go API reference", and "HTTP API reference", on the
   plugins that add an API reference.
4. "Writing a plugin".

Each page's file is named for its title, like `python-api-reference.md`
and `jupyter-notebooks.ipynb`. So is its URL, like
`/plugins/python-api-reference`.

[0047](./0047-naming-api-reference-plugins.md) named two of the plugins
for their languages' own tools, `pydoc` and `rustdoc`. But it kept their
pages' titles, so no URL changed. `jsdoc` and `godoc` followed, with pages
titled the same way. [0069](./0069-a-plugin-for-openapi.md) named
`openapi` for its format, as the plugins that read one are named. But its
page is titled for what it builds, an HTTP API's reference.

"Writing a plugin" is about any plugin, as "Using plugins" is. But it
comes last, after ten pages on particular plugins.

## Options considered
- **Keep the titles and files:** as 0047 has it, so no URL changes.
- **New titles, but the old files:** so no URL changes either. But each
  file is named for a title its page no longer has, and authors link to a
  page by its file.
- **Files named for their titles:** like `python-docs.md`, as the manual's
  other pages are.
- **Files named for their plugins:** like `pydoc.md`, as the plugin's
  folder, crate, and `.wasm` file are.

## Decision
The Plugins section lists its pages in this order, in `docs/plugins/`:

| Title | File | Was |
| ----- | ---- | --- |
| Using plugins | `using-plugins.md` | |
| Writing plugins | `writing-plugins.md` | "Writing a plugin", `writing-a-plugin.md` |
| Jupyter notebooks | `jupyter.ipynb` | `jupyter-notebooks.ipynb` |
| AsciiDoc | `asciidoc.adoc` | |
| Org mode | `org-mode.org` | |
| reStructuredText | `restructuredtext.rst` | |
| OpenAPI | `openapi.md` | "HTTP API reference", `http-api-reference.md` |
| Python docs | `pydoc.md` | "Python API reference", `python-api-reference.md` |
| Rust docs | `rustdoc.md` | "Rust API reference", `rust-api-reference.md` |
| Go docs | `godoc.md` | "Go API reference", `go-api-reference.md` |
| JavaScript docs | `jsdoc.md` | "JavaScript API reference", `javascript-api-reference.md` |

An empty "Was" means the page keeps its title and file.

- "Using plugins" and "Writing plugins", on any plugin, come first. Then
  come the pages on the plugins that read a format, then "OpenAPI", then
  the pages on the plugins that add an API reference from code.
- A page's title is the same in `nav`, its frontmatter's `title`, and its
  `#` heading, as on the manual's other pages.
- A page on a plugin is named for it, like `pydoc.md`, at `/plugins/pydoc`.
  But the page on `orgmode` keeps its file, `org-mode.org`. "Writing
  plugins" is named for its title, as "Using plugins" is.
- "OpenAPI" is titled for its format, as its plugin is named, and as the
  pages on the other formats are titled. A page on an API reference from
  code is titled "<language> docs".
- Each page keeps its format. The pages on notebooks, AsciiDoc, Org mode,
  and reStructuredText are still written in them.
- Every link to a page leads to its new file: in the manual, the READMEs,
  `RELEASING.md`, and `release-plugin.yml`. A plugin's README links to its
  page by its new title.
- 0047 and 0060 are kept as they were written, as history.

## Consequences
- A reader meets how to write a plugin second, not last. "JavaScript docs"
  is the section's last page, followed by Reference's first, "Commands".
- A page's file and URL name the plugin it's on, as the plugin's folder
  and tags do, but for `orgmode`'s page, at `/plugins/org-mode`.
- Once the manual is published again, the old URLs, like
  `/plugins/python-api-reference`, lead nowhere, as little has no
  redirects. So a link to one from another site breaks.
- A plugin's release notes link to its page in the manual's source at its
  tag. So the notes of earlier releases still lead to their pages.
  `release-plugin.yml` and `RELEASING.md` name the new files for the next.
- The pages in AsciiDoc, Org mode, and reStructuredText keep their files,
  which their plugins' tests read.
