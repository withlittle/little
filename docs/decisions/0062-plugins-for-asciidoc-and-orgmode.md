# 0062 — Plugins for AsciiDoc and Org mode

- Status: accepted; amended by
  [0063](./0063-a-plugin-for-restructuredtext.md), which adds
  `restructuredtext`, a plugin for reStructuredText, which this record
  left for later, that shares `markup` too
- Date: 2026-10-24
- Amends: [0051](./0051-semantic-versioning-and-conventional-commits.md),
  [0052](./0052-releasing-plugins-from-tags.md)

## Context
[0060](./0060-other-formats-in-plugins.md) gave plugins the `formats`
hook, so little reads Markdown alone, and a plugin reads any other format
into the parts of its page, Markdown, HTML, and code. It moved notebooks
into `jupyter`, and left AsciiDoc, reStructuredText, and Org mode to
plugins, once sites needed them. This record is about AsciiDoc and Org
mode; reStructuredText, which no parser in Rust reads well enough yet, is
left for now. Each has sites written in it: AsciiDoc's, with Asciidoctor,
in Ruby, and Org mode's, with Emacs. A plugin, built for `wasm32-wasip2`,
can't run either, and a site that had to install one would lose what
little gives it: one binary, which builds the same site the same way,
offline.

So each plugin needs a parser in Rust, and each turns what it reads into
Markdown, which they'd each do alike: text escaped, so it shows as it's
written; only the extensions to Markdown the site turns on; what Markdown
can't hold written as HTML, where `raw_html` is on; and a link to a heading
led to the `#fragment` little gives it.

## Options considered
- **A parser for AsciiDoc:** `asciidoc-parser`, MIT or Apache-2.0, and
  developed actively, which has read the whole language, STEM too, since
  0.25, and read Asciidoctor's README and a page of Antora's cleanly;
  `asciidork-parser`, which doesn't read STEM; or `acdc-parser`, which
  brings chrono, peg, and ureq, an HTTP client a plugin can't use.
- **A parser for Org mode:** `orgize` 0.10, MIT, built on rowan, which
  read the 23,780 lines of Org's own manual in half a second, but is an
  alpha, with no release since June 2024, and doesn't read `$…$` math or
  a bare URL; `orgize` 0.9, its last release, which panics on a
  footnote's definition in a debug build, and reads much less; `organic`,
  which needs Cargo's nightly; or `windancer`, under the GPL.
- **Writing Markdown:** each plugin writes its own, which would be a way
  for each format of escaping text, of falling back where an extension is
  off, and of working out a heading's `#fragment`; or a library they share,
  as the plugins that add an API reference share `api_reference`
  ([0044](./0044-a-plugin-for-rusts-doc-comments.md)), with a model of a
  document that each reads its format into, and that it writes.
- **Raw HTML:** in a `markdown` part, which a blank line in it ends; or an
  `html` part, written whole ([0060](./0060-other-formats-in-plugins.md)).
- **Names:** `<format>doc`, as [0047](./0047-naming-api-reference-plugins.md)
  names plugins that add an API reference, which these don't; the format's
  tool's, like `asciidoctor`, which they don't run; an extension, like
  `adoc` or `org`, of which a format can have more than one; or the
  format's own name, as `jupyter` is named for what it reads.
- **The manual's pages on them:** in Markdown; or each in its format, read
  by its plugin built from the same commit, as the page on notebooks is a
  notebook, so each uses what it documents.

## Decision
- Two plugins, each in `plugins/`, in its own workspace, as `jupyter`
  is, read their formats with the `formats` hook, and have no settings:
  - `asciidoc` reads `.adoc` and `.asciidoc` files, with
    `asciidoc-parser`;
  - `orgmode` reads `.org` files, with `orgize` 0.10, pinned to
    `=0.10.0-alpha.10`, as an alpha's next release can break it.

  Each is named for its format, as `jupyter` is, and released from a tag
  that names it, like `asciidoc-v0.1.0`
  ([0052](./0052-releasing-plugins-from-tags.md)).
