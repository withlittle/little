---
title: Theming
---

# Theming is the whole point

Every byte of HTML on this page comes from the theme's templates, here
`theme/_templates/default.html`. little passes the theme **structured data** — never
pre-rendered markup — so you can build a theme with plain HTML, Tailwind, Bootstrap, or
anything else. That's the Jekyll superpower, kept.

The theme receives two structured objects:

### `site`

The merged config (little → theme → site), plus a derived nav:

- `site.name` — the site title (this theme defaults it to `"Documentation"`)
- `site.nav` — the main navigation, derived from the files and folders under `docs/`.
  Each item is `{ title, url, children }`. A folder with a `README.md` links to that
  landing page; a folder without one is a plain section label, with no `url`.
  `site.nav` is shared by every page, so the theme marks the current item by comparing
  `item.url` with `page.url`. A `nav` in a `little.yml` replaces it, to order the
  sidebar by hand.

### `page`

- `page.type` — the type of page: `home`, `section`, `page`, or `404` (see below)
- `page.url` — the page's clean, extensionless URL (`/`, `/getting-started`, `/guide/install`)
- `page.title` — frontmatter `title`, else the first `<h1>`, else a name derived from the file
- `page.content` — the rendered Markdown (HTML)
- `page.toc` — an in-page table of contents built from the page's headings, from level
  `toc_min_depth` to `toc_max_depth`: `h2`–`h6`, as little sets them, but `h2`–`h3` with
  this theme, whose `little.yml` sets `toc_max_depth: 3`. Your `little.yml` has the last
  word. It's shaped like `site.nav`: each item is `{ title, url, children }`, where `url`
  is a `#fragment` link, and each heading is a child of the nearest one above it of a
  higher rank, like an `h3` of the `h2` above it. This theme lists it with a recursive
  loop, as it does its sidebar, and checks `{% if page.toc %}` first: a page without
  headings has none, and a page can turn it off with `toc: false`
- `page.children` — on home and section pages, what else is in the page's folder, shaped
  like `site.nav`: handy for listing a section's pages
- `page.prev` and `page.next` — the pages before and after this one in `site.nav`, read
  from top to bottom, so they follow the sidebar. Each is `{ title, url }`. The first page
  has no `page.prev`, and the last has no `page.next`. This theme links to them at the foot
  of each page
- `page.<key>` — every other key in the page's frontmatter, as written, like
  `page.description`, which this theme uses for the page's meta description

The page's frontmatter has the last word on `page`, even over what little works out: it
merges over it the way a `little.yml` merges over the level beneath, so
`prev: { title: Start here }` renames the previous page's link, `next: false` removes the
next one, and `toc: false` removes the table of contents. It changes only what the theme
sees: `url` doesn't move the page, and `type` doesn't choose its template. A key left blank
is ignored.

A page leaves out what it doesn't have, rather than setting it to null, so
`{% if page.description %}` and `{{ page.description | default("…") }}` both work.

## A template for each type of page

little renders each page with the theme's template for its type, or with `default.html`
if the theme doesn't have one:

- `home.html` — the site's home page, `docs/README.md`
- `section.html` — a folder's home page, like `docs/guide/README.md`
- `page.html` — every other page, like this one
- `404.html` — the page shown for a URL with no page. It has no content, just `page.type`
  and `page.url`, so a theme without this template gets no 404 page.

This theme has just `default.html` and `404.html`. Its `404.html` extends `default.html`
and replaces only the title and content; templates can extend and include any others in
`_templates/`, in subfolders too. Try it: visit a [page that isn't here](/no-such-page).

## The cascading config, made visible

Config cascades through three levels — **little** → **theme** → **site** — and the *last*
writer wins. Each level is a `little.yml`. little's own is built into little (it's
`src/little.yml` in little's source), and it leaves a site's name to the theme and the site,
so the name of this site is declared by the other two:

1. **theme** default — in `theme/little.yml`
2. **site** override — in `little.yml`

Open `theme/little.yml` and `little.yml` and change `name`. Save. The header updates live —
and if you delete the site's value, the theme's value takes over. That's the cascade. To see
the merged result without building, run `little config`.

Both files are optional. A theme or site with nothing to set can leave out its
`little.yml` (an empty one is fine too), and the levels before it still apply. Because
little doesn't default a site's name, language, or look, a theme without a `little.yml`
should give its templates their own fallbacks, e.g. `{{ site.lang | default('en') }}`.

## Bring your own theme

A theme is just a folder:

```text
my-theme/
├── _static/      optional: copied as-is to the site's root
├── _templates/   the templates; default.html is required
└── little.yml    optional: the theme's config
```

This theme keeps its stylesheet at `_static/assets/style.css`, so its pages link to
`/assets/style.css`. It's little's built-in theme, compiled into little from `theme/`, so a
site that doesn't set `theme` gets it. This site sets `theme: ./theme` to use the folder
instead, so edits to it reload live. To use a theme kept somewhere else, point `theme` at its
folder in the site's `little.yml`:

```yaml
theme: ./path/to/my/theme
```

The path is relative to the site's folder, or absolute, like `/Users/you/my-theme` on macOS
or `C:\Users\you\my-theme` on Windows. (Leave a Windows path unquoted or use single quotes:
inside double quotes, YAML reads `\` as an escape.) `little serve` watches the theme's
folder, so theme edits reload live too.

Or point `theme` at the URL of a theme's `.zip` or `.tar.gz` archive, like a tag's archive
on GitHub:

```yaml
theme: https://github.com/withlittle/theme-bootstrap/archive/refs/tags/0.0.1.tar.gz
```

The first build downloads it into the site's `.little/` folder, which git ignores, and later
builds use that copy, so they work offline. little never downloads the same URL again: change
the URL for another version, or delete `.little/` to download it afresh. A downloaded theme
never changes, so `little serve` doesn't watch it.

A theme's own `little.yml` can set `theme` too, to build on another theme: a path is relative
to the theme's folder, and a URL works just as it does for a site. Its settings go over that
theme's, and so do its templates and static files, file by file, so a company theme that only
changes some colours can be just a `little.yml` that builds on a downloaded theme.
