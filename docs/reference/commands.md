---
title: Commands
description: little's commands, init, build, serve, and config, and their options.
---

# Commands

```sh
little init my-docs                  # start a site in my-docs/
little init                          # start a site in the current folder
little build                         # render the site into dist/, and warn of broken links
little build --strict                # fail on anything it would warn of, as for CI
little serve                         # dev server with live reload at http://127.0.0.1:8080
little serve --port 3000             # serve on another port, just this once
little config                        # print the site's config, merged
little -C path/to/site build         # work on a site in another directory
little --help                        # list every command and option
```

Every command works on the site in the current folder, the one with its
`little.yml`, or in the folder you name with `-C`, or `--dir`. Each but
`little init` reads the site's [config](../getting-started/configuring.md)
first, and stops if a setting is wrong, even one that command doesn't use.
`little --version` prints little's version.

## little init

`little init` starts a site in the folder you name, making it if it isn't
there, or, without a name, in the current folder:

```sh
little init my-docs
```

```text
created my-docs/little.yml
created my-docs/docs/README.md
created my-docs/.gitignore
next: cd my-docs && little serve
```

- `little.yml` names the site after its folder, here `my-docs`, with what a
  site sets next, like its `url` and `repo`, ready to uncomment.
- `docs/README.md` is the home page, unless `docs/` has a `README.md`
  already, in any case, like `readme.md`, which it keeps.
- `.gitignore` keeps the built site, in `dist/`, out of git. One that's
  there already gets `/dist/` at its end, unless it ignores `dist/`.
- Where there's a `little.yml` already, it stops, and writes nothing.

With `-C`, the folder is in the one `-C` names, so
`little -C sites init my-docs` starts a site in `sites/my-docs`.

## little build

`little build` builds the site into its
[destination](../getting-started/configuring.md#choosing-folders), `dist/`
by default, and says how many pages it built:

```text
built 12 page(s) -> ./dist
```

On a site with [versions](../writing/versions.md), it builds every one, and
counts them too: `built 36 page(s), in 3 version(s) -> ./dist`. A build
that fails leaves the last one in place.

It warns of:

- each [broken link](../writing/links-and-images.md#checking-links)
- each formula it [can't render](../writing/math.md#writing-math), and
  diagram it [can't draw](../writing/diagrams.md)
- each code block a plugin
  [can't render](../plugins/using-plugins.md#code-blocks)
- each URL in [`search.exclude`](../writing/search.md#leaving-pages-out)
  that matches no page
- each warning from a plugin, and each of its hooks that
  [fails on a page](../plugins/using-plugins.md#when-a-plugin-fails)

`little build --strict` fails on any of them, counting each kind:

```text
Error: 1 formula(s) that can't be rendered and 2 broken link(s)
```

The site it built is still in the destination, for you to look at.

## little serve

`little serve` builds the site, and serves it at <http://127.0.0.1:8080>.
Whenever you save, it builds the site again, and refreshes each browser tab
that has it open.

- It watches `little.yml`, the source folder, the theme's folder and those
  of the themes it
  [builds on](../themes/choosing-a-theme.md#building-on-another-theme),
  each plugin's file, and the folders plugins read, and follows when you
  switch any of them.
- If a save breaks the build, it prints why, and keeps serving the last
  build.
- It warns of what [`little build`](#little-build) does, but checks only
  [links within the site](../writing/links-and-images.md#checking-links).
- A site with a
  [`url`](../getting-started/publishing.md#choosing-the-sites-url) is
  served under its path.
- On a site with [versions](../writing/versions.md), it builds every one
  again on each save, so a commit to a version's branch shows on your next
  save.

### Choosing where to serve

To serve the site somewhere else every time, like <http://localhost:3000>,
set `host` and `port`:

```yaml
# little.yml
host: localhost
port: 3000
```

`little serve --host` and `--port` override them for a single run, like
`little serve --port 4000` while something else has port 3000. After
changing them in `little.yml`, stop `little serve` and start it again.

The default host, `127.0.0.1`, serves only your own computer. To open the
site on another device, like your phone, run `little serve --host 0.0.0.0`
and browse to your computer's IP address.

## little config

`little config` prints the site's config, as YAML: little's, the theme's,
and the site's, merged. It's what a theme gets as
[`site`](template-data.md#site), before [plugins](plugin-hooks.md#config)
change it, and without what little works out as it builds: `site.nav`,
`site.year`, and, on a site with [versions](../writing/versions.md),
`site.version`. Each version has its `url` as you wrote it, and no `root`.
