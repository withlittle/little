# 0003 — Choosing themes, and themes that build on others

- Status: accepted
- Date: 2026-09-28
- Amends: [0002](./0002-theme-structure.md)

## Context
A site that didn't set `theme` used the theme in its own `theme/` folder, so
every new site had to copy one in before it could build. A theme's
`little.yml` couldn't set `theme`, yet a company wants one theme for all its
sites that only changes another theme's fonts and colours, without forking it.
And there was no way to see the merged config without building the site.

## Options considered
- **The default theme:** the site's `theme/` folder, or a theme compiled into
  little. Compiled in, a site needs nothing but its Markdown, and little is
  still one file.
- **What `theme` can be:** a folder only, or also `true` and `false` for the
  built-in theme or none, and later a URL to download a theme from.
- **A theme that sets `theme`:** an error, or building on that theme, its
  settings over the other's. Building on it is what the company theme needs.
- **The files of a theme that builds on another:** only its own, or layered
  over the other theme's, file by file, as Hugo layers a site over its themes.
  Layered, a theme holds only what it changes.

## Decision
- `theme` is `true` (little's default: the built-in theme), `false` (no
  theme), or the path to a theme's folder, relative to the folder of the
  `little.yml` that sets it. Anything else is an error. URLs come next; for
  now they're an error too.
- The built-in theme is this repo's `theme/` folder, compiled into little. It
  builds on nothing.
- A theme's `little.yml` can set `theme`, to build on another theme. The
  cascade becomes little → the base theme → … → the site's theme → site.
  Themes that build on each other in a loop are an error.
- Templates and static files come from the site's theme first, then from each
  theme it builds on, file by file. So a theme that builds on another needs no
  `_templates/` of its own, as long as one beneath it has `default.html`.
- `little config` prints the merged config, so the cascade can be checked
  without a build.

## Consequences
- A site is just its `docs/`, and a `theme/` folder in it is no longer
  special: a site that relied on one must set `theme: ./theme`.
- The built-in theme's templates ship inside the binary, but they're still a
  theme, not core code: the core emits no HTML, and a site can choose another
  theme.
- Every file in `theme/` must be listed in `src/theme.rs`; a test checks.
- A template can't extend the one it replaces, since it would extend itself.
  A theme replaces smaller templates that its base includes instead.
- `little build` with `theme: false` is an error until we decide what a site
  without a theme builds.
- `theme` in `little config` is always the site's choice: a theme's `theme`
  only chooses what that theme builds on.
