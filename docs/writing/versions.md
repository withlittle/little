---
title: Versions
description: A version of the site for each release, built from its git tag or branch, with a menu of them in the built-in theme.
---

# Versions

A reader of 1.4 needs 1.4's docs, not 2.0's. So list your releases in
`versions`, and little builds a version of the site for each, from its git
tag, branch, or commit:

```yaml
# little.yml
versions:
  - title: "2.0"   # no ref: the site's files, as they are
    url: /
  - title: "1.4"
    url: /1.4
    ref: v1.4
```

There's no copy of the docs to keep for each release. The built-in theme
adds a menu of the versions to its header.

## Listing versions

List the versions as the menu lists them, the latest first. Each has:

| Key | What it is |
| --- | ---------- |
| `title` | what the menu shows, like `1.4` or `2.0 (Alpha)` |
| `url` | where it's published, from the root of the site, like `/1.4` |
| `ref` | the git tag, branch, or commit its files are read from, like `v1.4`. Leave it out for the site's files as they are. |
| `source` | the folder its pages were in at its `ref`, if they've moved since, like `./documentation` |

`little build` builds every version, each at its `url`. So 1.4's
`docs/guide/install.md` is at `/1.4/guide/install`. `little serve` serves
every version, and builds them all again whenever you save.

- One version must be at `/`, the site's home, so readers land on it. It
  needn't be the first, or the latest. Others go in folders of it, like
  `/1.4` or `/versions/1.4`, but not in each other's, like `/2.0/beta` in
  `/2.0`.
