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

Every command works on the site in the current folder, the one holding its
`little.yml`. To work on another, name its folder with `-C`, or `--dir`.

Each command but `little init` reads the site's
[config](../getting-started/configuring.md) first. If a setting is wrong, it
stops and says why, even if that command doesn't use it.

`little --version` prints little's version.

## little init

`little init` starts a site in the folder you name, and makes the folder if
it isn't there. Without a name, it starts one in the current folder. Then
`little serve` serves it straight away:

```sh
little init my-docs
```

```text
created my-docs/little.yml
created my-docs/docs/README.md
created my-docs/.gitignore
next: cd my-docs && little serve
```

- `little.yml` names the site after its folder, here `my-docs`. It also has
  what a site sets next, like its `url` and `repo`, ready to uncomment.
- `docs/README.md` is the site's home page. If `docs/` has a `README.md`
  already, in any case, like `readme.md`, it's kept.
- A new site has no [plugins](../plugins/using-plugins.md), so a
  `README.ipynb` doesn't count as a home page. Once a plugin reads it,
  delete the `README.md`.
- `.gitignore` keeps the built site, in `dist/`, out of git. If there's one
  already, `/dist/` is added to its end. But if it ignores `dist/` already,
  it's left as it is.
- Where there's a `little.yml` already, it won't start a site. It stops, and
  writes nothing.
- It writes `little.yml` last. So if it fails part of the way through, run
  it again.
- It doesn't ask anything, start a git repository, or choose a theme. The
  site has the built-in theme until you [choose
  another](../themes/choosing-a-theme.md).
- With `-C`, the folder is in the one `-C` names, as with `git -C`. So
  `little -C sites init my-docs` starts a site in `sites/my-docs`.

## little build

`little build` builds the site from its source into its destination,
`dist/` unless you [choose
another](../getting-started/configuring.md#choosing-folders). Then it prints
how many pages it built:

```text
built 12 page(s) -> ./dist
```

- It wipes and regenerates the destination every time, so don't keep
  anything there by hand. A build that fails leaves the last one in place.
- Once it's built the site, it [checks every
  link](../writing/links-and-images.md#checking-links) in it. Then it warns
  of:
  - Each link that's broken.
  - Each formula it [couldn't
    render](../writing/math.md#choosing-how-math-is-written).
  - Each diagram it [couldn't
    draw](../writing/diagrams.md#choosing-how-diagrams-are-drawn).
  - Each code block a [plugin](../plugins/using-plugins.md) couldn't render.
  - Each URL in [`search.exclude`](../writing/search.md#leaving-pages-out)
    that matches no page.
  - Each warning from a plugin, and each
    [hook](../reference/plugin-hooks.md) of one that failed on a page.
- On a site with [versions](../writing/versions.md), it builds and checks
  every one. Then it says how many:
  `built 36 page(s), in 3 version(s) -> ./dist`.

`little build --strict` warns of each as well, then fails, counting each
kind. That's what CI needs, so a broken site isn't published:

```text
Error: 1 formula(s) that can't be rendered and 2 broken link(s)
```

The site it built is still in the destination, for you to look at.

## little serve

`little serve` builds the site, and serves it at <http://127.0.0.1:8080>.
Whenever you save, it rebuilds the site, and refreshes each browser tab that
has it open.

- It watches `little.yml`, the source folder, the theme's folder, and those
  of the themes it [builds
  on](../themes/choosing-a-theme.md#building-on-another-theme). It watches
  each plugin's file, and the folders plugins read, too.
- It doesn't watch a theme or plugin that's
  [downloaded](../themes/choosing-a-theme.md#downloading-a-theme), as it
  never changes.
- If you switch folders, themes, or plugins, it follows.
- If a save breaks the build, it prints why, and keeps serving the last
  build.
- After each build, it warns of what [`little build`](#little-build) would.
  But it checks only links within the site, not those to other sites.
- If the site has a
  [`url`](../getting-started/publishing.md#choosing-the-sites-url), it
  serves the site under its path, as it'll be published. This manual, say,
  is at <http://127.0.0.1:8080/little/>.
- It sends each file with the type its extension gives it, and text in
  UTF-8, as a host like GitHub Pages does. So a script module, a font, or a
  `.wasm` file works as it will once it's published.
- On a site with [versions](../writing/versions.md), it serves every one,
  and builds them all again on each save. But it doesn't watch their refs.
  So a commit to a version's branch shows on your next save.

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
  `site.port`, like every key. But the published site is somewhere else, so
  don't build links from them. That's what
  [`url`](../getting-started/publishing.md#choosing-the-sites-url) is for.
- `host` must be a host name or IP address, like `localhost` or `::1`.
  `port` must be a number from 1 to 65535. Otherwise, every command stops
  and says why, even `little build`, which doesn't use them.

## little config

`little config` prints the config a theme gets as `site`, as YAML. That's
little's, the theme's, and the site's, merged. So you can check a setting
without building.

It leaves out what little works out as it builds:

- `site.nav` and `site.year`.
- On a site with [versions](../writing/versions.md), `site.version`, and
  each version's `root`. Each version's `url` is as it's written: the theme
  gets it with the path of the site's `url` before it.

It prints the config of the site's files as they are. A version from a ref
is built with that config too, but for its own `source`, `nav`, and
plugins' settings.
