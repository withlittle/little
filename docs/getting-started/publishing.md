---
title: Publishing
description: Publish a site on any static host, or on GitHub Pages with a workflow.
---

# Publishing

```sh
little build
```

This builds your site into `dist/`, which any static host can serve. On
GitHub Pages, a [workflow](#publishing-on-github-pages) can
publish it each time you push.

## Choosing the site's URL

If your site is published under a path, like `https://example.com/docs`,
set `url` to its address:

```yaml
# little.yml
url: https://example.com/docs
```

little adds the path, `/docs`, to every link, so write links without it.
`little serve` serves the site under the path too, at
<http://127.0.0.1:8080/docs/>, so you see it as it'll be published.

A site at the root of its domain, like `https://docs.example.com`, needn't
set `url`, unless its theme makes absolute links, like for link previews.

## A 404 page

A theme's 404 page is built to `dist/404.html`, where most hosts look for
one, and `little serve` shows it too. The built-in theme has one.

## Checking it first

```sh
little build --strict
```

This fails on anything [`little build`](../reference/commands.md#little-build)
warns of, like a broken link. Run it in CI, so a broken site isn't
published.

## Publishing on GitHub Pages

To publish your site each time you push to `main`:

1. In `little.yml`, set `url` to the address GitHub Pages gives you, like
   `https://<owner>.github.io/<repo>`.
2. In your repository's settings, under **Pages**, set **Source** to
   **GitHub Actions**.
3. Add this workflow:

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
             curl -fsSL https://github.com/withlittle/little/releases/download/v0.3.0/little-x86_64-unknown-linux-musl.tar.gz \
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

The workflow downloads the version of little in its `curl` URL, so every
build uses the same little until you change it to a newer
[release](https://github.com/withlittle/little/releases). For a site in
its own folder, like `website/`, run `little -C website build --strict`,
and upload `website/dist`.
