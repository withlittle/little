---
title: Pages and files
description: How the files in docs/ become a site's pages, at clean URLs, and its other files.
---

# Pages and files

Every `.md` file in `docs/`, and every Jupyter notebook (`.ipynb`), becomes a
page, at a clean URL, without its extension:

![docs/guide/install.md becomes the page at /guide/install](images/pretty-urls.svg)

## Pages and URLs

```text
docs/
├── README.md           # → /
├── getting-started.md  # → /getting-started
└── guide/
    ├── README.md       # → /guide/
    ├── install.md      # → /guide/install
    ├── analysis.ipynb  # → /guide/analysis
    └── images/
        └── setup.png   # → /guide/images/setup.png, copied as it is
```

- As on GitHub, a folder's `README.md` is its landing page, at the folder's
  URL, which ends in `/`, and `docs/README.md` is the site's home page.
- A notebook is a page as a Markdown file is: see
  [Notebooks](notebooks.ipynb).
- A page's title is its frontmatter's `title`, else its first `#` heading,
  else its file's or folder's name: see [Frontmatter](frontmatter.md).
- The source is `docs/` unless the site [chooses another
  folder](../getting-started/configuring.md#choosing-folders).

## Hidden files

Anything whose name starts with `.`, like a `.drafts/` folder and everything
in it, or Jupyter's `.ipynb_checkpoints/`, is hidden, and left out, and
saving it doesn't rebuild the site in `little serve`. Only names inside the
source count, so the source can be a hidden folder itself, like `./.github`.

## Pages that clash

Two files that would be the same page, like `guide.md` and
`guide/README.md`, or `analysis.md` and `analysis.ipynb`, stop the build
with an error that names both:

```text
Error: docs/guide.md and docs/guide/README.md would both be the page at /guide/: rename or move one
```

On a site with [versions](versions.md), so does a page or file where
another version goes, like `docs/1.4.md`, for a version at `/1.4`.

Pages whose URLs differ only in case, like `API.md` and `api/README.md`,
aren't caught, but on macOS and Windows, which ignore case by default, one
replaces the other.

## Images and other files

Every other file in `docs/`, like an image or a PDF, is copied as it is to the
same path in the site, so keep it beside the pages that use it. The picture
above is `docs/writing/images/pretty-urls.svg`, beside this page's file,
`docs/writing/pages-and-files.md`, and shown with `images/pretty-urls.svg`,
relative to it, as on GitHub: see [Links and images](links-and-images.md).

- A file at the same path as one of the theme's files replaces it, like a
  `docs/assets/style.css` for the built-in theme's stylesheet.
- Sass in the source is copied as it is, like any other file: only a
  theme's Sass is [compiled](../themes/sass.md).
