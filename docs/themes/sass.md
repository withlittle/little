---
title: Sass
description: Write a theme's stylesheets in Sass, compiled to CSS as the site is built, with nothing to install.
---

# Sass

Put a `.scss` or `.sass` file in a theme's `_static/`, and little compiles
it to CSS at the same path. There's nothing to install or configure. So
`_static/assets/style.scss` becomes `/assets/style.css`, and the theme
links to `{{ '/assets/style.css' | url }}`.

What a stylesheet loads, with `@use`, `@forward`, or `@import`, goes in the
theme's `_sass/` folder, which isn't published. To build Bootstrap with
your own colours, say, copy its `scss/` folder into `_sass/` as it is:

```text
my-theme/
├── _sass/
│   ├── _variables.scss   # $primary: #6f42c1;
│   └── bootstrap/        # Bootstrap's scss/ folder
└── _static/
    └── assets/
        └── style.scss    # @import "variables"; @import "bootstrap/bootstrap";
```

- A stylesheet loads only from `_sass/`, never from beside it. A file in
  `_sass/` loads from its own folder first, then from `_sass/` itself.
  Nothing outside `_sass/` can be loaded.
- A theme that [builds on](choosing-a-theme.md#building-on-another-theme)
  this one can replace `_sass/_variables.scss` with its own. Then this
  one's stylesheet loads that instead.
- A partial, a Sass file whose name starts with `_`, in `_static/` stops
  the build. Move it to `_sass/`.
- Two files in one theme that would be published at the same path, like
  `style.scss` and `style.css`, stop the build too.
- A stylesheet that doesn't compile stops the build, naming the file,
  line, and column of the error. `little serve` keeps serving the last good
  build. `@warn` and `@debug` messages name theirs too.
- Sass in the site's source is copied as it is, like any other
  [file](../writing/pages-and-files.md#images-and-other-files).

## Limits

little compiles Sass with [grass](https://github.com/connorskees/grass),
which follows Dart Sass 1.54. So newer Sass, like `color.channel()`, won't
compile, and there are no `pkg:` URLs.

Bootstrap 6's calls to the newer `if()`, like
`if(sass($a > $b): $a; else: $b)`, stop the build with `expected ")"`.
Write each the older way, `if($a > $b, $a, $b)`.
