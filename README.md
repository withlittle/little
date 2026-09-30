# little

A little static site generator for documentation: drop Markdown into `docs/`
and get a site. One self-contained binary, a three-level cascading config, and
themes that own 100% of the markup.

> **Status:** early development (v0.1). Expect breaking changes. See the
> [roadmap](./VISION.md#roadmap-thin-vertical-slices).

## Install

There are no prebuilt binaries yet, so build little from source with the stable
[Rust toolchain](https://rustup.rs):

```sh
git clone https://github.com/withlittle/little.git
cd little
cargo install --locked --path .   # installs `little` into ~/.cargo/bin
```

Or run `cargo build --release` and use the standalone binary at
`target/release/little`.

## Quick start

This repository doubles as an example site. From the repo root, run:

```sh
little serve   # or, without installing: cargo run -- serve
```

Open <http://127.0.0.1:8080>, edit a file in `docs/`, and the page reloads
itself.

### Start your own site

A site needs only a `docs/` folder of Markdown, which little builds with its
built-in theme. You can choose [other folders](#choosing-folders) and
[another theme](#choosing-a-theme).

```sh
mkdir -p my-docs/docs && cd my-docs
echo "# Hello, little" > docs/README.md
little serve
```

Then add a `little.yml` to name your site. See [Configuration](#configuration).

## Commands

```sh
little build                         # render the site into dist/
little serve                         # dev server with live reload at http://127.0.0.1:8080
little serve --port 3000             # serve on another port, just this once
little config                        # print the site's config, merged
little -C path/to/site build         # work on a site in another directory
little --help                        # list every command and option
```

- `build` wipes and regenerates `dist/` (or your
  [destination](#choosing-folders)) every time, so don't keep anything there by
  hand. A build that fails leaves the last one in place.
- `serve` builds first, then watches the source folder, the theme's folder
  (and those of the themes it [builds on](#building-on-another-theme), unless
  they're [downloaded](#downloading-a-theme)), and `little.yml`, rebuilding and
  refreshing open browser tabs whenever you save. If you switch folders or
  themes, it follows. If a save breaks the build, `serve` prints why and keeps
  serving the last build. It serves at the [host and
  port](#choosing-where-to-serve) your config sets, unless `--host` or
  `--port` says otherwise.
- `config` prints the [config](#configuration) the theme gets as `site`
  (without `site.nav`), so you can check a setting without building.

## Site layout

```text
my-docs/
├── little.yml              # site config (optional)
├── docs/                   # the source: every .md file becomes a page
│   ├── README.md           # → /
│   ├── getting-started.md  # → /getting-started
│   └── guide/
│       ├── README.md       # → /guide/
│       └── install.md      # → /guide/install
└── dist/                   # the destination: build output (generated)
```

## Writing docs

- **Pages and URLs.** Every `.md` file under `docs/` becomes a page at a clean,
  extensionless URL (see the layout above). As on GitHub, a folder's
  `README.md` is its landing page.
- **Markdown.** CommonMark plus GitHub Flavored Markdown: tables,
  strikethrough, task lists, autolinks, and footnotes. Every heading gets an
  `id`, so you can link straight to a section (`/guide/install#from-source`).
- **Links.** Link to other pages by URL (`[Install](/guide/install)`), not by
  file name. `.md` links aren't rewritten.
- **Frontmatter** is an optional block of YAML settings at the very top of a
  page. Every setting reaches the theme as `page.<key>`, like `description`
  for a meta description, and has the last word on what `page` holds. little
  itself reads `title` for the nav (otherwise the first `#` heading, then the
  file or folder name):

  ```markdown
  ---
  title: Install
  description: How to install little.
  ---
  ```

- **Navigation** is built from the folder structure: the home page first, then
  pages and folders sorted by title. A folder with a `README.md` becomes a
  linked section; without one, it's a plain label. Each page's previous and
  next pages follow the same order, from top to bottom, and its breadcrumbs
  are what's above it: the home page, then each folder it's in. To write the
  nav yourself, set `nav` in `little.yml` to a list of
  `{ title, url, children }`. It replaces little's. Breadcrumbs and previous
  and next pages follow it instead, so a page that isn't in it has none. Use
  each page's URL as little builds it (`/guide/`, not `/guide`), so the theme
  can mark the current page:

  ```yaml
  # little.yml
  nav:
    - { title: Home, url: / }
    - title: Guide
      url: /guide/
      children:
        - { title: Install, url: /guide/install }
  ```

- **On this page.** Each page's `##` and `###` headings become its table of
  contents, with each `###` under the `##` above it. To list other levels of
  heading, set [`toc_min_depth` and
  `toc_max_depth`](#choosing-headings-for-tables-of-contents).

## Configuration

Config is YAML that cascades through three levels: **little's built-in
defaults → the theme's `little.yml` → the site's `little.yml`**. Each level is
deep-merged over the one before, so the last writer wins. Both files are
optional (an empty file is fine), so a site sets only what it wants to change,
and `little config` prints the result:

```yaml
# little.yml
name: "My Project Docs"
tagline: "Docs that just work"
```

Every key reaches the theme as `site.<key>`, so you can add your own. little's
built-in defaults (see [`src/little.yml`](./src/little.yml)) cover only
little's own settings, like the [theme](#choosing-a-theme), the
[folders](#choosing-folders) a site is built from and into, the
[headings](#choosing-headings-for-tables-of-contents) its tables of contents
list, and [where `little serve` serves it](#choosing-where-to-serve); a site's
name, language, and look are up to the theme and the site. The
built-in theme uses these keys, with defaults from its
[`little.yml`](./theme/little.yml):

| Key | Default | Used for |
| --- | ------- | -------- |
| `name` | `Documentation` | Site title (header and browser tab) |
| `tagline` | none | Text beside the site title |
| `font` | a system UI font stack | Body text `font-family` |
| `lang` | `en` | The page's `lang` attribute |

### Choosing a theme

`theme` chooses the theme a site is built with:

| `theme` | The site is built with… |
| ------- | ----------------------- |
| `true` (the default) | little's built-in theme |
| `false` | no theme, which `little build` can't do yet |
| a path, like `../shared/my-theme` | the theme in that folder |
| a URL, like `https://example.com/my-theme.tar.gz` | the theme in that archive, [downloaded](#downloading-a-theme) |

```yaml
# little.yml
theme: ../shared/my-theme
```

- Relative paths start from the folder holding the `little.yml` that sets
  them, not from where you run `little`.
- Absolute paths work too, like `/Users/you/my-theme` on macOS or
  `C:\Users\you\my-theme` on Windows. YAML reads `\` as an escape inside
  double quotes, so leave Windows paths unquoted or use single quotes.
- Only unquoted `true` and `false` choose the built-in theme or none:
  `"true"`, `yes`, and `no` are all folder names.

### Downloading a theme

Set `theme` to the URL of a theme's `.zip` or `.tar.gz` archive, and the first
build downloads it. On GitHub, a tag's archive is at
`https://github.com/OWNER/REPO/archive/refs/tags/TAG.tar.gz`:

```yaml
# little.yml
theme: https://github.com/withlittle/theme-bootstrap/archive/refs/tags/0.0.1.tar.gz
```

- The archive must hold the theme in a single folder, as GitHub's do.
- little keeps the theme in the site's `.little/` folder, which git ignores,
  and never downloads that URL again, so later builds work offline. To use
  another version, change the URL; to download the same URL again (say, a
  branch's archive, which changes), delete `.little/`.
- Downloads are over HTTPS only, and little can't sign in, so a private
  repository's archive answers 404. For safety, an archive holding symlinks,
  hard links, or paths that lead out of it is an error.
- `little serve` doesn't watch a downloaded theme, as it never changes.
- With `source: .`, `.little/` is inside the source, so the Markdown in a
  downloaded theme (its README, say) becomes pages too.

### Building on another theme

A theme's own `little.yml` can set `theme` too, to build on another theme. Its
settings go over that theme's, and its files go over that theme's, file by
file, so it needs only what it changes. A company theme can be just a
`little.yml` that builds on a downloaded theme:

```yaml
# company-theme/little.yml
theme: https://github.com/withlittle/theme-bootstrap/archive/refs/tags/0.0.1.tar.gz
font: "Inter, sans-serif"
colors:
  accent: navy
```

A site that uses it cascades **little → theme-bootstrap → company-theme →
site**. Themes can build on themes that build on others, but not in a loop. In
a theme's `little.yml`, a path is relative to the theme's folder, and a URL is
downloaded into the site's `.little/`.

- A template in `_templates/` replaces the base theme's template at the same
  path, and a file in `_static/` replaces its file at the same path. Templates
  can extend and include the base theme's too.
- A template can't `{% extends %}` the one it replaces: it would extend
  itself. To change part of a base theme's template, replace a smaller one it
  `{% include %}`s instead.

### Choosing folders

A site is built from its `source` folder into its `destination` folder:

| Key | Default | The folder… |
| --- | ------- | ----------- |
| `source` | `./docs` | whose `.md` files become the site's pages |
| `destination` | `./dist` | the site is built into |

To use others, set them in `little.yml`:

```yaml
# little.yml
source: ./content
destination: ./public
```

- Paths are relative to the site's folder, the one holding its `little.yml`,
  or absolute. A theme's `little.yml` can set them too, but they're still
  relative to the site.
- Every build wipes the destination, so point it at a folder that holds nothing
  but builds. little leaves a `.little-build` file in each folder it builds,
  and only wipes a folder that has one or is empty: pointed at any other
  folder, a build stops instead.

### Choosing headings for tables of contents

Each page's table of contents lists its headings from one level to another, 2
for `##` to 6 for `######`:

| Key | Default | Lists headings… |
| --- | ------- | --------------- |
| `toc_min_depth` | `2`, for `##` | from this level |
| `toc_max_depth` | `6`, for `######`; `3`, for `###`, in the built-in theme | down to this level |

To list the `####` headings too, say, set it in `little.yml`:

```yaml
# little.yml
toc_max_depth: 4
```

- A page's `#` heading is its title, so tables of contents start at `##`.
  With `toc_min_depth: 1`, they list it too, and the other headings under it.
- A theme can set them in its `little.yml`, as the built-in theme does, and
  the site's `little.yml` has the last word. They're the same for every page:
  a page can only turn its table of contents off, with `toc: false` in its
  frontmatter.
- Each must be a level from 1 to 6, and `toc_min_depth` can't be more than
  `toc_max_depth`: otherwise, the build stops and says why.

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
  don't build links from them.
- `host` must be a host name or IP address, like `localhost` or `::1`, and
  `port` a number from 1 to 65535. Otherwise, every command stops and says
  why, even `little build`, which doesn't use them.

## Theming

The theme owns 100% of the markup; little bakes in no HTML. A theme is a
folder:

```text
my-theme/
├── _static/      # optional: copied as-is to the site's root
├── _templates/   # the templates; default.html is required
└── little.yml    # optional: the theme's config
```

little's built-in theme is this repo's [`theme/`](./theme/) folder, compiled
into little.

Templates are [MiniJinja](https://docs.rs/minijinja) (Jinja2-style). little
renders each page with the template for its type, or with `default.html` if
the theme doesn't have that template:

| Page type | Pages | Template |
| --------- | ----- | -------- |
| `home` | `docs/README.md` | `home.html`, else `default.html` |
| `section` | a folder's `README.md`, like `docs/guide/README.md` | `section.html`, else `default.html` |
| `page` | every other `.md` file | `page.html`, else `default.html` |
| `404` | the page a host shows for a URL with no page | `404.html`, else none |

A 404 page has no content for `default.html` to show, so a theme without a
`404.html` gets no 404 page, and the host shows its own. Templates can
`{% extends %}` and `{% include %}` any others in `_templates/`, in subfolders
too, so `default.html` can be a layout that the rest extend.

Each template gets this data. A page leaves out anything it doesn't have,
rather than setting it to null, so `{% if page.description %}` and
`{{ page.description | default("…") }}` both work:

| Variable | Contents |
| -------- | -------- |
| `site` | The merged config: `site.name`, `site.tagline`, and any keys you add |
| `site.nav` | The nav tree: a list of `{ title, url, children }`; a folder without a `README.md` has no `url`. A `nav` in `little.yml` replaces it |
| `page.type` | The page's type: `home`, `section`, `page`, or `404` |
| `page.url` | The page's URL, e.g. `/guide/install`; compare it with `item.url` to mark the current nav item |
| `page.title` | The page title |
| `page.content` | The rendered Markdown as HTML; output it with the `safe` filter |
| `page.toc` | The page's table of contents (its headings from [`toc_min_depth` to `toc_max_depth`](#choosing-headings-for-tables-of-contents)), shaped like `site.nav`: a list of `{ title, url, children }`, where `url` is the heading's `#fragment` and each heading is a child of the nearest one above it with fewer `#`s, like a `###` of the `##` above it |
| `page.children` | Home and section pages only: what else is in the page's folder, shaped like `site.nav` |
| `page.breadcrumbs` | What's above this page in `site.nav`, from the top down: a list of `{ title, url }`, where a folder without a `README.md` has no `url`. It starts with the home page, if `site.nav` lists it at its top, and leaves out this page. The home page, and a page that isn't in `site.nav`, have an empty list |
| `page.prev` | The page before this one in `site.nav`, read from top to bottom: `{ title, url }`. The first page has none |
| `page.next` | The page after this one in `site.nav`: `{ title, url }`. The last page has none |
| `page.<key>` | Every other key in the page's frontmatter, as written, like `page.description` |

A 404 page has just `page.type` and `page.url` (`/404.html`).

A page's frontmatter has the last word on `page`, even over what little works
out: it merges over it the way a `little.yml` merges over the level beneath,
so `prev: { title: Start here }` renames the previous page's link,
`next: false` removes the next one, `toc: false` removes the table of
contents, and `breadcrumbs: false` removes the breadcrumbs. It changes only
what the theme sees:
`url` doesn't move the page, and `type` doesn't choose its template. A key
left blank is ignored.

Everything in `_static/` is copied to the site's root as it is, so
`_static/assets/style.css` becomes `/assets/style.css`. If a static file would
take the place of a page, the build stops and names both. The built-in theme's
[`default.html`](./theme/_templates/default.html) and
[`404.html`](./theme/_templates/404.html) are complete working examples.

## Deploying

`dist/` (or your [destination](#choosing-folders)) is plain static HTML: upload
it to any static host. Page links and assets are root-relative
(`/guide/install`, `/assets/style.css`), so serve the site from the root of a
domain rather than a subpath. A theme's 404 page is built to `dist/404.html`,
where GitHub Pages, Netlify, and most other static hosts look for one;
`little serve` uses it too.

## Development

```sh
cargo test                  # run the tests
cargo clippy --all-targets  # lint (CI fails on warnings)
cargo fmt                   # format
```

See [`AGENTS.md`](./AGENTS.md) for architecture and conventions,
[`VISION.md`](./VISION.md) for where little is headed, and
[`decisions/`](./decisions/) for design records.

## License

[MIT](./LICENSE)
