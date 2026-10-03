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
little build                         # render the site into dist/, and warn of broken links
little build --strict                # fail on a broken link, as for CI
little serve                         # dev server with live reload at http://127.0.0.1:8080
little serve --port 3000             # serve on another port, just this once
little config                        # print the site's config, merged
little -C path/to/site build         # work on a site in another directory
little --help                        # list every command and option
```

- `build` wipes and regenerates `dist/` (or your
  [destination](#choosing-folders)) every time, so don't keep anything there by
  hand. A build that fails leaves the last one in place. Once it's built the
  site, it [checks every link](#checking-links) in it, and warns of each one
  that's broken.
- `serve` builds first, then watches the source folder, the theme's folder
  (and those of the themes it [builds on](#building-on-another-theme), unless
  they're [downloaded](#downloading-a-theme)), and `little.yml`, rebuilding and
  refreshing open browser tabs whenever you save. If you switch folders or
  themes, it follows. If a save breaks the build, `serve` prints why and keeps
  serving the last build. It serves at the [host and
  port](#choosing-where-to-serve) your config sets, unless `--host` or
  `--port` says otherwise, and under the path of the site's
  [`url`](#choosing-the-sites-url), if it has one. After each build, it warns
  of broken links within the site.
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
│       ├── install.md      # → /guide/install
│       └── images/
│           └── setup.png   # → /guide/images/setup.png, copied as it is
└── dist/                   # the destination: build output (generated)
```

## Writing docs

- **Pages and URLs.** Every `.md` file under `docs/` becomes a page at a clean,
  extensionless URL (see the layout above). As on GitHub, a folder's
  `README.md` is its landing page. Anything whose name starts with `.`, like a
  `.drafts/` folder and everything in it, is hidden and left out. Two files
  that would be the same page, like `guide.md` and `guide/README.md`, stop the
  build with an error that names both. Pages whose URLs differ only in case,
  like `API.md` and `api/README.md`, aren't caught, but on macOS and Windows,
  which ignore case by default, one replaces the other.
- **Images and other files.** Every other file under `docs/`, like an image or
  a PDF, is copied as it is to the same path in the site, so keep it beside
  the pages that use it. One at the same path as one of the theme's files
  replaces it.
- **Markdown.** CommonMark plus GitHub Flavored Markdown: tables,
  strikethrough, task lists, autolinks, and footnotes. Every heading gets an
  `id`, so you can link straight to a section
  (`guide/install.md#from-source`).
