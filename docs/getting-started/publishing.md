---
title: Publishing
description: Publish a site on any static host, under a path with url, and on GitHub Pages with a workflow.
---

# Publishing

`little build` builds the site into `dist/`, or your
[destination](configuring.md#choosing-folders): plain static HTML, for any
static host to serve. Page links, images, and assets are root-relative, like
`/guide/install`, `/guide/images/setup.png`, and `/assets/style.css`, even
those written relative to a page, so they work whether or not a host adds a
`/` to the end of a page's URL.

## Choosing the site's URL

A site published under a path, like a GitHub Pages project site at
`https://<owner>.github.io/<repo>`, needs that path at the start of every
link. Set `url` to the address it's published at, and little adds it, as
this manual does:

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
  [`url` filter](../themes/making-a-theme.md#urls-in-a-theme), and makes a URL
  absolute, as a link preview needs, with `absolute_url`.
- A site at the root of its domain doesn't need `url`, but can set it, like
  `https://docs.example.com`, for a theme that uses `absolute_url`.
- `url` must start with `https://` or `http://`, with nothing after its path
  (no `?` or `#`). Otherwise, every command stops and says why.

## A 404 page

A theme's 404 page, for a URL with no page, is built to `dist/404.html`,
where GitHub Pages, Netlify, and most other static hosts look for one, and
`little serve` uses it too. The built-in theme has one. A theme without one
has no 404 page, and the host shows its own.

## Checking it first

`little build --strict` fails on anything `little build` warns of: a
[broken link](../writing/links-and-images.md#checking-links), a
[formula](../writing/math.md) it couldn't render, a
[diagram](../writing/diagrams.md) it couldn't draw, or a code block a
[plugin](../plugins/using-plugins.md) couldn't render, a plugin's warning,
or a [hook](../reference/plugin-hooks.md) of one that failed on a page. So
build with it in CI, to keep a broken site from being published.
`dist/` is written either way.

Links to other sites are checked too, so a site that's down, or turns link
checkers away, fails the build until it's back, or the link is gone.

## Publishing on GitHub Pages

GitHub Pages can publish a site each time you push to `main`, with a
workflow that installs little and builds it:

1. Set the site's `url` to the address GitHub Pages publishes it at, like
   `https://<owner>.github.io/<repo>`, in `little.yml`.
2. In the repository's settings, under **Pages**, set **Source** to
   **GitHub Actions**.
3. Add this workflow to the repository:

   ```yaml
   # .github/workflows/docs.yml
   name: Docs

   on:
     push:
       branches: [main]

   permissions:
     contents: read
     pages: write
     id-token: write

   concurrency:
     group: pages
     cancel-in-progress: false

   jobs:
     build:
       runs-on: ubuntu-latest
       steps:
         - uses: actions/checkout@v7
         - run: |
             curl -fsSL https://github.com/withlittle/little/releases/download/v0.2.0/little-x86_64-unknown-linux-musl.tar.gz \
               | sudo tar -xz -C /usr/local/bin little
         - run: little build --strict
         - uses: actions/upload-pages-artifact@v5
           with:
             path: dist

     deploy:
       needs: build
       runs-on: ubuntu-latest
       environment:
         name: github-pages
         url: ${{ steps.deployment.outputs.page_url }}
       steps:
         - id: deployment
           uses: actions/deploy-pages@v5
   ```

- Each run downloads the release of little that `curl`'s URL names, for
  Linux on x64, so the site is built by the same little each time, until
  you change the version in that URL to a newer
  [release](https://github.com/withlittle/little/releases)'s.
- For a site in a folder of its own, like `website/`, run
  `little -C website build --strict`, and upload `website/dist`.
- A site with [versions](../writing/versions.md#publishing-versions) needs
  `fetch-depth: 0` in `actions/checkout`'s `with:`, so it fetches each
  version's tag or branch, not just the last commit.