- They share `plugins/markup/`, a library, `little-markup`, in its own
  workspace, as the plugins that add an API reference share
  `api_reference`. Each plugin reads its format into the library's model
  of a document, its blocks, inlines, footnotes, and metadata, like its
  title, and the library writes it as `markdown` parts, with an `html`
  part for each block of raw HTML, with the site's `markdown` settings, as
  `formats` is given them:
  - text is escaped, so `*`, `#`, `|`, `$`, or `:smile:` shows as it's
    written, whatever extensions are on;
  - a link to a heading on the page, by the format's id for it, or its
    text, leads to the `#fragment` little gives the heading; any other id
    is an `<a id>`, written only with `raw_html`;
  - where the site turns an extension off, it writes what Markdown can,
    and warns of what's lost: an alert as a quote, with its kind in bold; a
    definition list as HTML, with `raw_html`, else a list; a table as a
    list; and subscript, superscript, and highlighting as HTML, else plain
    text;
  - a heading deeper than Markdown's sixth level is written at the sixth.
- A page is written as its format's own HTML would show it, as near as
  Markdown comes:
  - its title and description, from the format's own header, are its
    `title` and `description`; YAML frontmatter, between `---` lines at
    the top, as in Markdown, goes over them, and sets little's own keys,
    like `search`;
  - a link to a file a plugin reads, like `install.adoc`, is kept as it's
    written, so little leads it to the page, as it does a `.md` file's;
  - an include isn't followed, as a plugin reads only the file it's
    given: it's warned of, and left out;
  - an admonition is the nearest of GitHub's five alerts, and a block's
    title an emphasised paragraph before it, or after an image;
  - a table is GitHub's: a merged cell is written in its first place, with
    empty cells after it, and blocks in a cell are run together, and each
    is warned of;
  - raw HTML is kept, with `raw_html`, else left out, and warned of; raw
    output for other formats, like LaTeX, is left out, as an HTML export
    leaves it out;
  - math is TeX, written as `markdown.math` says; AsciiMath stays code,
    and is warned of;
  - what a format leaves out of its HTML stays out: comments, and, in
    Org mode, `:noexport:` and `COMMENT` subtrees, and planning lines; an
    Org headline's TODO keyword, priority, and tags, and its drawers, but
    a block's results, are left out too, as docs have no use for them, and
    no headline is numbered, but each is a heading, whatever `H:` says; an
    Org source block's `:exports` is followed, and nothing is run;
  - a macro, or other markup a plugin doesn't read, is warned of, with its
    line, and its text kept.
- The built-in theme takes the margins off the first and last blocks in a
  definition list's term or description, which hold paragraphs when
  they're written as HTML, and hides a table's header row when its cells
  are all empty, as the library writes one for a table that has none.
- The manual has a page on each, in its format:
  `docs/plugins/asciidoc.adoc` and `org-mode.org`, each read by its plugin
  built from the same commit, as the page on notebooks is.
- `asciidoc`, `orgmode`, and `markup` are scopes for commits
  ([0051](./0051-semantic-versioning-and-conventional-commits.md)).
  `markup` is released only inside the plugins, as `api_reference` is, so
  a format plugin's notes list the commits that changed its folder or
  `plugins/markup/`, as one that adds an API reference lists
  `plugins/api_reference/`, and `jupyter`'s list only its folder's
  ([0052](./0052-releasing-plugins-from-tags.md)).
- New crates, in the plugins, not in little: `asciidoc-parser`;
  `orgize`, and `rowan`, which it's built on; and a few small crates of
  theirs. Others they use little has already: `asciidoc-parser` uses
  `regex` and `ahash`, `orgize` uses `nom`, and `markup` uses
  `finl_unicode`, as comrak does to give a heading its `#fragment`, so
  the library works out the same one.

## Consequences
- A site in AsciiDoc or Org mode is built by little, with nothing else to
  install, not Ruby or Emacs, and its pages can be mixed with Markdown's,
  each linking to the others by their files; a theme styles them as it
  styles Markdown's.
- little itself doesn't grow: a site names the plugin for each format it
  uses, and `asciidoc` is about 2.1 MB, and `orgmode` about 0.6 MB.
- Limits, for now:
  - none of Asciidoctor's extensions is read, and nothing in Org mode is
    run;
  - an include isn't followed;
  - a link to an id on another page leads to that page, but little may
    give the id's place there another `#fragment`, which is warned of, as
    a broken link is;
  - a table's merged cells are split, and the blocks in a cell are run
    together;
  - AsciiMath isn't rendered;
  - `orgize` is an alpha, and reads some of Org as Org doesn't, so the
    plugin reads `$…$` math, a bare URL, and a few sub- and superscripts
    itself.
