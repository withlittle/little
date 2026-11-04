---
title: Your first site
description: Start a site, add pages to it, name it, and build it.
---

# Your first site

All a site needs is a `docs/` folder of Markdown. This page starts one,
adds pages to it, and builds it.

## Start it

```sh
little init my-docs
cd my-docs
little serve
```

`little init` makes a `my-docs/` folder with three files:

- `little.yml`, the site's config
- `docs/README.md`, the home page
- `.gitignore`, which keeps the built site out of git

`little serve` builds the site and serves it at <http://127.0.0.1:8080>.
Open it, edit `docs/README.md`, and save. The page reloads itself. Press
Ctrl-C to stop.

To add docs to a project you already have, run `little init` in the
project's folder. It keeps a `docs/README.md` you already have, and adds
to your `.gitignore`. See [`little init`](../reference/commands.md#little-init)
for more.

## Add pages

Each `.md` file in `docs/` is a page. A folder's `README.md` is the
folder's own page, as on GitHub:

```text
my-docs/
├── .gitignore              # keeps dist/ out of git
├── little.yml              # the site's config (optional)
├── docs/                   # each .md file is a page
│   ├── README.md           # → /
│   ├── getting-started.md  # → /getting-started
│   └── guide/
│       ├── README.md       # → /guide/
│       ├── install.md      # → /guide/install
│       └── images/
│           └── setup.png   # → /guide/images/setup.png, copied as it is
└── dist/                   # the built site
```

A page's title is its first `#` heading. To give it another, add
[frontmatter](../writing/frontmatter.md), a block of YAML at the very top:

```markdown
---
title: Install
description: How to install my project.
---

# Installing my project
```

The sidebar lists every page: the home page first, then the rest
[by title](../writing/navigation.md#the-nav). See
[Pages and files](../writing/pages-and-files.md) for how files become
pages, and [Markdown](../writing/markdown.md) for what you can write in
them.

## Link them

Link to a page by its file, relative to the page you're writing. The link
works both on GitHub and in the site:

```markdown
See [Install](guide/install.md), and its [options](guide/install.md#options).
```

## Name it

`little init` names the site after its folder. To change its name, and add
a description, edit `little.yml`:

```yaml
# little.yml
name: My Project
description: Docs that just work
```

`little serve` sees the change, and reloads. See
[Configuring](configuring.md) for what else you can set.

## Build it

```sh
little build
```

This builds the site into `dist/`, replacing the last build. Then it checks
every link, and warns of each broken one:

```text
built 3 page(s) -> ./dist
warning: broken link guide/install.md#optoins in ./docs/README.md: Cannot find fragment
```

`dist/` is plain HTML, ready for any static host. See
[Publishing](publishing.md).
