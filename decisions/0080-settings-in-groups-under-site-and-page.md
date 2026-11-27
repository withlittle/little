# 0080 — Settings in groups, under `site` and `page`

- Status: accepted; amended by
  [0082](./0082-the-dates-of-a-site-and-its-pages.md), which has
  `site.built`, the time of the build, in place of `site.year`, and by
  [0086](./0086-the-built-in-themes-design-system.md), which has one colour
  for each of `site.theme.colors`, not a light and a dark, and adds
  `site.theme.radius`, and by
  [0087](./0087-each-pages-file-copied-beside-it.md), which gives every
  page a `source`, on in little's `little.yml`, like its other groups
- Date: 2026-11-18
- Amends: [0002](./0002-theme-structure.md),
  [0003](./0003-choosing-and-layering-themes.md),
  [0004](./0004-downloading-themes.md),
  [0005](./0005-previous-and-next-pages.md),
  [0006](./0006-frontmatter-and-config-win.md),
  [0007](./0007-page-nav-shaped-like-site-nav.md),
  [0008](./0008-page-toc.md),
  [0009](./0009-toc-depth-in-config.md),
  [0010](./0010-breadcrumbs.md),
  [0011](./0011-serve-host-and-port-in-config.md),
  [0019](./0019-site-search.md),
  [0020](./0020-links-to-a-pages-file-in-its-repo.md),
  [0028](./0028-how-diagrams-look.md),
  [0029](./0029-plugins-that-render-code-blocks.md),
  [0042](./0042-versions-share-the-sites-config-and-theme-files.md),
  [0043](./0043-a-versions-own-nav-and-plugins.md),
  [0046](./0046-plugin-hooks.md),
  [0057](./0057-starting-a-site-with-little-init.md),
  [0061](./0061-leaving-pages-out-of-search.md),
  [0074](./0074-pruning-the-nav.md)

## Context
A `little.yml` is one flat list of settings, and each new one adds to it.
Those that belong together share a prefix, like `toc_min_depth` and
`toc_max_depth` ([0009](./0009-toc-depth-in-config.md)). `nav_prune` is
flat because `nav` is a list, which `prune` can't go in
([0074](./0074-pruning-the-nav.md)). Some names are short for others,
like `nav`, `repo`, `lang`, and `prev`, and some settings are written more
than one way:

- `search` is `true`, `false`, or its settings, which turn it on
  ([0061](./0061-leaving-pages-out-of-search.md));
- a plugin is its `.wasm` file's path, its settings, or `false`
  ([0029](./0029-plugins-that-render-code-blocks.md));
- `theme` is `true`, `false`, a path, or a URL
  ([0003](./0003-choosing-and-layering-themes.md),
  [0004](./0004-downloading-themes.md));
- a page turns off what little works out for it with `false` in its
  place, like `toc: false`
  ([0006](./0006-frontmatter-and-config-win.md)).

So a name can't be guessed from the ones beside it, by a site's author or
a theme's, whose templates get the same names, as `site.<key>`.

Only frontmatter sets a page's keys, a page at a time, so a site can't
turn breadcrumbs off for every page. And the headings a table of contents
lists are the site's to choose, for every page: a page can't choose its
own.

## Options considered
- **Flat names or groups:**
  - flat, with a prefix for those that belong together, as MkDocs has
    `site_name`, `site_url`, and `repo_url`: less to write, but each new
    setting is another name at the top of the file, and only their
    spelling says which belong together;
  - or groups, like `serve: { host, port }`, as Docusaurus has
    `navbar: { items }`: a line and an indent more, but a group's name
    says what its settings are about, and a new one has a place to go.
- **Where a page's settings go:** at the top, beside the site's, as
  `toc_max_depth` is; or in a group of their own, `page`, with the site's
  in `site`, named for what templates get. Then a `little.yml` reads as a
  template does, and `page` can hold any key frontmatter can, as every
  page's default.
- **A list with settings:** a list under its own name, like `nav`, with
  its settings beside it, under a prefix; or a group that holds the list
  as `items`, like `navigation: { prune, items }`, so a theme writes
  `site.navigation.items`, `page.outline.items`, and
  `page.breadcrumb.items` alike.
- **Short forms:** kept, as `search: true` and `csv: ./csv.wasm` are less
  to write; or one way to write each setting, so nobody learns two, and a
  template tests one thing, where `{% if site.search %}` had to be true
  of `true` and of settings.
- **Turning something off:** `false` in place of its value, which leaves
  its other settings nowhere to go; or `enabled`, beside them.
