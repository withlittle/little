# 0040 — Versions of a site, built from git refs

- Status: accepted; amended by [0041](./0041-version-titles-and-urls.md),
  which gives each version a `title`, for readers, and a `url`, where it's
  published, in place of its `name`, with one at `/`, whatever its place
  in the list, and by
  [0042](./0042-versions-share-the-sites-config-and-theme-files.md), which
  builds every version with the site's config, but for its own `source`
  and `nav`, and writes the theme's files once, at the root
- Date: 2026-10-12
- Amends: [0013](./0013-pages-that-clash.md),
  [0014](./0014-site-url-and-its-path.md),
  [0016](./0016-checking-links.md),
  [0034](./0034-the-built-in-themes-script.md),
  [0039](./0039-a-long-site-name-in-the-built-in-themes-header.md)

## Context
A product that's released keeps more than one release in use, so its docs
need a version for each: a reader of 1.4 needs 1.4's pages, not 2.0's. A
site built by little has one version, of the files as they are.

Docusaurus copies the docs into a folder for each version, with
`docusaurus docs:version 1.4`, so every version is in the source, built
by each build, and edited as any page is; it puts the latest at the root,
the rest in folders, like `/1.4/`, and recommends fewer than ten. mike
builds each version on its own with MkDocs, and commits each build to the
`gh-pages` branch, so an old version is never built again: it keeps the
theme, and the bugs, of its day, and its menu of versions is a script
that reads a `versions.json`. Both are configured beside the site.

A product's repository already holds its versions: a tag for each
release, and a branch for each line it fixes.

## Options considered
- **Where a version's files come from:** a copy in the source for each,
  as Docusaurus makes, so each page is there once for each version, and
  a fix to an old one is made in its copy; a build of its own, kept on a
  branch for the built site, as mike keeps; or a git ref, a tag, branch,
  or commit, that little reads its files from, so nothing is copied.
- **What builds it:** the little of its day, once, as mike does, so it
  can't change; or the little building the site, each time, so it gets
  little's fixes, and the site's theme, but must build with today's
  little.
- **Whose theme it's in:** its own, at its ref, so each version looks as
  it did, and an old one has no menu of the versions after it; or the
  site's, so every version looks alike, and each has the menu.
- **Where each version is:** each in a folder named for it, with a
  redirect, or an alias, from the root to the latest, as mike does,
  which is a page little writes, not the theme; or the first at the
  root, and the rest in folders, as Docusaurus does, so the root needs
  no redirect.
- **How the theme finds the same page in another version:** by a script
  that reads a list of each version's pages; or from little, which knows
  them as it builds them all.

## Decision
- `versions` lists the site's versions, the latest first, each with a
  `name` and the `ref` it's read from, a tag, branch, or commit, as git
  names it:

  ```yaml
  versions:
    - name: "2.0"   # no ref: the site's files, as they are
    - name: "1.4"
      ref: v1.4
  ```

  At most one has no `ref`. A site without `versions` is built as it was.
- A `name` is text, like `"2.0"` in quotes, as YAML reads `2.0` as a
  number: letters, digits, `.`, `-`, and `_`, starting with a letter or a
  digit, so it can be a folder in a URL. Two versions can't have the same
  name.
- `little build` builds every version in one build. The first is at the
  root of the site, and each other in a folder named for it, like `/1.4/`,
  built as a site under a path is ([0014](./0014-site-url-and-its-path.md)):
  its `url` is the site's with `/1.4` after it, so the URLs little gives
  the theme, the links in its Markdown, and the URLs the theme writes with
  `url` stay in it. Each has its own copy of the theme's files, and its
  own search index. The 404 page is the first version's.
- A version with a `ref` is the site as it was at that ref: its own
  `little.yml`, source, nav, Markdown settings, and plugins, which read
  their folders as they were. little reads them with `git`, as a checkout
  of the ref would have them, into a folder of its own, which it deletes
  once the build's done, so the site's files and repository don't change.
  It's built in the site's theme, by the little that builds the site, with
  the site's `theme`, `url`, `destination`, `repo`, `host`, `port`, and
  `versions`.
- The theme gets:
  - `site.versions`: each entry in `versions`, with the `url` of its
    version's home page;
  - `site.version`: the name of the version it's building, worked out
    beneath the config, as `site.year` is
    ([0006](./0006-frontmatter-and-config-win.md));
  - `page.versions`: each entry in `versions`, with the `url` of the page
    in that version, at the same URL from its root, or, if it's not
    there, of that version's home page.
- The built-in theme has a menu of versions in its header, when a site
  has two or more: a `<details>`, beside the site's name, or below it on a
  phone, where the name keeps to one line, cut short with an ellipsis
  ([0039](./0039-a-long-site-name-in-the-built-in-themes-header.md)). It
  works without a script, and with one, closes on a click elsewhere, or
  Escape. On each version but the first, a note above the page names its
  version, and links to the page in the first.
- A page from a ref links to its file at the ref's commit, as
  `page.source_url`, but has no `page.edit_url`, as a tag or a commit
  can't be edited.
- A ref git can't find, as in a shallow clone, which is how CI often
  checks a repository out, stops the build, saying to fetch its whole
  history, as `actions/checkout` does with `fetch-depth: 0`. So does a site
  with `versions` that isn't in a git repository, or a version little
  can't build, naming it. A page or a file of the first version where
  another version's folder goes, like `docs/1.4.md`, stops the build,
  naming both, as two files that would be the same page do
  ([0013](./0013-pages-that-clash.md)).
- Every version's links within the site are checked, and its formulas,
  diagrams, and plugins' blocks and warnings warned of, naming each file
  from a ref as git does, like `v1.4:docs/guide.md`, and `--strict` fails
  on them. But a version from a ref's links to other sites aren't checked
  ([0016](./0016-checking-links.md)): they break as the web changes, and a
  tag can't be fixed.
- `little serve` serves every version, and builds them all again on each
  change.

## Consequences
- Releasing a version of the docs is tagging the release, which a product
  does anyway. The source holds each page once, and searching it finds
  each once.
- An old version gets today's theme, with its menu, and little's fixes,
  but must build with today's little: a setting little no longer reads,
  or a plugin's file that isn't committed, stops the build. A version from
  a branch, like `origin/1.x`, can be fixed, where one from a tag can't.
- A site's CI must fetch each version's ref.
- Limits, for now:
  - each build builds every version, so a build, and each save in
    `little serve`, takes longer for each;
  - a version is only at its folder: no alias, like `/latest/`, or
    redirect;
  - a version can't be published somewhere else, like docs from before
    the site used little;
  - search finds the pages of its own version;
  - a version's `little.yml` must be in the same folder of the
    repository as the site's.
