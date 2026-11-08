---
title: Sass
description: Write a theme's stylesheets in Sass, compiled to CSS as the site is built, with nothing to install.
---

# Sass

Put a `.scss` or `.sass` file in your theme's `_static/`, and little
compiles it to CSS at the same path, with nothing to install or configure:
`_static/assets/style.scss` becomes `/assets/style.css`, which the theme
links to as `{{ '/assets/style.css' | url }}`.

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

A stylesheet loads only from `_sass/`, not from beside it, and a file in
`_sass/` loads from its own folder first, then from `_sass/` itself. A
theme that [builds on](choosing-a-theme.md#building-on-another-theme) this
one can replace `_sass/_variables.scss` with its own, which this one's
stylesheet then loads instead.

A stylesheet that doesn't compile stops the build, and the error names its
file, line, and column. little prints `@warn` and `@debug` messages with
theirs too.

little compiles Sass as Dart Sass 1.54 does, so a newer `if()`, like
Bootstrap 6's `if(sass($a > $b): $a; else: $b)`, stops the build with
`expected ")"`. Write it the older way, `if($a > $b, $a, $b)`.

Sass in the site's source is copied as it is, like any other
[file](../writing/pages-and-files.md#images-and-other-files).