- **Who acts on a page's `enabled`:** each theme, which tests it beside
  the data, so a theme that forgets shows what a page turned off; or
  little, which leaves the data out, so a theme tests only what it's
  given, as it did when `toc: false` took the list's place.
- **Names:** short, like `nav` and `repo`; or whole words, which nobody
  has to learn. A table of contents has no one word, so it takes the name
  editors give their list of a file's headings, its outline.
- **How diagrams look:** `mermaid`, at the top; or in `site.theme`, as
  it's a look, which a theme sets and a site changes
  ([0028](./0028-how-diagrams-look.md)).
- **A group a page hasn't got:** MiniJinja stops at a key of something
  that isn't there, like `page.outline.items` on a page with no
  `page.outline`. A theme could test each group first; MiniJinja could
  pass over them, which would hide a misspelt name at any depth; or
  little could give every page each group.
- **Names and forms from before:** read, beside the new, which would be
  the two ways to write a setting this record removes; or not read, as
  nobody has a site to move yet.

## Decision
- A `little.yml` holds two groups, `site` and `page`, and nothing else: a
  key beside them stops the build, which says to put it in one of them.
  - `site` holds the site's settings: little's, a theme's, and a site's
    own, which templates get as `site`, as before.
  - `page` holds settings for every page. Each page's frontmatter goes
    over it, key by key, as a `little.yml` goes over the one beneath it,
    so a page's cascade is little → theme → site → the page, and a key
    under `page` does what it would in every page's frontmatter. A key
    left blank there is ignored, as it is in frontmatter.
  - What little works out for a page stays beneath both, so a page's
    settings have the last word on what its theme gets, as frontmatter
    had ([0006](./0006-frontmatter-and-config-win.md)). Frontmatter is
    written as before, with no `page:` around its keys.

  ```yaml
  site:
    name: My Project
    repository:
      url: https://github.com/example/my-project
    navigation:
      prune: true
    plugins:
      python:
        wasm: ./plugins/little_python.wasm
        read: src
  page:
    outline:
      depth:
        max: 4
    breadcrumb:
      enabled: false
  ```
- Settings are named by four rules, from now on:
  - Those that belong together are a group, like `serve`, not names with
    a prefix.
  - A name is whole words, like `navigation`, `repository`, `language`,
    and `previous`. `url`, `min`, `max`, `ref`, and `wasm` stay, as
    nobody writes them out.
  - A group with a singular name holds its list as `items`, beside its
    settings. A plural name, like `children` and `versions`, is the list
    itself.
  - Each setting is written one way. What can be turned off has
    `enabled`, `true` or `false`, in its group, and no `true`, `false`,
    or other value stands in for a group. A setting that chooses among
    values keeps `false` among them, like `markdown.math`.
- In a `little.yml`, and so in what templates get as `site`:

  | Before | Now |
  | ------ | --- |
  | `name`, `description`, `url`, `source`, `destination`, `markdown`, `plugins`, `versions`, `year`, and a site's own keys | the same, under `site` |
  | `lang` | `site.language` |
  | `host`, `port` | `site.serve.host`, `site.serve.port` |
  | `repo`, `branch` | `site.repository.url`, `site.repository.branch` |
  | `nav` | `site.navigation.items` |
  | `nav_prune` | `site.navigation.prune` |
  | `search`, `true` or `false` | `site.search.enabled` |
  | `search.exclude` | `site.search.exclude` |
  | `theme`, `true` or `false` | `site.theme.enabled` |
  | `theme`, a path or a URL | `site.theme.source` |
  | `mermaid` | `site.theme.mermaid` |
  | `font`, in the built-in theme | `site.theme.fonts.body` |
  | a plugin's path alone, like `csv: ./csv.wasm` | its `wasm` |
  | a plugin that's `false` | its `enabled: false` |
  | `toc_min_depth`, `toc_max_depth` | `page.outline.depth.min`, `page.outline.depth.max` |
  | `nav`, in a version's entry | its `navigation.items` |

- In frontmatter, and in what templates get as `page`:

  | Before | Now |
  | ------ | --- |
  | `page.toc` | `page.outline.items` |
  | `page.breadcrumbs` | `page.breadcrumb.items` |
  | `page.prev` | `page.previous` |
  | `page.source_url`, `page.edit_url` | `page.repository.source_url`, `page.repository.edit_url` |
  | `toc: false` | `outline: { enabled: false }` |
  | `breadcrumbs: false` | `breadcrumb: { enabled: false }` |
  | `prev: false`, `next: false` | `previous: { enabled: false }`, `next: { enabled: false }` |
  | `edit_url: false` | `repository: { enabled: false }`, which turns off both links |
  | `search: false` | `search: { enabled: false }` |

