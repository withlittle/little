# 0063 — A plugin for reStructuredText

- Status: accepted
- Date: 2026-11-03
- Amends: [0051](./0051-semantic-versioning-and-conventional-commits.md),
  [0052](./0052-releasing-plugins-from-tags.md),
  [0062](./0062-plugins-for-asciidoc-and-orgmode.md)

## Context
[0062](./0062-plugins-for-asciidoc-and-orgmode.md) added plugins for
AsciiDoc and Org mode, which read their formats with the `formats` hook
([0060](./0060-other-formats-in-plugins.md)) into a model of a document,
which a library they share, `markup`, writes as Markdown. It left
reStructuredText for later, as no parser in Rust read enough of it.
reStructuredText is docutils' format, which Sphinx builds on, both in
Python, and the format of Python's own docs, and of many projects' built
with Sphinx. A plugin, built for `wasm32-wasip2`, can't run Python, and a
site that had to install docutils would lose what little gives it: one
binary, which builds the same site the same way, offline.

docutils is reStructuredText's specification, as well as its parser: the
syntax, and its directives and roles, like `note`, `image`, and `:math:`,
are documented, and its parser, in Python, settles the rest. Sphinx adds
directives and roles of its own, like `toctree`, `:ref:`, and `:doc:`,
which work across a project's pages, and `automodule`, which runs Python.

## Options considered
- **A parser:** `docutils-core` 0.1.0, a first release, from October 2026,
  which took over 20 seconds on docutils' own demo page, and doesn't
  resolve a section's implicit target, or read `__ URI`, `code-block`, or
  most roles; `rst_parser` 0.4, which hasn't changed in years, and reads no
  tables, and only some lists; or the plugin's own, following docutils',
  which would be little's to keep up.
- **Which reStructuredText:** docutils', with its directives and roles; or
  Sphinx's too, whose `toctree`, `:ref:`, and `:doc:` need a project's
  other pages, which a plugin given one file doesn't have, and whose
  `automodule`, and its like, need Python.
- **A directive docutils rejects,** like a `code-block` with Sphinx's
  `:linenos:`, which docutils' doesn't have: left out, and warned of; or
  kept as code, and warned of, as an unknown directive is, and as
  docutils' error shows its source.
- **Its name:** `restructuredtext`, the format's, as 0062 names its
  plugins; `rst`, its extension; or `docutils`, its tool's, which it
  doesn't run.

## Decision
- `plugins/restructuredtext/` is the plugin, a crate,
  `little-restructuredtext`, in its own workspace, which builds
  `little_restructuredtext.wasm`. It reads `.rst` files with the `formats`
  hook, into `markup`'s model, which writes it by 0062's rules, and has no
  settings. It's named for its format, as 0062's plugins are, and released
  from a tag like `restructuredtext-v0.1.0`
  ([0052](./0052-releasing-plugins-from-tags.md)).
- Its parser is its own, after docutils 0.23's, state by state, and names
  what it follows in docutils, so each part can be checked against it. Its
  tables of characters, like the punctuation inline markup may follow, and
  of URI schemes, are generated from docutils.
- It reads docutils' reStructuredText, with its directives and roles, by
  their English names, but none of Sphinx's. A directive it doesn't read,
  like `toctree`, is kept as code, and warned of, with its line; a role it
  doesn't, like `:ref:`, keeps its text, and is warned of.
- A directive docutils rejects, like a `code-block` with `:linenos:`, is
  kept as code too, and warned of, with docutils' reason, so what an
  author wrote isn't lost.
- A page is written as docutils 0.23's `rst2html5` would show it, as near
  as Markdown comes:
  - the document's title, which docutils makes of a lone section at the
    top, is the page's `#` heading, and its subsections move up a level; a
    lone subsection after it is a subtitle, an emphasised paragraph; and
    the docinfo, a field list after them, is a definition list, with RCS
    keywords cleaned, as docutils cleans them;
  - `meta`'s `description` is the page's `description`, and the `title`
    directive's its `title`, under YAML frontmatter, as 0062 sets;
  - each reference is led to its target as docutils' transforms lead it: a
    section's implicit target, or an internal, external, indirect, or
    anonymous one; a footnote, numbered, auto-numbered, or a symbol, or a
    citation, is a footnote, numbered through the page; a substitution is
    replaced; and a name that leads nowhere keeps its text, and is warned
    of;
  - a heading's `#fragment` is the one little gives it, and, with
    `raw_html`, an `<a id>` gives it docutils' id too, where that differs,
    so a link written for docutils still leads to it;
  - an admonition is the nearest of GitHub's alerts, titled with its
    directive's name where that isn't the alert's, like a `danger`, a
    caution titled Danger;
  - `code`, `code-block`, and `sourcecode` are code blocks; a literal
    block is code, and a doctest block code in `pycon`; and a
    `parsed-literal` is code, without its markup, which is warned of;
  - `math`, the directive and the role, is TeX; `raw` for `html` is raw
    HTML, and for any other format left out;
  - grid and simple tables, and those of `table`, `csv-table`, and
    `list-table`, are GitHub's, as 0062's are;
  - an `include`, or a `raw` or `csv-table` with `:file:` or `:url:`, isn't
    followed, and is warned of, and left out;
  - `contents` is left out, but for an anchor for its title, as the theme
    has its own table of contents; `sectnum`, `header`, `footer`, and
    `target-notes` are left out, and warned of, as the plugin doesn't
    number sections, or list targets' URLs, and the theme writes a page's
    header and footer;
  - comments, and the `class` directive, are left out.
- The manual has a page on it, `docs/plugins/restructuredtext.rst`, in
  reStructuredText, read by the plugin built from the same commit, as
  0062's plugins' pages are, and its tests check it's read with no
  warning.
- `restructuredtext` is a scope for commits
  ([0051](./0051-semantic-versioning-and-conventional-commits.md)), and
  its release's notes list the commits that changed its folder or
  `plugins/markup/`, as 0062's plugins' do
  ([0052](./0052-releasing-plugins-from-tags.md)).
- It needs no new crate: little has `unicode-width` already, which the
  plugin measures a title's underline and a table's columns with, counting
  a wide character as two, as docutils does.

## Consequences
- A site in reStructuredText is built by little, with nothing else to
  install, not Python, and its pages can be mixed with Markdown's,
  AsciiDoc's, and Org mode's, each linking to the others by their files.
- A Sphinx project isn't, as it is: its `toctree`s are kept as code, and
  its `:ref:`s and `:doc:`s as text, each warned of, until they're written
  as `nav`, and links to files.
- little itself doesn't grow: a site names the plugin, which is about
  570 KB.
- The parser, about 8,200 lines, a seventh of them tables generated from
  docutils, is little's own to keep up as docutils changes; as it follows
  docutils state by state, a change there is easy to find here.
- Limits, for now:
  - none of Sphinx's directives or roles is read;
  - an include isn't followed;
  - a citation is a numbered footnote, so its label is lost;
  - a table's merged cells are split, and the blocks in a cell are run
    together, as 0062's are;
  - sections aren't numbered, and `:number-lines:` doesn't number a code
    block's lines.
