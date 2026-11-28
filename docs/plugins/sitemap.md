---
title: Sitemap
description: Write a sitemap.xml of your site's pages, for search engines, with the sitemap plugin.
---

# Sitemap

`sitemap`, one of [little's own plugins](using-plugins.md#littles-own-plugins),
writes a `sitemap.xml` at the root of your site, for search engines: the
address of each page, and when it last changed. Name it in your site's
[`plugins`](using-plugins.md#naming-a-plugin), and set the site's
[`url`](../getting-started/publishing.md#choosing-the-sites-url), which
every address starts with:

```yaml
# little.yml
site:
  url: https://example.com/docs
  plugins:
    sitemap:
      wasm: https://github.com/withlittle/little/releases/download/sitemap-v0.1.0/little_sitemap.wasm
```

It has no settings.

## What it lists

```xml
<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/docs/</loc>
    <lastmod>2026-11-19T02:37:42Z</lastmod>
  </url>
  <url>
    <loc>https://example.com/docs/guide/install</loc>
    <lastmod>2026-11-02T21:05:10Z</lastmod>
  </url>
</urlset>
```

It lists every page, whatever it's built from, like a notebook, or a page
[a plugin adds](using-plugins.md#pages-and-files), by the address a reader
follows. It leaves out the
[404 page](../getting-started/publishing.md#a-404-page),
[redirects](../writing/pages-and-files.md#redirects), and the files little
copies, like [images](../writing/pages-and-files.md#images-and-other-files),
and each [page's own file](../writing/pages-and-files.md#a-pages-file).

`<lastmod>` is when the page
[last changed](../reference/template-data.md#dates), which little reads
from git, unless the page's
[frontmatter](../writing/frontmatter.md#a-pages-dates) sets `modified`. A
page without a date is listed without one. In a shallow clone, as CI often
makes, that's every page, so fetch your repository's whole history there.

## Telling search engines

Give each search engine your sitemap's address, like
`https://example.com/docs/sitemap.xml`. A site at the root of its domain
can name it in its `robots.txt` instead, a file in `docs/`:

```text
Sitemap: https://example.com/sitemap.xml
```

## Versions

On a site with [versions](../writing/versions.md), each version has a
sitemap of its own pages, in its folder, like `/1.4/sitemap.xml`. To leave
an old version without one, turn the plugin off in
[its entry](../writing/versions.md#a-versions-own-nav-and-plugins):

```yaml
# little.yml
site:
  versions:
    - title: "2.0"
      url: /
    - title: "1.4"
      url: /1.4
      ref: v1.4
      plugins:
        sitemap:
          enabled: false
```
