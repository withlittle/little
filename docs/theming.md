---
title: Theming
---

# Theming is the whole point

Every byte of HTML on this page comes from `theme/templates/page.html`. little passes
the theme **structured data** — never pre-rendered markup — so you can build a theme with
plain HTML, Tailwind, Bootstrap, or anything else. That's the Jekyll superpower, kept.

The theme receives three objects:

- `site` — the merged config
- `page` — `{ title, content, url }`
- `nav` — the list of pages, each with an `active` flag

## The cascading config, made visible

The accent color of this site is declared at **all three levels**, and the *last* one wins:

1. **little** default — `#2563eb` (blue), in `src/config.rs`
2. **theme** override — in `theme/theme.yml`
3. **site** override — in `little.yml`

Open `theme/theme.yml` and `little.yml` and change `accent`. Save. The color updates live —
and if you delete the site's value, the theme's value takes over. That's the cascade.