- little's `little.yml` turns on each page's `outline`, `breadcrumb`,
  `previous`, `next`, and `repository`, and sets `outline.depth` to 2 and
  6. The built-in theme's sets `page.outline.depth.max` to 3. A page can
  now choose its own depth, in its frontmatter.
- little acts on a page's `enabled`, so a theme tests only what it's
  given, and a page turns something off whatever its theme. Turned off,
  `outline` and `breadcrumb` have no `items`, `previous` and `next` are
  left out, as on the first and last pages, and `repository` has neither
  URL, whatever else the page's settings write in them.
- `page.search.enabled` is whether the page is in search. little works it
  out from `site.search.exclude`, and a page's own has the last word, as
  before. `exclude` no longer turns search on: only `site.search.enabled`
  does.
- `site.theme.enabled` and `site.theme.source` choose the theme, as
  `theme` did: little sets `enabled: true`, for the built-in theme, and
  `source` is another's path or URL. `enabled: false` is no theme,
  whatever `source` is. In a theme's own `little.yml` they choose the
  theme it builds on, and `enabled: true` alone is the built-in one. What
  templates get, and `little config` prints, is the site's choice, not
  its theme's. The rest of `site.theme` is how the site looks, which
  cascades like any setting: `mermaid`, and `fonts.body`, the built-in
  theme's font.
- A plugin's settings are `wasm`, its file; `read`, which only the site's
  `little.yml` can set, as before; `enabled`; and its own. little gives a
  plugin its settings without `wasm` and `enabled`.
- little works out `site.repository.owner` and `site.repository.name`
  from its `url`: the last part of its path is the `name`, and what's
  before it the `owner`. A `little.yml` that sets either wins.
- `site.error` holds the 404 page's `title` and `description`, which
  little sets, and a theme or a site can change. The 404 page gets them
  as `page.title` and `page.description`, and `page`'s settings too, so
  each group is there for its template, with nothing in its `items`.
- A version's entry with a `ref` sets its own `source`,
  `navigation.items`, and `plugins`, and still none of the site's other
  settings. Of the `little.yml` at its ref, only `site.source` and
  `site.navigation.items` count.
- A plugin's `config` hook gets both groups, and can't change
  `site.theme.enabled`, `site.theme.source`, `site.plugins`,
  `site.source`, `site.destination`, or `site.serve`. Its `site` and
  `page` hooks get what templates do: little acts on a page's `enabled`
  before its `page` hook, and of what the hook gives back, reads only
  `search.enabled`. In `wit/plugin.wit`, only comments change, so
  `little:plugin` stays 0.2.1.
- `little config` prints both groups, and `little init` writes
  `site.name`, with `description`, `url`, and `repository.url` commented
  out.
- little reads no name or form from before. A value where a group goes,
  like `search: true` or `theme: ./theme`, stops the build, and the error
  shows what to write.
- No new crates.

## Consequences
- Every `little.yml` and every theme changes, and so does frontmatter
  that turns something off, so little's minor version rises. little's
  plugins needn't be released again: they export only `setup`, `files`,
  and `formats`, which get none of it. A plugin that exports `config`,
  `site`, or `page` reads the new names.
- A site sets a default for every page under `page`, like
  `breadcrumb: { enabled: false }`, and so can a theme, for its own keys
  too, like `description`.
- A new setting has a group to go in, or starts one, and the rules give
  it its name.
- A `little.yml` is longer: it starts with `site:`, and each setting is
  indented under it.
- A key little doesn't know, under `site` or `page`, is still for the
  theme. So a misspelt setting does nothing, as before, and nor does one
  written there by its old name, and nothing says so.
- A `little.yml` at a version's ref isn't checked, and one from before
  this record sets neither `site.source` nor `site.navigation.items`. Its
  version has little's `source`, and a nav from its folders, unless its
  entry sets them.
- Limits, for now:
  - the built-in theme reads only `fonts.body` of the look `site.theme`
    is to hold: a `logo`, with a `light` and a `dark`; `fonts`, for
    `heading`, `code`, and `math` too; and `colors`, `light` and `dark`,
    each with `background`, `foreground`, `muted`, `primary`,
    `secondary`, `accent`, `success`, `danger`, `warning`, and `info`;
  - only a plugin's `read` is the site's alone: a theme can still set
    `site.source`, `site.destination`, and `site.versions`, which choose
    what little reads and writes on the site's disk;
  - every diagram still has one look, in light and dark, as
    [0028](./0028-how-diagrams-look.md) left it.
