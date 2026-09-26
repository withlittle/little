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

- `site.name` — the site title (little's built-in default is `"Documentation"`)
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

The accent color of this site is declared at **all three levels**, and the *last* one wins:

1. **little** default — `#2563eb` (blue), in `src/config.rs`
2. **theme** override — in `theme/little.yml`
3. **site** override — in `little.yml`

Open `theme/little.yml` and `little.yml` and change `accent`. Save. The color updates live —
and if you delete the site's value, the theme's value takes over. That's the cascade.
