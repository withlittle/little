# 0042 — Versions share the site's config, and its theme's files

- Status: accepted
- Date: 2026-10-14
- Amends: [0014](./0014-site-url-and-its-path.md),
  [0040](./0040-versions-from-git-refs.md)

## Context
A version from a git ref was the site as it was at its ref: its own
`little.yml`, Markdown settings, and plugins, in the site's theme, with a
few of the site's settings ([0040](./0040-versions-from-git-refs.md)).
Each version, built as a site under a path is
([0014](./0014-site-url-and-its-path.md)), had its own copy of the theme's
files, like `/1.4/assets/style.css`, though every version is in the same
theme, so each copy was the same.

What changes from release to release is a version's pages. If an author
kept 1.4's pages in `docs/1.4/`, they'd be built with the site's
settings, and link to the theme's one stylesheet; a version from a ref is
those pages, from git, with its own nav and search. Its ref's
`little.yml` sets what that release's little read, not what today's
does, and a release from before the site used little has none, so its
version had little's and the theme's settings, not the site's, like its
`name`. A plugin's `.wasm` file had to be committed at the ref, though
one built in CI isn't. And docs move: a release's pages may be in
`documentation/`, where today's are in `docs/`.

## Options considered
- **Whose settings a version has:** its ref's `little.yml`, over a few of
  the site's, as before, so it renders as it did, but needs a `little.yml`
  today's little reads, and a plugin's file committed; or the site's,
  with only what's a version's own, its pages and their nav, from its
  ref.
- **Where its pages are at its ref:** in the site's `source`; in the
  `source` of its ref's `little.yml`, if it has one; or in a `source` its
  entry in `versions` gives, for docs that have moved since, with or
  without a `little.yml`.
- **Where the theme's files are:** in each version's folder, so every
  version's are copies; or once, at the root of the site, with every
  version's links to them leading there.
- **Search:** one index of every version, with a filter for the reader's;
  or one for each, in its folder, as before.

## Decision
- Every version is built with the site's config, as the site's
  `little.yml` sets it: its settings, theme, and plugins, with their
  `.wasm` files and settings.
- A version from a ref has its own pages and files, from its `source` at
  the ref, and its own nav, from its pages and folders, and its plugins
  read their folders as they were there.
- Of its ref's `little.yml`, if it has one in the same folder of the
  repository as the site's, only `source` and `nav` count, over little's
  defaults and the site's themes, as the site's own would be. A ref
  without one has the site's `source`, and a nav from its folders.
- An entry in `versions` with a `ref` can set `source`, the folder its
  pages were in at the ref, relative to the site's folder, which wins over
  both:

  ```yaml
  versions:
    - title: "2.0"
      url: /
    - title: "1.3"
      url: /1.3
      ref: v1.3
      source: ./documentation
  ```

  One without a `ref` can't, as its pages are in the site's `source`. A
  ref without its `source` folder stops the build, saying to set the
  version's `source`.
- The theme's static files are written once, with the version at `/`. In
  every other version, the `url` filter, and a link in Markdown, lead to
  one of them at the root of the site, so `{{ '/assets/style.css' | url }}`
  is `/little/assets/style.css` in `/little/1.4/`. A version's own file at
  the same path is its own, in its folder, and its links lead to it, as a
  site's file replaces the theme's.
- Each version keeps its own search index, in its folder, so a search
  finds the pages of the version it's made in.
- little checks out only a version's `little.yml`, its source, and the
  folders its plugins read, so it compiles a plugin once, from the site's
  file, as it did before versions.

## Consequences
- A site has one copy of the theme's files, however many versions it has,
  so there's less to publish, and a reader's browser fetches the
  stylesheet once for every version.
- Every version is rendered alike, and gets a plugin the site adds; one
  from before the site used little is built with the site's settings and
  its own nav.
- An old version is rendered with today's settings, not those of its day,
  so a change to one, like `markdown`, changes it too.
- A plugin the site uses must find the folders it reads at every
  version's ref, or the build stops; there's no plugin, or setting, for
  one version, for now.
- A file of the version at `/` at the path of one of the theme's replaces
  it for every version, as they all link to it.
