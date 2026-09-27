---
title: Theming
---

# Theming is the whole point

Every byte of HTML on this page comes from `theme/templates/page.html`. little passes
the theme **structured data** — never pre-rendered markup — so you can build a theme with
plain HTML, Tailwind, Bootstrap, or anything else. That's the Jekyll superpower, kept.

The theme receives two structured objects:

### `site`

The merged config (little → theme → site), plus a derived nav:

- `site.name` — the site title (this theme defaults it to `"Documentation"`)
- `site.nav` — the main navigation, derived from the files and folders under `docs/`.
  Each item is `{ title, url, children }`. A folder with a `README.md` links to that
  landing page; a folder without one is a plain section label (`url` is null).
  `site.nav` is shared by every page, so the theme marks the current item by comparing
  `item.url` with `page.url`.

### `page`

- `page.title` — frontmatter `title`, else the first `<h1>`, else a name derived from the file
- `page.url` — the page's clean, extensionless URL (`/`, `/getting-started`, `/guide/install`)
- `page.description` — frontmatter `description`, or null
- `page.content` — the rendered Markdown (HTML)
- `page.nav` — an in-page table of contents built from the page's `h2`–`h3` headings;
  each item is `{ title, anchor, depth }`, where `anchor` is a `#fragment` link

## The cascading config, made visible

Config cascades through three levels — **little** → **theme** → **site** — and the *last*
writer wins. Each level is a `little.yml`. little's own is built into little (it's
`src/little.yml` in little's source), and it leaves a site's name to the theme and the site,
so the name of this site is declared by the other two:

1. **theme** default — in `theme/little.yml`
2. **site** override — in `little.yml`

Open `theme/little.yml` and `little.yml` and change `name`. Save. The header updates live —
and if you delete the site's value, the theme's value takes over. That's the cascade.

Both files are optional. A theme or site with nothing to set can leave out its
`little.yml` (an empty one is fine too), and the levels before it still apply. Because
little doesn't default a site's name, language, or look, a theme without a `little.yml`
should give its templates their own fallbacks, e.g. `{{ site.lang | default('en') }}`.

## Bring your own theme

A theme is just a folder: `templates/page.html`, plus an optional `assets/` folder and
`little.yml`. This site uses the one in `theme/`. To use a theme kept somewhere else, point
`theme` at its folder in the site's `little.yml`:

```yaml
theme: ./path/to/my/theme
```

The path starts with `./`, `../`, or `/` and is relative to the site's folder. Only the
site chooses its theme, so a theme's own `little.yml` can't set `theme`. `little serve`
watches whichever theme the site uses, so theme edits reload live too.
