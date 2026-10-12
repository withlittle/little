# 0042 — Compiling Sass with Dart Sass

- Status: proposed
- Date: 2026-10-12
- Amends: [0004](./0004-downloading-themes.md),
  [0018](./0018-sass-in-themes.md)

## Context
little compiles a theme's Sass with grass
([0018](./0018-sass-in-themes.md)), which aims at Dart Sass 1.54, from July
2022. Bootstrap 6.0.0-alpha.1, released on 8 October 2026, builds with Dart
Sass 1.103.1 or later. It uses the CSS-style `if()` that Dart Sass 1.95.0
added in December 2025:

```scss
@return if(sass($l1 > $l2): math.div($l1 + .05, $l2 + .05); else: math.div($l2 + .05, $l1 + .05));
```

Bootstrap 6 has 12 of these, in 6 files. grass stops at the first, with
`_functions.scss:162:29: expected ")"`, so a theme built on Bootstrap 6
doesn't compile unless its author edits Bootstrap.

With the 12 calls written in the old form, `if($condition, $a, $b)`, grass
compiles Bootstrap 6. Its CSS has the same 8,523 declarations as Dart Sass's,
and only one is written differently, in a way that means the same. So today,
for Bootstrap's own CSS, the gap is one function. But Dart Sass has
deprecated the old form, and the gap grows with each release that libraries
follow:

- 1.79 added colour spaces, and functions like `color.channel()`, which
  grass doesn't have, though Bootstrap 6's `luminance()` calls it. It also
  stopped rounding a colour's channels.
- 1.92 writes declarations in the order of the source, even after nested
  rules.
- 1.95 added the CSS-style `if()`.
- 1.105 added first-class modules, with `meta.load()`.

grass's last release, 0.13.4, was in August 2024, and its code hasn't changed
since September 2024. 0018 left Dart Sass out because every author would have
to install it. It also noted that themes are plain Sass, so another compiler
could take grass's place without changing them.

