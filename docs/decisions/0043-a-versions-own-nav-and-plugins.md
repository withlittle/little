# 0043 — A version's own nav, and its plugins' settings

- Status: accepted
- Date: 2026-10-14
- Amends: [0042](./0042-versions-share-the-sites-config-and-theme-files.md)

## Context
A version from a git ref is a release's docs, shown in today's site
([0042](./0042-versions-share-the-sites-config-and-theme-files.md)): it's
built with the site's config, but has its own pages, from its `source`
there, and its own nav, from its ref's `little.yml`, if it sets one, or
its folders. Of its entry in `versions`, only `source` counted; every
other key was for the theme.

A release's docs may have been a site of their own, with no `little.yml`,
or one that set any of little's settings differently, so a version could
set any of them, as each of mike's versions is a build of its own, with
its day's config. But two of a version's needs couldn't be met at all:

- Its nav: one from before the site used little got a nav from its
  folders, sorted by title, not in the order its old tool gave it, and a
  tag can't be given a `little.yml` after it's made.
- Its plugins: every version had the site's, with the site's settings, so
  a plugin whose folders had moved since, like `lib/` to `src/`, or one
  for a release with nothing for it to read, stopped the build, with no
  way to fix it but to drop the version.

## Options considered
- **What an entry can set:** any of the site's settings, over the site's,
  so a version renders as it did, in its own theme, with its own
  `search`, `markdown`, and `name`; or only what's a version's own, its
  pages, from its `source`, their nav, and the folders its plugins read,
  with their settings.
- **Of a plugin:** its settings, over the site's, setting by setting, and
  `false`, to turn it off; or its `.wasm` file, too, for a plugin of the
  release's day.
- **`markdown`:** a version's, as how its pages are written; or the site's,
  as it mixes how pages are written, like `math`, with what a theme
  expects of them, like `highlight`'s classes, or `math: tex` for its
  script.
- **little's other settings in an entry:** for the theme, as any other key
  is, so `search: false` there quietly does nothing; or an error.

## Decision
- An entry in `versions` with a `ref` can set its own `nav` and `plugins`,
  beside its `source`, over its ref's `little.yml` and the site's:

  ```yaml
  versions:
    - title: "2.0"
      url: /
    - title: "1.4"
      url: /1.4
      ref: v1.4
      nav:
        - { title: Home, url: / }
      plugins:
        docstrings:
          read: ./lib
    - title: "1.3"
      url: /1.3
      ref: v1.3
      plugins:
        docstrings: false
  ```

- Its `nav` replaces the one its ref's `little.yml` sets, or that from its
  folders. Each URL is from the version's home, as if it were the site's,
  and little adds the version's `url`, as it does to its ref's `nav`.
- Its `plugins` go over the site's, setting by setting, as the site's go
  over a theme's, or turn one off, with `false`, for that version alone. A
  `read` there is from the site's folder at the ref, as its `source` is.
  It can name only plugins the site uses, so a typo stops the build, and
  can't give one a `.wasm` file: every version is built with the site's,
  the ones `little serve` watches, each compiled once.
- An entry without a `ref` can set none of them, as it's built from the
  site's files as they are, with the site's settings.
- Every other setting of little's in an entry stops the build, naming it:
  `theme`, `search`, `markdown`, `mermaid`, `toc_min_depth`,
  `toc_max_depth`, `destination`, `repo`, `branch`, `host`, `port`,
  `year`, and `versions`. Any other key, like `name` or `released`, is for
  the theme, in `site.versions`, as before.
- A folder a plugin reads that isn't at a version's ref stops the build,
  as before, now saying to set its `read` in the version's `plugins`, or
  to turn it off there.

## Consequences
- A release whose nav doesn't fit, or whose code has moved, builds as it
  is, with no commit to its tag.
- Every version is still in the site's theme, with its search, Markdown,
  and the menu, so they all look alike, and an old version is rendered
  with today's settings, not those of its day.
- A setting of little's in an entry, which was quietly for the theme, now
  stops the build, so an author who expects `search: false` for one
  version to count is told it can't, before publishing.
- A theme sees an entry's `nav` and `plugins` in `site.versions`, as it
  sees each key of the entry.