- At most one version has no `ref`. It's built from the site's files as
  they are, so `little serve` shows your edits to it. It needn't be the one
  at `/`: see [Releasing a version](#releasing-a-version).
- A `ref` is named as git names it: a tag, like `v1.4`, a branch, like
  `origin/1.x`, or a commit. A version from a branch can be fixed after
  it's released, by committing to the branch. One from a tag can't.
- `little serve` doesn't watch refs, so a commit to a version's branch
  shows on your next save.
- Put a `title` in quotes if it looks like a number, as YAML reads `1.10`
  as `1.1`. Keep it short, as the built-in theme's menu shows it on one
  line. Two versions can't have the same title.
- Write a `url` without the path of the site's
  [`url`](../getting-started/publishing.md#choosing-the-sites-url), as in
  the [nav](navigation.md#writing-the-nav-yourself). little adds it, so
  under `/little`, 1.4's install page is at `/little/1.4/guide/install`.
- Each part of a `url` between its `/`s names a folder. So it's letters,
  digits, `.`, `-`, and `_`, starting with a letter or a digit. Two
  versions can't be at the same `url`.
- A `source` is relative to the site's folder, like the site's own
  `source`. A version with no `ref` can't have one: its pages are in the
  site's `source`.
- A site with a version from a `ref` must be in a git repository, with git
  installed. Otherwise, `little build` and `little serve` stop, and say
  why. The site's folder, where its `little.yml` is, must be in the
  repository, not just its `source`, as little finds each version's files
  from the same folder at its `ref`.
- A site without `versions` has just the one version, its files as they
  are.

## Releasing a version

Before you tag a release, check its docs with `little build --strict`, as
nothing in a tag can be fixed. Then tag it, as you would anyway, and list
it in `little.yml`.

If the site's files are the latest release's docs, as above, retitle the
version at `/` for the new release. List the one before it below, from its
tag, in a folder of its own:

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

To keep readers on the latest release while you write the next one's docs,
give the version at `/` the release's tag. Publish the site's files in a
folder, like `/dev`:

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

CI must fetch each version's ref. GitHub's `actions/checkout` fetches just
the last commit, with no tags, unless you give it `fetch-depth: 0`. So add
that to the
[workflow](../getting-started/publishing.md#publishing-on-github-pages)
that publishes the site:

```yaml
# .github/workflows/docs.yml
steps:
  - uses: actions/checkout@v7
    with:
      fetch-depth: 0
```

- Without it, the build stops, saying to fetch the repository's whole
  history.
- In CI, only the branch being built is checked out. So name another branch
  as it is on the remote, like `origin/1.x`, not `1.x`. That name works on
  your computer too, once you've fetched it.
- A fix on a version's branch is published when the site's next built, like
  on the next push to `main`.

## The menu of versions

With two or more versions, the built-in theme's header has a menu of them,
beside the site's name, or below it on a phone. It shows the version you're
reading.

- Each version in it leads to the page you're on, in that version, if it
  has a page at the same URL. Otherwise, it leads to that version's home
  page. So from `/1.4/guide/install`, 2.0 leads to `/guide/install`.
- On each version but the one at `/`, a note above the page says which
  version it's for. It links to the version at `/`, at the same page if it
  has one.
- It's a popover, so it works without a script. A click elsewhere, or
  Escape, closes it.
- It opens below the version you're reading. In a browser without CSS
  anchor positioning, it opens in the middle of the window. Every major
  browser has had anchor positioning since January 2026.

A theme of your own can have one too: see
[A menu of versions](../themes/making-a-theme.md#a-menu-of-versions).

## What's in a version

Each version is built by the little you run, with the site's settings,
theme, and [plugins](../plugins/using-plugins.md). So every version looks
alike, has the menu, and gets little's fixes.

What's in a version's folder, at its `url`, is its own:

- Its pages, and their images and other files, from the folder they were in
  at its `ref`. That's its `source` in `versions`, if it has one.
  Otherwise, it's the `source` of the `little.yml` at its `ref`, if there's
  one there, or else the site's `source`.
- Its [nav](navigation.md#the-nav). That's its
  [own](#a-versions-own-nav-and-plugins), if its entry in `versions` sets
  one. Otherwise, it's the `nav` of the `little.yml` at its `ref`, if it
  sets one, or else one from its pages and folders.
- The folders its plugins read, as they were at its `ref`. So an older
  version's [API reference](../plugins/pydoc.md) is of its
  own code, with any [settings](#a-versions-own-nav-and-plugins) its entry
  gives the plugins.
- Its [search](search.md) index, so a search finds the pages of the version
  it's made in.

little reads a version's files with git, into a folder of its own, which it
deletes once it's built them. Your files, and your repository, don't
change.

- Of the `little.yml` at a version's `ref`, only `source` and `nav` count,
  and its entry's win over them. Every other setting there, like `name`,
  `markdown`, or `plugins`, is the site's. So a tag's `little.yml` needn't
  list the versions after it. A tag from before the site used little, with
  no `little.yml`, is built too.
- A version must build with today's little. A `ref` without its pages'
  folder, or a folder a plugin reads, stops the build, naming the version.
  Its entry can give the folder they were in there, or turn the plugin off.
- Its pages link to their files at the ref's commit, so **View source of
  this page** shows the file as it was. They have no **Edit this page**, as
  a tag can't be edited.
- The theme's files, like `/assets/style.css`, are written once, with the
  version at `/`. Every version's pages link to them there. But a version's
  own file at the same path, like `docs/assets/style.css` at its `ref`, is
  its own, as a site's file replaces the theme's. Its pages link to it, in
  its folder.
- Every version has the [404 page](../getting-started/publishing.md#a-404-page)
  of the version at `/`.
- A page or file of the version at `/` where another version goes stops the
  build, naming both, as [pages that clash](pages-and-files.md#pages-that-clash)
  do. For a version at `/1.4`, that's a file like `docs/1.4.md` or
  `docs/1.4/setup.png`. A version at `/pagefind`, where search's files go,
  stops the build too.

## A version's own nav and plugins

An old release's docs may not fit today's settings. Its nav may not be the
one you'd give it now, or a plugin may read a folder that's moved since. So
a version from a `ref` can set its own `nav` and `plugins`, beside its
`source`, in its entry in `versions`:

```yaml
# little.yml
plugins:
  pydoc:
    wasm: ./little_pydoc.wasm
    read: ./src
versions:
  - title: "2.0"
    url: /
  - title: "1.4"
    url: /1.4
    ref: v1.4
    nav:
      - { title: Home, url: / }
      - { title: Install, url: /guide/install }
    plugins:
      pydoc:
        read: ./lib   # where its code was, at v1.4
  - title: "1.3"
    url: /1.3
    ref: v1.3
    plugins:
      pydoc: false   # 1.3 had no API reference
```

- Its `nav` is written as the
  [site's](navigation.md#writing-the-nav-yourself) is. It replaces the
  `nav` of the `little.yml` at its `ref`, or the one from its pages and
  folders.
- Write each URL in its `nav` from the version's home, as if it were the
  site's. little adds the version's `url`, so 1.4's `/guide/install` is at
  `/1.4/guide/install`.
- Its `plugins` go over the site's, setting by setting, as the site's go
  over a theme's. So 1.4's `pydoc` has the site's `wasm`, but its own
  `read`.
- A `read` there is from the site's folder at the version's `ref`, as its
  `source` is. `false` turns a plugin off for that version alone.
- It can set only plugins the site uses, and can't give one a `.wasm`
  file, as every version is built with the site's.
- A version without a `ref` can't set either, as it's built from the
  site's files as they are. Set the site's instead.
- Every other setting of little's, like `theme`, `search`, or `markdown`,
  is the site's, for every version. So one in a version's entry stops the
  build.
- Any other key, like `released: 2025-06-01`, is for a theme to show, in
  [`site.versions`](../themes/making-a-theme.md#a-menu-of-versions). So a
  `name` there is the version's, not the site's.

## Checking every version

`little build` checks every version as it checks a site with one: its
[links](links-and-images.md#checking-links) within the site, formulas,
diagrams, and plugins' blocks. It warns of each that's broken, naming a
file from a ref as git does, like `v1.4:docs/guide.md`. `--strict` fails on
them.

A version from a ref doesn't have its links to other sites checked: they
break as the web changes, and a tag can't be fixed.

## Limits, for now

- Each build builds every version, so a build, and each save in
  `little serve`, takes longer for each one.
- A version is only at its `url`. There's no alias for one, like
  `/latest/`, or redirect to one. So a release that moves a version, as
  from `/` to `/2.0`, moves its pages.
- Every version is built from the site's repository. So the menu can't list
  docs published somewhere else, like an old release's, published with
  another tool.
- Every version has the site's settings, but for its own `source`, `nav`,
  and plugins' settings. So an old version is rendered with today's
  `theme`, `markdown`, and the rest, not as it was.
- A file of the version at `/` that replaces one of the theme's, like
  `docs/assets/style.css`, replaces it for every version, as they all link
  to the one at `/`.
- little looks for a version's `little.yml` only in the same folder of the
  repository as the site's. If the site's folder has moved since, set the
  version's `source`. Its nav is then built from its pages and folders,
  unless you set its `nav` too.
