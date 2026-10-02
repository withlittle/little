# 0018 — Sass in themes

- Status: accepted
- Date: 2026-10-02
- Amends: [0002](./0002-theme-structure.md),
  [0003](./0003-choosing-and-layering-themes.md)

## Context
Many themes are written in [Sass](https://sass-lang.com/), as Bootstrap and
Bulma are. little copies a theme's `_static/` as-is, so a `.scss` file there
is published as it is, and no browser reads it. A theme has to ship its Sass
compiled, as [theme-bootstrap](https://github.com/withlittle/theme-bootstrap)
ships Bootstrap's CSS. To change Bootstrap's colours, its author has to
compile Bootstrap again with another tool, and a company theme that builds on
it can't change them at all, only add CSS over them.

## Options considered
- **What compiles Sass:**
  - [grass](https://crates.io/crates/grass), in Rust, which Zola uses. It
    passes about 90% of the Sass spec's tests, aims at Dart Sass 1.54, and
    compiles Bootstrap 5 untouched, but it's had no release since August
    2024.
  - [rsass](https://crates.io/crates/rsass), also in Rust, is maintained, but
    passes fewer than half of the spec's tests.
  - Dart Sass itself would be another program for every author to install.
- **Whose Sass:** the theme's only, or the site's in `docs/` too. A theme is
  where a site's look is, and a `.scss` file in `docs/` is as likely to be an
  example to show as one to compile.
- **Where a theme's Sass goes:** all of it beside the stylesheets in
  `_static/`, partials and all, or the stylesheets in `_static/` and what they
  load in a folder of its own, `_sass/`, as in Jekyll. `_sass/` keeps
  `_static/` to what's published, takes a library like Bootstrap whole, and
  layers like `_templates/`. But an editor needs `_sass/` as a load path to
  follow a stylesheet's loads.
- **What a stylesheet can load:** from `_sass/` and from beside itself, or
  only from `_sass/`. Only `_sass/` gives one place to look, and one way for a
  theme that builds on another to replace what's loaded.

## Decision
- A `.scss` or `.sass` file in a theme's `_static/` is a stylesheet. little
  compiles it to the CSS file at its path: `_static/assets/style.scss` is
  published as `/assets/style.css`, which the theme links to as
  `{{ '/assets/style.css' | url }}`.
- What a stylesheet loads, with `@use`, `@forward`, or `@import`, comes from
  the theme's `_sass/` folder, which isn't published:
  - a stylesheet loads only from `_sass/`, never from beside itself, and a
    Sass partial in `_static/`, like `_colors.scss`, stops the build, saying
    to move it to `_sass/`;
  - a file in `_sass/` loads from its own folder first, then from the root of
    `_sass/`, as Sass does with `_sass/` as a load path;
  - nothing outside `_sass/` can be loaded, with `..` or by an absolute path,
    so a downloaded theme can't read other files on disk.
- The files in `_sass/` are layered like templates: each comes from the
  nearest theme that has it. So a theme that builds on another can replace
  one, like `_sass/_variables.scss`, and the other theme's stylesheets load
  the replacement. A file can't load the one it replaces, since it would load
  itself.
- A theme's static files go over those of the theme it builds on by the path
  they're published at, so a stylesheet replaces the CSS file it would be
  compiled to, and a CSS file replaces a stylesheet. Two files in one theme
  that would be published at the same path, like `style.scss` and
  `style.css`, stop the build, naming both. A file in the site at that path
  still has the last word ([0015](./0015-files-and-relative-links.md)).
- Sass in the site's source, in `docs/`, is copied as-is, like any other
  file.
- The CSS is expanded, as Sass writes it by default, with no source maps.
  There's nothing to configure.
- A stylesheet that doesn't compile stops the build before it touches
  `dist/`, naming the stylesheet and the file, line, and column of the error,
  and `little serve` keeps serving the last good build. Warnings and `@debug`
  messages name their file, line, and column too.
- New crates: `grass`, without its command line, and `codemap`, which grass
  already uses, to name where a warning is.

## Consequences
- A theme can be written in Sass, with nothing else to install, and can
  compile a library like Bootstrap with its own variables. A company theme
  that builds on it changes its colours with one file in `_sass/`.
- little grows from about 22.7 MB to about 25.7 MB, and by 9 crates.
- The built-in theme stays plain CSS. A theme that builds on it can replace
  its `assets/style.css` with an `assets/style.scss`.
- grass is behind Dart Sass, and has slowed down: Sass newer than 1.54, like
  `color.channel()`, doesn't compile, and fixes may not come. Themes are
  plain Sass, though, so another compiler could take grass's place without
  changing them.
- Limits, for now:
  - a theme's stylesheets are compiled even when a file in the site replaces
    their CSS, so a site can't get round one that grass can't compile, though
    a theme that builds on it can;
  - there are no `pkg:` URLs, so a library goes into `_sass/`, not in from
    npm.
