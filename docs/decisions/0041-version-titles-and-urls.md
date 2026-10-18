# 0041 — A version's title and URL

- Status: accepted
- Date: 2026-10-12
- Amends: [0040](./0040-versions-from-git-refs.md)

## Context
Each version in `versions` had a `name`
([0040](./0040-versions-from-git-refs.md)), which did two jobs: it was what
the menu showed, and it named the version's folder, so it was limited to
letters, digits, `.`, `-`, and `_`. A version couldn't be shown as
`2.0 (Alpha)`, or published at `/versions/1.4`, and the first in the list
was always at the root, so the menu's order chose where readers landed.

The nav already lists items as `{ title, url }`: what a reader sees, and
where it leads. No release of little has `versions` yet, so no site uses
`name`.

## Options considered
- **What an entry holds:** `name`, as it was, with an optional label for
  the menu; or `title` and `url`, as the nav's items do, so each says what
  it shows and where it is.
- **Whether `url` is required:** optional, with a folder made from the
  title, which needs rules to turn `2.0 (Alpha)` into a folder; or
  required, so each entry says where it is, and the root is chosen, not
  the first by its order.
- **Whether a version must be at `/`:** a site's home could be empty, but
  little writes no redirects, which aren't the theme's
  ([0040](./0040-versions-from-git-refs.md)), so readers who go to the
  site would find nothing; or one is at `/`, always.
- **How the theme finds the version at `/`:** by its place in the list,
  which no longer says; by comparing each `url` with `/`, which a theme
  can't, as the `url` filter puts each version's own folder before it; or
  from little, which knows.

## Decision
- Each version in `versions` has a `title`, what readers see, any text,
  in quotes if YAML would read it as a number; a `url`, where it's
  published, from the root of the site, without the path of the site's
  `url`, as the nav's are; and a `ref`, as before:

  ```yaml
  versions:
    - title: "2.0 (Alpha)"   # no ref: the site's files, as they are
      url: /2.0-alpha
    - title: "1.4 (Latest)"
      url: /
      ref: v1.4
    - title: "1.3"
      url: /versions/1.3
      ref: v1.3
  ```

- `name` is gone. Two versions can't have the same title, or be at the
  same `url`.
- Each part of a `url` between its `/`s names a folder, so it's letters,
  digits, `.`, `-`, and `_`, starting with a letter or a digit, and a `/`
  at its end is left off. URLs that differ only in case aren't checked, as
  pages' aren't ([0013](./0013-pages-that-clash.md)).
- Exactly one version is at `/`. Others can be in its folders, at any
  depth, like `/versions/1.3`, but not in each other's, like `/2.0/beta` in
  `/2.0`, as only the version at `/` has its files checked against the
  folders other versions go in.
- The order of `versions` is only the menu's order. The version at `/` has
  the 404 page, and a page or file of it where another version goes stops
  the build, as before; so does a version where search's files go.
- `site.version` is the title of the version being built. Each entry in
  `site.versions` and `page.versions` has the keys it's written with, but
  `url`, which little sets to the version's home page, or the page in it,
  with the path of the site's `url`, as before, and `root`, which little
  sets true for the version at `/`, and false for the others.
- The built-in theme's menu shows each version's title, and its note, on
  each version but the one at `/`, links to the one at `/`, which it finds
  with `selectattr("root")`.

## Consequences
- A version can be shown as readers know it, like `1.4 (Latest)`, and put
  where an author wants it, and the nav and `versions` are written alike.
- Every entry has a `url`, even in the common case, where it's the title
  with a `/` before it.
- A theme reads `version.title`, not `version.name`, and finds the version
  at `/` from `root`, not by its place.
- A long title crowds the built-in theme's header, as the menu shows it on
  one line; the manual says to keep it short.