- **Links and images.** Write them relative to the file they're in, as on
  GitHub, and they work wherever the site is published: in
  `guide/install.md`, `![Setup](images/setup.png)` shows
  `guide/images/setup.png`. Link to another page by its file, as on GitHub,
  and the link leads to the page: `[Usage](usage.md#options)` to
  `usage#options`, and `[Guide](guide/README.md)` to `guide/`. A link to the
  page's URL, like `[Usage](usage)`, works too, but only in the site. If the
  site is published under a path, its [`url`](#choosing-the-sites-url) adds
  it to every link for you, but on GitHub, a link from the root leads from
  the root of the repository. `little build`
  [warns of a broken link](#checking-links).
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
description: "Docs that just work"
```

Every key reaches the theme as `site.<key>`, so you can add your own. little's
built-in defaults (see [`src/little.yml`](./src/little.yml)) cover only
little's own settings, like the [theme](#choosing-a-theme), the
[folders](#choosing-folders) a site is built from and into, the
[headings](#choosing-headings-for-tables-of-contents) its tables of contents
list, [where `little serve` serves it](#choosing-where-to-serve), the
[repository](#choosing-the-sites-repository) its source is in, and whether
it's [searchable](#search); a site's name, language, and look are up to the
theme and the site. The
built-in theme uses these keys, with defaults from its
[`little.yml`](./theme/little.yml):

| Key | Default | Used for |
| --- | ------- | -------- |
| `name` | `Documentation` | Site title (header and browser tab) |
| `description` | none | Text beside the site title |
| `font` | a system UI font stack | Body text `font-family` |
| `lang` | `en` | The page's `lang` attribute |
| `search` | `true` | The [search box](#search) in the header, and the index it searches |
| `repo` | none | A link to the [repository](#choosing-the-sites-repository) in the header, and links to edit and view each page's file there below its content |

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
  path, and so does a file in `_sass/`. A file in `_static/` replaces the base
  theme's file published at the same path, so a `style.scss` replaces a
  `style.css`, and the other way round. Templates can extend and include the
  base theme's too.
- A template can't `{% extends %}` the one it replaces: it would extend
  itself. To change part of a base theme's template, replace a smaller one it
  `{% include %}`s instead.

### Choosing folders

A site is built from its `source` folder into its `destination` folder:

| Key | Default | The folder… |
| --- | ------- | ----------- |
| `source` | `./docs` | whose `.md` files become the site's pages, and whose other files are copied into it |
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
- The source can be a hidden folder itself, like `./.github`: only what's
  hidden inside it is left out.
- With `source: .`, everything in the site's folder that isn't hidden is
  published, its `little.yml` included, but the destination: a build never
  copies itself.
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
  don't build links from them: that's what [`url`](#choosing-the-sites-url)
  is for.
- `host` must be a host name or IP address, like `localhost` or `::1`, and
  `port` a number from 1 to 65535. Otherwise, every command stops and says
  why, even `little build`, which doesn't use them.

### Choosing the site's URL

A site published under a path, like a GitHub Pages project site at
`https://<owner>.github.io/<repo>`, needs that path at the start of every
link. Set `url` to the address it's published at, and little adds it:

```yaml
# little.yml
url: https://withlittle.github.io/little
```

- Every URL little gives the theme gets the path: `/guide/install` becomes
  `/little/guide/install` in `site.nav`, in `page.url` and the other links on
  `page`, and in Markdown links and images. So write URLs as usual, from the
  root of the site, without the path, or relative to the file they're in:
  `images/setup.png`, in `guide/install.md`, becomes
  `/little/guide/images/setup.png`. A `#fragment` and a link to another site
  are left as they are.
- `little serve` serves the site under the path too, at
  <http://127.0.0.1:8080/little/>, so a link that leaves it out is broken
  while you write, not after you publish. The server's root sends you to the
  site's home page.
- The build doesn't change shape: the home page is still `dist/index.html`,
  and the host puts it under the path.
- A theme puts the path before its own URLs, like its stylesheet's, with the
  [`url` filter](#urls-in-a-theme), and makes a URL absolute, as a link
  preview needs, with `absolute_url`.
- A site at the root of its domain doesn't need `url`, but can set it, like
  `https://docs.example.com`, for a theme that uses `absolute_url`.
- `url` must start with `https://` or `http://`, with nothing after its path
  (no `?` or `#`). Otherwise, every command stops and says why.

### Choosing the site's repository

Set `repo` to the address of the repository your site's source is in, as a
browser shows it, and a theme can link to it, and to each page's file there,
so readers can see the page's Markdown, or suggest a change to it:

```yaml
# little.yml
repo: https://github.com/withlittle/little
```

- The theme gets `repo` as `site.repo`. On GitHub and GitLab, each page gets
  where to view and edit its file too, as `page.source_url` and
  `page.edit_url`, like
  `https://github.com/withlittle/little/blob/main/docs/guide/install.md` and
  `https://github.com/withlittle/little/edit/main/docs/guide/install.md`. The
  built-in theme links to the repository in its header, and to edit and view
  the page's file below its content.
- The links are to files on the `main` branch. Set `branch` for another, like
  `branch: trunk`, in quotes if it looks like a number, like `branch: "2.0"`.
- Each file's path starts at the root of the repository: the nearest folder,
  from `docs/` up, with a `.git` in it, or, without one, the site's folder. So
  a site in a `website/` folder links to `website/docs/guide/install.md`. A
  source folder outside the repository has no links to its files.
- A page's frontmatter can change either link, or turn it off, like
  `edit_url: false` on a page that's generated from something else.
- A repository anywhere else, like a GitLab or Gitea of your own, gets
  `site.repo`, but no `page.source_url` or `page.edit_url`: little doesn't
  know how its links are made.
- Links into the repository aren't [checked](#checking-links).
- `repo` must start with `https://` or `http://`, with a path after its host,
  and nothing after that path (no `?` or `#`). It isn't the address you clone
  from, so it doesn't end in `.git`. Otherwise, every command stops and says
  why.

## Theming

The theme owns 100% of the markup; little bakes in no HTML. A theme is a
folder:

```text
my-theme/
├── _sass/        # optional: Sass for the stylesheets in _static/ to load
├── _static/      # optional: copied to the site's root, Sass compiled to CSS
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
| `site` | The merged config: `site.name`, `site.description`, and any keys you add |
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
| `page.source_url` | Where to view the page's file in the site's [repository](#choosing-the-sites-repository), if it's on GitHub or GitLab, like `https://github.com/withlittle/little/blob/main/docs/guide/install.md` |
| `page.edit_url` | Where to edit the page's file there, like `https://github.com/withlittle/little/edit/main/docs/guide/install.md` |
| `page.<key>` | Every other key in the page's frontmatter, as written, like `page.description` |

A 404 page has just `page.type` and `page.url` (`/404.html`).

A page's frontmatter has the last word on `page`, even over what little works
out: it merges over it the way a `little.yml` merges over the level beneath,
so `prev: { title: Start here }` renames the previous page's link,
`next: false` removes the next one, `toc: false` removes the table of
contents, `breadcrumbs: false` removes the breadcrumbs, and `edit_url: false`
removes the link to edit the page. It changes only what the theme sees:
`url` doesn't move the page, and `type` doesn't choose its template. A key
left blank is ignored.

Everything in `_static/` is copied to the site's root as it is, but for
[Sass](#sass), so `_static/assets/style.css` becomes `/assets/style.css`,
unless a site has a file at the same path in its source, which replaces it. If
a static file would take the place of a page, or of a file little writes for
[search](#search), the build stops and names both.
The built-in theme's [`default.html`](./theme/_templates/default.html) and
[`404.html`](./theme/_templates/404.html) are complete working examples.

### Sass

A `.scss` or `.sass` file in `_static/` is a stylesheet, which little compiles
to CSS at the same path: `_static/assets/style.scss` becomes
`/assets/style.css`, so the theme links to `{{ '/assets/style.css' | url }}`.
There's nothing to install or configure.

What a stylesheet loads, with `@use`, `@forward`, or `@import`, goes in the
theme's `_sass/` folder, which isn't published. To build Bootstrap with your
own colours, say, copy its `scss/` folder into `_sass/` as it is:

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
  `_sass/` loads from its own folder first, then from `_sass/` itself. Nothing
  outside `_sass/` can be loaded.
- A partial, a Sass file whose name starts with `_`, in `_static/` stops the
  build: move it to `_sass/`. So do two files in one theme that would be
  published at the same path, like `style.scss` and `style.css`.
- A theme that [builds on](#building-on-another-theme) this one can replace
  `_sass/_variables.scss` with its own, and this one's stylesheet loads it
  instead.
- A stylesheet that doesn't compile stops the build, naming the file, line,
  and column of the error, and `little serve` keeps serving the last good
  build. `@warn` and `@debug` messages name theirs too.
- little compiles Sass with [grass](https://github.com/connorskees/grass),
  which follows Dart Sass 1.54, so newer Sass, like `color.channel()`, won't
  compile, and there are no `pkg:` URLs.
- Sass in the site's source is copied as it is, like any other file.

### URLs in a theme

Every URL little gives a theme, like `page.url` and those in `site.nav`,
already has the path of the site's [`url`](#choosing-the-sites-url) before
it, if it has one. For the URLs a theme writes itself, little adds two
filters:

- `url` puts the path before a URL from the root of the site:
  `{{ '/assets/style.css' | url }}` is `/little/assets/style.css` for a site
  at `https://withlittle.github.io/little`, and `/assets/style.css` for one at
  the root of its domain. Don't put little's URLs through it, or they get the
  path twice.
- `absolute_url` puts the site's scheme and host before a URL that has the
  path, for a link preview, a canonical link, or a feed:
  `{{ page.url | absolute_url }}` is
  `https://withlittle.github.io/little/guide/install`. For one of the theme's
  own URLs, use both: `{{ '/og.png' | url | absolute_url }}`. A site without
  a `url` gets the URL back as it is.

Both leave a URL that isn't from the root of the site, like `https://…` or
`#…`, as it is. A theme's CSS can't use them, so link to fonts and images
from a stylesheet relative to it, like `url(fonts/inter.woff2)`.

### A search box

A theme with a search box sets `search: true` in its `little.yml`, so little
[indexes](#search) each site built with it into `pagefind/`. It shows the box
only when `site.search` is on, since a site can turn search off. The quickest
box is [Pagefind](https://pagefind.app/)'s own, which the built-in theme uses:

```html
<!-- In <head>, before the theme's stylesheet -->
{% if site.search %}
<link rel="stylesheet" href="{{ '/pagefind/pagefind-component-ui.css' | url }}">
<script src="{{ '/pagefind/pagefind-component-ui.js' | url }}" defer></script>
{% endif %}

<!-- Where its button goes -->
{% if site.search %}
<pagefind-modal-trigger></pagefind-modal-trigger>
<pagefind-modal></pagefind-modal>
{% endif %}
```

- Load its script with `defer`, not as the module Pagefind's docs show:
  deferred, it finds the index beside it, even under the path of the site's
  [`url`](#choosing-the-sites-url). A module looks in `/pagefind/` at the
  root of the domain, unless
  `<pagefind-config bundle-path="{{ '/pagefind/' | url }}">` tells it where
  the index is.
- Restyle it with Pagefind's `--pf-*` CSS variables, as the built-in theme's
  [`style.css`](./theme/_static/assets/style.css) does. Pagefind keeps the
  rest of a theme's CSS out of it.
- For a box of your own, search with Pagefind's
  [JavaScript API](https://pagefind.app/docs/api/), from
  `{{ '/pagefind/pagefind.js' | url }}`.
- Each page is indexed in the language of its `<html lang>`.

## Checking links

Once it's built the site, `little build` checks every link and image in it,
the theme's too, and warns of each one that's broken: a page, file, or
`#fragment` that isn't there, in the site or on another.

```text
$ little build
built 12 page(s) -> ./dist
warning: broken link guide/install.md#from-sorce in ./docs/README.md: Cannot find fragment
warning: broken link https://github.com/withlittle/litle in ./docs/about.md: Rejected status code: 404 Not Found
```

- Each warning names the file to fix, and the link as it's written there, so
  you can search for it. One in the theme, or in an HTML file in `docs/`, is
  shown as a reader follows it, from the root of the domain. A link on many
  pages, like one in the theme, is one warning, which names a few of them and
  counts the rest.
- `little build --strict` fails on a broken link, so CI can keep a broken
  site from being published. `dist/` is written either way.
- `little serve` checks only the links within the site, so a save needn't
  wait for other sites.
- The site is checked as it's published. Under the path of its
  [`url`](#choosing-the-sites-url), a link that leaves the path out is
  broken. A link to the site's own `url`, like one a theme makes with
  `absolute_url`, is checked in `dist/`, so a new page isn't broken before
  it's published.
- Links to other sites are checked on every build, so without a network,
  each one warns. One to a site that doesn't answer holds the build up until
  it times out, which can take about a minute. Some sites turn link checkers
  away, so a link to one can warn even though it works; there's no way to
  skip one yet.
- Links that only work on your computer or network, like
  `http://localhost:8080/`, and example domains, like `example.com`, aren't
  checked. Nor are `mailto:` and `tel:` links, or URLs in code.
- Links into the site's [repository](#choosing-the-sites-repository), like
  the theme's links to edit each page, aren't checked either: a new page's
  file isn't there until you push it, and a link checker can't see the files
  in a private repository.
- Only pages and other HTML files are checked, not CSS. A warning names the
  file, not the line.
- Links are checked with [lychee](https://lychee.cli.rs), which is built into
  little, so there's nothing else to install.

## Search

A site built with the built-in theme is searchable out of the box: the search
box in its header, which ⌘K or Ctrl+K opens too, finds pages, and the
headings within them, as a reader types.

- Each build indexes every page into `pagefind/` in `dist/`, and the box
  searches it in the reader's browser, so there's no server or service to set
  up. `little serve` indexes each save. little indexes with
  [Pagefind](https://pagefind.app/), which is built in, so there's nothing
  else to install.
- What's indexed is each page's own content, from its Markdown, under its
  title, so nothing the theme puts around it, like its header or nav, fills
  the results. The 404 page isn't indexed, nor is an HTML file copied from
  `docs/`.
- Words are matched by their stem in the site's [`lang`](#configuration), so
  "installing" finds "install".
- To go without, set `search: false`, and the built-in theme leaves out the
  box too:

  ```yaml
  # little.yml
  search: false
  ```

- A page or file where little writes one of the search's files, like
  `docs/pagefind/pagefind.js`, stops the build. A page at `/pagefind`, from
  `docs/pagefind.md`, is fine, and so is any other file in `docs/pagefind/`.
- `pagefind/` is about 0.7 MB for a site of a few pages, and about 3 MB for one
  of 450. A reader downloads only the parts of it a search needs.
- A page can't leave itself out of search yet. Chinese, Japanese, and Thai,
  which don't put spaces between words, search poorly.

A theme without a search box leaves search off; see [A search
box](#a-search-box) to add one.

## Deploying

`dist/` (or your [destination](#choosing-folders)) is plain static HTML: upload
it to any static host. Page links, images, and assets are root-relative
(`/guide/install`, `/guide/images/setup.png`, `/assets/style.css`), even those
written relative to a page, so they work whether or not a host adds a `/` to
the end of a page's URL. To publish the site under a path, like a GitHub
Pages project site, set its [`url`](#choosing-the-sites-url).
A theme's 404 page is built to `dist/404.html`,
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
