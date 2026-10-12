---
title: Versions
description: A version of the site for each release, built from its git tag or branch, with a menu of them in the built-in theme.
---

# Versions

A product that's released keeps more than one release in use, so its docs
need a version for each: a reader of 1.4 needs 1.4's pages, not 2.0's.
little builds each version of a site from a git tag, branch, or commit, so
there's no copy of the docs to keep for each, and the built-in theme adds a
menu of them to its header.

## Listing versions

List the site's versions in `versions`, as the menu lists them, the latest
first, each with the `title` readers see, the `url` it's published at, and
the `ref` its files are read from:

```yaml
# little.yml
versions:
  - title: "2.0"   # no ref: the site's files, as they are
    url: /
  - title: "1.4"
    url: /1.4
    ref: v1.4
  - title: "1.3"
    url: /1.3
    ref: v1.3
```

- `little build` builds every version, each time, each at its `url`, so
  1.4's `docs/guide/install.md` is at `/1.4/guide/install`, or, under the
  path of the site's
  [`url`](../getting-started/publishing.md#choosing-the-sites-url), like
  `/little`, at `/little/1.4/guide/install`.
- `little serve` serves every version, and builds them all again whenever
  you save. It doesn't watch their refs, so a commit to a version's branch
  shows on your next save.
- A `ref` is a tag, like `v1.4`, a branch, like `origin/1.x`, or a commit,
  as git names it. A version from a branch can be fixed after it's
  released, by committing to the branch; one from a tag can't.
- One version, at most, has no `ref`, and is built from the site's files as
  they are, so `little serve` shows your edits to it. It needn't be the one
  at `/`: see [Releasing a version](#releasing-a-version).
- A `title` is what the menu shows, like `1.4` or `2.0 (Alpha)`. Put it in
  quotes if it looks like a number, as YAML reads `1.10` as `1.1`. Two
  versions can't have the same title. Keep it short, as the built-in
  theme's menu shows it on one line.
- A `url` is from the root of the site, without the path of the site's
  `url`, as in the [nav](navigation.md#writing-the-nav-yourself): little
  adds it. Each part between its `/`s names a folder, so it's letters,
  digits, `.`, `-`, and `_`, starting with a letter or a digit. Two
  versions can't be at the same `url`.
- One version must be at `/`, the site's home, so readers land on it. It
  needn't be the first, or the latest. Others go in folders of it, like
  `/1.4` or `/versions/1.4`, but not in each other's, like `/2.0/beta` in
  `/2.0`.
- A site with a version from a `ref` must be in a git repository, with git
  installed, or `little build` and `little serve` stop, and say why. A site
  without `versions` has just the one, its files as they are.

## What's in a version

A version from a `ref` is the site as it was there: its own `little.yml`,
pages, nav, [Markdown's extensions](markdown.md#choosing-markdowns-extensions),
and [plugins](../plugins/using-plugins.md), which read their folders as they
were, so an older version's [API
reference](../plugins/python-api-reference.md) is of its own code. little
reads its files with git, into a folder of its own, which it deletes once
it's built them, so your files, and your repository, don't change.

But it's built by the little you run, in the site's theme, so every version
looks alike, has the menu, and gets little's fixes:

- Its `theme`, `url`, `destination`, `repo`, `host`, `port`, and `versions`
  are the site's, whatever its own `little.yml` sets, so a tag's needn't
  list the versions after it.
- It must build with today's little: a setting little doesn't accept, or a
  plugin's `.wasm` file that isn't committed at the ref, as one built in CI
  isn't, stops the build, naming the version.
- Its pages link to their files at the ref's commit, so **View source of
  this page** shows the file as it was, but they have no **Edit this
  page**, as a tag can't be edited.
- Each version has its own copy of the theme's files, like
  `/1.4/assets/style.css`, and its own [search](search.md) index, so a
  search finds the pages of the version it's made in. The [404
  page](../getting-started/publishing.md#a-404-page) is the one at `/`'s.
- A page or file of the version at `/` where another version goes, like
  `docs/1.4.md` or `docs/1.4/setup.png` for a version at `/1.4`, stops the
  build, naming both, as [pages that
  clash](pages-and-files.md#pages-that-clash) do. So does a version at
  `/pagefind`, where search's files go.

## The menu of versions

With two or more versions, the built-in theme's header has a menu of them,
beside the site's name, or below it on a phone, showing the version you're
reading:

- Each version in it leads to the page you're on, in that version, if it
  has a page at the same URL, or else to its home page. So from
  `/1.4/guide/install`, 2.0 leads to `/guide/install`.
- On each version but the one at `/`, a note above the page says which
  version it's for, and links to the one at `/`, at the same page if it has
  one.
- It's a `<details>`, so it works without a script. With the theme's, a
  click elsewhere, or Escape, closes it.

A theme of your own can have one too: see [A menu of
versions](../themes/making-a-theme.md#a-menu-of-versions).

## Checking every version

`little build` checks every version as it checks a site with one: its
[links](links-and-images.md#checking-links) within the site, formulas,
diagrams, and plugins' blocks, warning of each that's broken, and naming a
file from a ref as git does, like `v1.4:docs/guide.md`. `--strict` fails on
them.

- A version from a ref's links to other sites aren't checked: they break as
  the web changes, and a tag can't be fixed.
- Nothing else in a tag can be fixed either, so check a release with
  `little build --strict` before you tag it.

## Releasing a version

Tag each release, as you would anyway, so its docs can be built from the
tag, then list it in `little.yml`:

- If the site's files are the latest release's docs, as above, retitle the
  version at `/` for the new release, and list the one before it below,
  from its tag, in a folder of its own:

  ```yaml
  # little.yml
  versions:
    - title: "2.1"
      url: /
    - title: "2.0"
      url: /2.0
      ref: v2.0
    - title: "1.4"
      url: /1.4
      ref: v1.4
  ```

- To keep readers on the latest release while you write the next one's
  docs, give the version at `/` the release's tag, and publish the site's
  files in a folder, like `/dev`:

  ```yaml
  # little.yml
  versions:
    - title: dev   # no ref: the site's files, as they are
      url: /dev
    - title: "2.1 (Latest)"
      url: /
      ref: v2.1
    - title: "2.0"
      url: /2.0
      ref: v2.0
  ```

## Publishing versions

A site's CI must fetch each version's ref. GitHub's `actions/checkout`
fetches just the last commit, with no tags, unless you give it
`fetch-depth: 0`, so add that to the
[workflow](../getting-started/publishing.md#publishing-on-github-pages) that
publishes the site:

```yaml
# .github/workflows/docs.yml
steps:
  - uses: actions/checkout@v7
    with:
      fetch-depth: 0
```

- Without it, the build stops, saying to fetch the repository's whole
  history.
- In CI, only the branch being built is checked out, so name another as it
  is on the remote, like `origin/1.x`, not `1.x`. That's found on your
  computer too, once you've fetched it.
- A fix on a version's branch is published when the site's next built, as
  on the next push to `main`.

## Limits, for now

- Each build builds every version, so a build, and each save in
  `little serve`, takes longer for each.
- A version is only at its `url`: there's no alias for one, like
  `/latest/`, or redirect to one, so a release that moves a version, as
  from `/` to `/2.0`, moves its pages.
- Every version is built from the site's repository, so the menu can't list
  docs published somewhere else, like those from before the site used
  little.
- A search finds the pages of its own version.
- A version's `little.yml` must be in the same folder of the repository as
  the site's. Without one there, it's built with little's and the theme's
  settings, so it has the theme's `name`.
