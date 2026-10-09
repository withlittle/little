---
title: Your first site
description: Start a site, add pages to it, name it, and build it.
---

# Your first site

A site needs only a `docs/` folder of Markdown, which little builds with its
built-in theme.

## Start it

```sh
mkdir -p my-docs/docs && cd my-docs
echo "# Hello, little" > docs/README.md
little serve
```

`little serve` builds the site and serves it at <http://127.0.0.1:8080>. Open
it, edit `docs/README.md`, and save: the page reloads itself. Press Ctrl-C to
stop serving.

## Add pages

Each `.md` file in `docs/` is a page, at its path without `.md`, and a
folder's `README.md` is the folder's own page, as on GitHub:

```text
my-docs/
├── little.yml              # the site's config (optional)
├── docs/                   # the source: each .md and .ipynb file is a page
│   ├── README.md           # → /
│   ├── getting-started.md  # → /getting-started
│   └── guide/
│       ├── README.md       # → /guide/
│       ├── install.md      # → /guide/install
│       ├── analysis.ipynb  # → /guide/analysis
│       └── images/
│           └── setup.png   # → /guide/images/setup.png, copied as it is
└── dist/                   # the destination: the built site
```

A page's title is its first `#` heading, unless its
[frontmatter](../writing/frontmatter.md), a block of YAML at the very top,
gives another:

```markdown
---
title: Install
description: How to install my project.
---

# Installing my project
```

The sidebar lists every page: the home page first, then the rest by title,
with each folder's pages under it. See [Pages and
files](../writing/pages-and-files.md) for how files become pages, and
[Markdown](../writing/markdown.md) for what you can write in them.

## Link them

Link to a page by its file, relative to the file you're writing in, and the
link leads to the page, on GitHub and in the site:

```markdown
See [Install](guide/install.md), and its [options](guide/install.md#options).
```

## Name it

The built-in theme calls a site "Documentation" until you name it, in a
`little.yml` beside `docs/`:

```yaml
# little.yml
name: My Project
description: Docs that just work
```

`little serve` sees the change, and reloads. See
[Configuring](configuring.md) for what else a site can set.

## Build it

```sh
little build
```

builds the site into `dist/`, which it wipes first, then checks every link in
it, and warns of each one that's broken:

```text
built 3 page(s) -> ./dist
warning: broken link guide/install.md#optoins in ./docs/README.md: Cannot find fragment
```

`dist/` is plain HTML, for any static host: see [Publishing](publishing.md).
