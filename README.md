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

A site needs a `docs/` folder for content and a theme for templates, which
little looks for in the site's `theme/` folder unless you
[choose another](#choosing-a-theme). Until `little init` lands, copy the
default theme from this repo:

```sh
mkdir my-docs && cd my-docs
cp -r /path/to/little/theme .
mkdir docs && echo "# Hello, little" > docs/README.md
little serve
```

Then add a `little.yml` to name your site. See [Configuration](#configuration).

## Commands

```sh
little build                         # render the site into dist/
little serve                         # dev server with live reload at http://127.0.0.1:8080
little serve --addr 127.0.0.1:3000   # serve on a different address or port
little -C path/to/site build         # work on a site in another directory
little --help                        # list every command and option
```

- `build` wipes and regenerates `dist/` every time, so don't keep anything
  there by hand. A build that fails leaves the last one in `dist/`.
- `serve` builds first, then watches `docs/`, the theme, and `little.yml`,
  rebuilding and refreshing open browser tabs whenever you save. If you switch
  themes, it starts watching the new one. If a save breaks the build, `serve`
  prints why and keeps serving the last build.

## Site layout

```text
my-docs/
├── little.yml              # site config (optional)
├── docs/                   # content: every .md file becomes a page
│   ├── README.md           # → /
│   ├── getting-started.md  # → /getting-started
│   └── guide/
│       ├── README.md       # → /guide/
│       └── install.md      # → /guide/install
├── theme/                  # the theme, unless little.yml chooses another
│   ├── _static/            # copied as-is to the site's root (optional)
│   ├── _templates/         # the templates; default.html is required
│   └── little.yml          # theme config (optional)
└── dist/                   # build output (generated)
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
- **Frontmatter** is an optional YAML block at the very top of a page. little
  reads `title` (otherwise the first `#` heading, then the file or folder name)
  and `description` (passed to the theme, e.g. for a meta description):

  ```markdown
  ---
  title: Install
  description: How to install little.
  ---
  ```

- **Navigation** is built from the folder structure: the home page first, then
  pages and folders sorted by title. A folder with a `README.md` becomes a
  linked section; without one, it's a plain label.
- **On this page.** Each page's `##` and `###` headings become its table of
  contents.

## Configuration

Config is YAML that cascades through three levels: **little's built-in
defaults → the theme's `little.yml` → the site's `little.yml`**. Each level is
deep-merged over the one before, so the last writer wins. Both files are
optional (an empty file is fine), so a site sets only what it wants to change:

```yaml
# little.yml
name: "My Project Docs"
tagline: "Docs that just work"
```

Every key reaches the theme as `site.<key>`, so you can add your own. little's
built-in defaults (see [`src/little.yml`](./src/little.yml)) cover only
little's own settings; a site's name, language, and look are up to the theme
and the site. The default theme uses these keys, with defaults from its
`theme/little.yml`:

| Key | Default | Used for |
| --- | ------- | -------- |
| `name` | `Documentation` | Site title (header and browser tab) |
| `tagline` | none | Text beside the site title |
| `font` | a system UI font stack | Body text `font-family` |
| `lang` | `en` | The page's `lang` attribute |

### Choosing a theme

A site that doesn't set `theme` gets little's built-in theme, which for now is
the one in the site's `theme/` folder. To use another theme, such as one shared
by several sites, point `theme` at its folder:

```yaml
# little.yml
theme: ../shared/my-theme
```

- Relative paths start from the folder holding `little.yml`, not from where
  you run `little`.
- Absolute paths work too, like `/Users/you/my-theme` on macOS or
  `C:\Users\you\my-theme` on Windows. YAML reads `\` as an escape inside
  double quotes, so leave Windows paths unquoted or use single quotes.
- Only a site chooses its theme, so a theme's own `little.yml` can't set
  `theme`.

## Theming

The theme owns 100% of the markup; little bakes in no HTML. A theme is a
folder:

```text
my-theme/
├── _static/      # optional: copied as-is to the site's root
├── _templates/   # the templates; default.html is required
└── little.yml    # optional: the theme's config
```

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
| `site.nav` | The nav tree: a list of `{ title, url, children }`; a folder without a `README.md` has no `url` |
| `page.type` | The page's type: `home`, `section`, `page`, or `404` |
| `page.url` | The page's URL, e.g. `/guide/install`; compare it with `item.url` to mark the current nav item |
| `page.title` | The page title |
| `page.description` | Frontmatter `description`, if the page has one |
| `page.content` | The rendered Markdown as HTML; output it with the `safe` filter |
| `page.nav` | The page's table of contents (its `##`/`###` headings): a list of `{ title, anchor, depth }` |
| `page.children` | Home and section pages only: what else is in the page's folder, shaped like `site.nav` |

A 404 page has just `page.type` and `page.url` (`/404.html`).

Everything in `_static/` is copied to the site's root as it is, so
`_static/assets/style.css` becomes `/assets/style.css`. If a static file would
take the place of a page, the build stops and names both. The default theme's
[`default.html`](./theme/_templates/default.html) and
[`404.html`](./theme/_templates/404.html) are complete working examples.

## Deploying

`dist/` is plain static HTML: upload it to any static host. Page links and
assets are root-relative (`/guide/install`, `/assets/style.css`), so serve the
site from the root of a domain rather than a subpath. A theme's 404 page is
built to `dist/404.html`, where GitHub Pages, Netlify, and most other static
hosts look for one; `little serve` uses it too.

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
