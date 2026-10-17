# 0049 — An API reference's URL and title

- Status: accepted
- Date: 2026-10-17
- Amends: [0031](./0031-a-plugin-for-pythons-docstrings.md),
  [0044](./0044-a-plugin-for-rusts-doc-comments.md),
  [0048](./0048-a-plugin-for-javascripts-doc-comments.md)

## Context
Each plugin that adds an API reference, `pydoc`
([0031](./0031-a-plugin-for-pythons-docstrings.md)), `rustdoc`
([0044](./0044-a-plugin-for-rusts-doc-comments.md)), and `jsdoc`
([0048](./0048-a-plugin-for-javascripts-doc-comments.md)), puts its pages
under `/reference` unless `url` is set, so they're at
`/reference/mypackage/`, `/reference/my_crate/`, or `/reference/my-lib/`.
That's a folder of little's choosing, which a site may not want at all, or
may keep for pages of its own, as little's manual does for its commands
and settings.

The page of what each documents, the package or crate, is titled with its
name, in the nav, the breadcrumbs, the browser's tab, and search, so a
site that would call it "API reference" has to write a nav of its own.
rustdoc and jsdoc need that name, in `crate` and `package`, as it's in
`Cargo.toml` or `package.json`, outside the one folder they read: it's
the page's heading and title, its folder unless `url` is set, and how
links name it, like ``[`my_crate::Config`]`` or
`{@link module:my-lib/config}`.

## Options considered
- **Where the pages go, unless `url` is set:**
  - `/reference`, as now, a folder a site may not want;
  - `/api`, another folder a site may not want;
  - nowhere: `url` required, a line every site writes;
  - the name of what it documents, at the source's root, with nothing
    before it.
- **What the page of what it documents is called:**
  - its name, as now, unless a site writes a nav of its own;
  - a setting of little's, retitling any page from `little.yml`, which
    grows little's core for what a plugin can do itself;
  - a setting of each plugin's, `title`, which it writes in the page's
    frontmatter, which wins over its heading
    ([0006](./0006-frontmatter-and-config-win.md)), so little's core
    doesn't change;
  - for pydoc, a page at `url` that lists its packages, titled `title`,
    which adds a level above the package rather than renaming it, and,
    at `/`, would be the home page.
- **The page's heading:** the title, which would leave the name off the
  page; or the name, as now.

## Decision
- Unless `url` is set, a reference goes at the root of the source:
  - pydoc's in `/`, so each package is in a folder of its name, like
    `/mypackage/`, and a module outside a package is at the root;
  - rustdoc's in the crate's name, like `/my_crate`;
  - jsdoc's in the package's name, scope and all, like `/my-lib` or
    `/@my-org/my-lib`.

  What `url` means doesn't change: for rustdoc and jsdoc, it's the folder
  of the crate's or package's page, and for pydoc, the folder its
  packages are in, so `url: /api` works as it did.
- Each takes `title`, a string with more than spaces in it, for the title
  of the page of what it documents, like `API reference`:
  - rustdoc's crate's, or jsdoc's package's, `README.md` in `url`;
  - pydoc's one package's or module's, like `mypackage/README.md` in
    `url`. With `title`, the folders in `read` hold that one alone, like
    `./mypackage`, or `./src` for `src/mypackage/`; more stops the build,
    naming them. None, which is warned of already, or a module that can't
    be parsed, which has no page, leaves nothing to title.

  The plugin writes it in frontmatter at the top of the page, as a quoted
  YAML string, so a `:`, `#`, or quote reads back as it was set. It wins
  over the page's `#` heading, which is still the name. It's what the nav
  little makes calls the page, and so the breadcrumbs of the pages below
  it, and what the browser's tab and search show; a nav written in
  `little.yml` keeps its own titles.
- `plugins/api_reference/` writes the frontmatter, for all three.
- `crate` and `package` stay, and are still needed: without them, links
  that name the crate or package would lead nowhere, and the default
  `url` would need another name.
- The manual sets `title: little's API` for rustdoc, at `url: /api`, so
  the tab of `/api/` says what its nav does.
- 0031, 0044, and 0048 are kept as they were written, as history.

## Consequences
- A reference is at the top of a site, where the nav little makes lists it
  among the site's own pages and folders, by its title, rather than in a
  "Reference" folder; `url` puts it anywhere else.
- A site that relied on the old default finds its reference moved, and its
  own links to the old pages warned of as broken, which `--strict` fails
  on; `url: /reference` for pydoc, or `url: /reference/<name>` for
  rustdoc and jsdoc, keeps it where it was. None of the plugins is
  released, and the manual sets `url`, so that's few sites.
- A page of the site's at the same path, like `docs/my-lib.md` or
  `docs/my-lib/README.md`, stops the build, naming both, as two pages at
  one path do ([0013](./0013-pages-that-clash.md)); `url` moves the
  reference.
- A scoped package is in a folder of its scope, like `/@my-org/my-lib/`,
  which the nav calls "@my Org", as it did in `/reference/`. Leaving the
  scope out would be shorter, but packages of the same name in two scopes
  would clash.
- A version can set its own `title`, as it can any of a plugin's settings
  ([0043](./0043-a-versions-own-nav-and-plugins.md)).
- A site whose pydoc documents more than one package or module, with
  `title`, gives each an entry of its own under `plugins`, with its own
  `title`, or leaves `title` out, and writes the page above them.
