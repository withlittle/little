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

A site needs a `docs/` folder for content and a `theme/` folder for templates.
Until `little init` lands, copy the default theme from this repo:

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
  there by hand.
- `serve` builds first, then watches `docs/`, `theme/`, and `little.yml`,
  rebuilding and refreshing open browser tabs whenever you save.

## Project layout

```text
my-docs/
├── little.yml              # site config (optional)
├── docs/                   # content: every .md file becomes a page
│   ├── README.md           # → /
│   ├── getting-started.md  # → /getting-started
│   └── guide/
│       ├── README.md       # → /guide/
│       └── install.md      # → /guide/install
├── theme/
│   ├── little.yml          # theme config (optional)
│   ├── templates/
│   │   └── page.html       # renders every page (required)
│   └── assets/             # copied as-is to dist/assets/
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
defaults → `theme/little.yml` → `little.yml`**. Each level is deep-merged over
the one before, so the last writer wins. Both files are optional (an empty file
is fine), so a site sets only what it wants to change:

```yaml
# little.yml
name: "My Project Docs"
tagline: "Docs that just work"
accent: "#e11d48"
```

Every key reaches the theme as `site.<key>`, so you can add your own. little's
built-in defaults cover only little's own settings; a site's name, language,
and look are up to the theme and the site. The default theme uses these keys,
with defaults from its `theme/little.yml`:

| Key | Default | Used for |
| --- | ------- | -------- |
| `name` | `Documentation` | Site title (header and browser tab) |
| `tagline` | none | Text beside the site title |
| `accent` | `#7c3aed` | Links and highlights |
| `font` | a system UI font stack | Body text `font-family` |
| `lang` | `en` | The page's `lang` attribute |

## Theming

The theme owns 100% of the markup; little bakes in no HTML. For each page it
renders `theme/templates/page.html`, a [MiniJinja](https://docs.rs/minijinja)
(Jinja2-style) template, with this data (little loads just this one template
for now):

| Variable | Contents |
| -------- | -------- |
| `site` | The merged config: `site.name`, `site.accent`, and any keys you add |
| `site.nav` | The nav tree: a list of `{ title, url, children }`; `url` is null for folders without a `README.md` |
| `page.title` | The page title |
| `page.url` | The page's URL, e.g. `/guide/install`; compare it with `item.url` to mark the current nav item |
| `page.description` | Frontmatter `description`, or null |
| `page.content` | The rendered Markdown as HTML; output it with the `safe` filter |
| `page.nav` | The page's table of contents (its `##`/`###` headings): a list of `{ title, anchor, depth }` |

Everything in `theme/assets/` is copied to `dist/assets/`, so reference it from
the site root (`/assets/style.css`). The default
[`page.html`](./theme/templates/page.html) is a complete working example.

## Deploying

`dist/` is plain static HTML: upload it to any static host. Page links and
assets are root-relative (`/guide/install`, `/assets/style.css`), so serve the
site from the root of a domain rather than a subpath.

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
