---
title: Commands
description: little's commands, build, serve, and config, and their options.
---

# Commands

```sh
little build                         # render the site into dist/, and warn of broken links
little build --strict                # fail on anything it would warn of, as for CI
little serve                         # dev server with live reload at http://127.0.0.1:8080
little serve --port 3000             # serve on another port, just this once
little config                        # print the site's config, merged
little -C path/to/site build         # work on a site in another directory
little --help                        # list every command and option
```

Every command works on the site in the current folder, the one holding its
`little.yml`, unless `-C` (or `--dir`) names another. Each reads the site's
[config](../getting-started/configuring.md) first, and stops, saying why, if
a setting is wrong, even one that command doesn't use. `little --version`
prints little's version.

## little build

`little build` builds the site from its source into its destination,
`dist/` unless you [choose
another](../getting-started/configuring.md#choosing-folders), and prints how
many pages it built:

```text
built 12 page(s) -> ./dist
```

- It wipes and regenerates the destination every time, so don't keep
  anything there by hand. A build that fails leaves the last one in place.
- Once it's built the site, it [checks every
  link](../writing/links-and-images.md#checking-links) in it, and warns of
  each one that's broken, of each formula it [couldn't
  render](../writing/math.md#choosing-how-math-is-written), of each diagram
  it [couldn't draw](../writing/diagrams.md#choosing-how-diagrams-are-drawn),
  of each code block a [plugin](../plugins/using-plugins.md) couldn't
  render, and of each warning from a plugin, or
  [hook](../reference/plugin-hooks.md) of one that failed on a page.
- On a site with [versions](../writing/versions.md), it builds and checks
  every one, and says how many: `built 36 page(s), in 3 version(s) -> ./dist`.

`little build --strict` warns of each as well, then fails, counting each
kind, as CI needs, so a broken site isn't published:

```text
Error: 1 formula(s) that can't be rendered and 2 broken link(s)
```

The site it built is still in the destination, for you to look at.

## little serve

`little serve` builds the site, and serves it at <http://127.0.0.1:8080>,
rebuilding it, and refreshing each browser tab that has it open, whenever you
save.

- It watches `little.yml`, the source folder, the theme's folder, and those
  of the themes it [builds
  on](../themes/choosing-a-theme.md#building-on-another-theme), each
  plugin's file, and the folders plugins read, but not a theme or plugin
  that's [downloaded](../themes/choosing-a-theme.md#downloading-a-theme), as
  it never changes. If you switch folders, themes, or plugins, it follows.
- If a save breaks the build, it prints why, and keeps serving the last
  build.
- After each build, it warns of broken links within the site, but not those
  to other sites, of math it couldn't render, of diagrams it couldn't draw,
  of code blocks plugins couldn't render, and of plugins' warnings, and
  their hooks that failed on a page.
- It serves the site under the path of its
  [`url`](../getting-started/publishing.md#choosing-the-sites-url), if it
  has one, as it'll be published, like <http://127.0.0.1:8080/little/> for
  this manual.
- It sends each file with the type its extension gives it, as a host like
  GitHub Pages does, with text in UTF-8, so a script module, a font, or a
  `.wasm` file works as it will once it's published.
- On a site with [versions](../writing/versions.md), it serves every one,
  and builds them all again on each save. It doesn't watch their refs, so a
  commit to a version's branch shows on your next save.

### Choosing where to serve

`little serve` serves the site at <http://127.0.0.1:8080> unless you choose
another host or port:

| Key | Default | Serves the site… |
| --- | ------- | ---------------- |
| `host` | `127.0.0.1` | at this host name or IP address |
| `port` | `8080` | on this port |

To serve it at <http://localhost:3000> every time, say, set them in
`little.yml`:

```yaml
# little.yml
host: localhost
port: 3000
```

- `little serve --host` and `--port` override them for a single run, like
  `little serve --port 4000` while something else has port 3000.
- The default host, `127.0.0.1`, serves only your own computer. To open the
  site on another device, like your phone, run
  `little serve --host 0.0.0.0` and browse to your computer's IP address.
- `little serve` reads them when it starts, so after changing them, stop it
  and start it again.
- They're only for `little serve`. A theme gets them as `site.host` and
  `site.port`, like every key, but the published site is somewhere else, so
  don't build links from them: that's what
  [`url`](../getting-started/publishing.md#choosing-the-sites-url) is for.
- `host` must be a host name or IP address, like `localhost` or `::1`, and
  `port` a number from 1 to 65535. Otherwise, every command stops and says
  why, even `little build`, which doesn't use them.

## little config

`little config` prints the config a theme gets as `site`, little's, the
theme's, and the site's, merged, as YAML, so you can check a setting without
building. It leaves out what little works out as it builds: `site.nav`,
`site.year`, and, on a site with [versions](../writing/versions.md),
`site.version`, and each version's `root`, and its `url` as the theme gets
it, with the path of the site's `url`. It prints the config of the site's
files as they are, which a version from a ref is built with too, but for
its own `source` and `nav`.