## Options considered
- **What compiles Sass:**
  - grass, in a fork of little's with the new `if()`. It's the smallest
    change, and Sass stays in little's process, offline. But little would
    own a Sass compiler, and would have to port each feature a theme needs.
  - [rsass](https://crates.io/crates/rsass) still passes fewer than half of
    the Sass spec's tests (5,988 of 13,041), and has no new `if()`.
  - [Dart Sass](https://sass-lang.com/dart-sass/), the reference
    implementation, which Bootstrap builds with. It's a Dart program. Its
    JavaScript build needs Node, which little can't require. Dart compiled
    to WebAssembly needs a JavaScript host, so it can't run inside little, as
    a plugin does.
- **How little gets Dart Sass:**
  - authors install it, as [Hugo](https://gohugo.io/functions/css/sass/)
    asks them to. 0018 turned this down. Each machine would also have its
    own version, so a site's CSS could differ from one machine to the next.
  - in little's binary, as the
    [sass-embedded](https://rubygems.org/gems/sass-embedded) gem, which
    Jekyll uses, ships it in a gem for each platform. Every copy of little
    would carry 4–5 MB more, even for a site whose theme has no Sass, like
    the built-in theme. Building little would need the network.
  - downloaded by the first build that needs it, as a theme or a plugin is
    ([0004](./0004-downloading-themes.md),
    [0029](./0029-plugins-that-render-code-blocks.md)). Only sites whose
    theme has Sass download it.
- **How little talks to Dart Sass:**
  - through its command line, with `_sass/` as a load path. But a load path
    doesn't stop `@use "/home/…"` or a `file:` URL, so a downloaded theme
    could read any file on disk, which 0018 forbids.
  - through its
    [embedded protocol](https://github.com/sass/sass/blob/main/spec/embedded-protocol.md),
    `sass --embedded`, which Hugo and the sass-embedded gem use. The
    messages are protocol buffers, over standard input and output. Dart Sass
    asks the host to find and load each file a stylesheet loads. It reports
    each error and warning with its file, line, and column.
- **Deprecation warnings:** Dart Sass warns of Sass it will remove, which
  grass never did. Bootstrap 5.3.8, set up as the manual shows, sets off 332
  deprecation warnings on every build. Dart Sass shows 20 of them, in 279
  lines, and counts the rest. Its `--quiet-deps` keeps quiet those from
  files loaded through a load path. That leaves 2, for the stylesheet's own
  `@import`s.

## Decision
- little compiles a theme's stylesheets with Dart Sass, not grass. The rest
  of 0018 stands:
  - which files are stylesheets;
  - the `_sass/` folder and its layers;
  - nothing outside `_sass/` can be loaded;
  - the CSS is expanded, with no source maps;
  - an error stops the build, naming its file, line, and column, and
    `little serve` keeps the last good build.
- little pins a version of Dart Sass, 1.105.1 to begin with. It also pins
  the SHA-256 of the release for each platform, which GitHub lists beside
  it. A release of little can move to a newer version, and the manual names
  the one it uses.
- The first build that compiles a stylesheet downloads the release for the
  platform little was built for, like `dart-sass-1.105.1-macos-arm64.tar.gz`,
  of about 4–5 MB, from Dart Sass's releases on GitHub. little checks its
  SHA-256 and unpacks it into a folder named for the release, in the site's
  `.little/sass/`, as it does a theme. It never downloads that release
  again. A download that doesn't match its SHA-256 stops the build and isn't
  kept. A site whose theme has no Sass downloads nothing.
- little starts Dart Sass once in a build, with `--embedded`, and compiles
  each stylesheet with it. `little serve` keeps it running between builds,
  and starts it again if it stops.
- little answers every load itself. Its importer has the layered files of
  `_sass/` and nothing else, at URLs of its own, like
  `little:/bootstrap/scss/_functions.scss`. Dart Sass gets no load paths,
  so a stylesheet loads nothing except through little.
- Dart Sass's deprecation warnings from files in `_sass/`, where libraries
  like Bootstrap go, are kept quiet, as `--quiet-deps` keeps quiet those
  from a load path. Those from a stylesheet in `_static/` are shown, and so
  are `@warn` and `@debug`, from any file. To do this, the stylesheet's own
  importer finds nothing, so each of its loads falls through to little's
  importer for `_sass/`, which Dart Sass then counts as a dependency.
- Each error, warning, and `@debug` message names the real file, line, and
  column, in whichever theme has the file. A `@warn` comes with a stack
  trace, not a span, so little takes its file, line, and column from the
  trace's first frame. As now, a warning doesn't fail `--strict`.
- New crate: `prost`, for the protocol's messages. Its code is generated from
  Sass's `embedded_sass.proto` and kept in little's source, so building
  little doesn't need `protoc`. `grass` and `codemap` are removed.

## Consequences
- Bootstrap 6.0.0-alpha.1 compiles untouched, with no warnings, to the same
  CSS as Dart Sass's own command line writes, and a theme can use anything
  in Sass up to 1.105.1.
- A theme's CSS changes where Dart Sass has changed since 1.54. Bootstrap
  5.3.8, which builds its own CSS with Dart Sass 1.78.0, keeps all 5,372
  declarations, but 128 values change:
  - 123 colours are written as `rgb()` with fractional percentages, not hex,
    because Dart Sass 1.79 doesn't round a colour's channels. No channel is
    more than 0.86 of 255 from grass's.
  - 4 SVG `data:` URLs hold colours like these, with a `%` that isn't
    escaped. The URL standard keeps it as it is, and Chrome and WebKit draw
    them as before.
  - One channel of `--bs-link-hover-color-rgb` is 164, not 165.
  - Declarations written after nested rules stay after them, where grass
    moved them up (Dart Sass 1.92).
- The manual's Bootstrap example uses `@import`, so it would warn twice on
  every build. It would show `@forward` instead, with
  `@forward "bootstrap/bootstrap" with ($primary: #6f42c1);` in
  `_sass/_variables.scss`, and `@use "variables";` in the stylesheet. That
  gives the same CSS, byte for byte, with no warnings, and a theme that
  builds on it can still replace `_variables.scss`.
- A site whose theme has Sass needs the network for its first build. It
  needs it again when little moves to a newer Dart Sass. Offline, the build
  stops, saying what it couldn't download. Each site keeps its own copy,
  about 10 MB unpacked. A CI run downloads it unless it keeps `.little/`.
- little runs a program it downloaded, outside any sandbox, which it never
  did before. The pinned SHA-256 means it runs only the release it was
  built for, and its importer means a stylesheet loads only what's in
  `_sass/`. Tried over the protocol with Dart Sass 1.105.1, a stylesheet
  loaded nothing through `..`, an absolute path, a `file:` or `pkg:` URL,
  `meta.load-css()`, `meta.load()`, the current folder, or `SASS_PATH`.
- Builds take about as long. Once Dart Sass is running, it compiles
  Bootstrap 5 or 6 in 130–140 ms, where grass takes 100–150 ms. Starting it
  adds 30–60 ms to a build, and `little serve` starts it once.
- little's binary loses grass, about 3 MB by 0018's count, and gains prost.
  little's own tests need Dart Sass too, so CI downloads it.
- Dart Sass is under the MIT licence, and the Dart runtime in its release is
  under BSD-3-Clause. little doesn't ship them. Each download carries its
  own licences.
- Dart Sass 3.0 will remove `@import` and Sass's global functions, which
  Bootstrap 5 uses. little stays on 1.x until a decision of its own moves
  it.
- Limits, for now:
  - there's no Sass on a platform Dart Sass has no release for, like
    FreeBSD, or on a machine that can't reach GitHub, and no setting to use
    a `sass` that's already installed;
  - a theme's author doesn't see deprecation warnings from their own files
    in `_sass/`, only from their stylesheets;
  - there are still no `pkg:` URLs or source maps.
