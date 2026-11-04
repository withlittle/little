---
title: Links and images
description: Link to pages by their files, and show images, as on GitHub. little checks every link.
---

# Links and images

Write links and images as on GitHub, relative to the file they're in:

```markdown
<!-- guide/install.md -->
See [Usage](usage.md#options).

![Setup](images/setup.png)
```

A relative link or image leads from the folder of the file it's in,
whatever URL its page is served at. So here, the image is
`guide/images/setup.png`, and the link leads to the page built from
`guide/usage.md`. They work on GitHub, and wherever the site is published.
The picture on [Pages and files](pages-and-files.md#images-and-other-files)
is shown this way.

## Links to pages' files

Link to another page by its file, and the link leads to the page. So
`usage.md#options` leads to `usage#options`, and `guide/README.md` to
`guide/`. This manual's pages link to each other this way, so their links
work on GitHub too.

- A link to a file that a
  [plugin](../plugins/using-plugins.md#pages-in-other-formats) reads leads
  to its page too, like `[Analysis](analysis.ipynb#results)` for a
  notebook.
- A link to a page's URL, like `[Usage](usage)`, works too, but only in the
  site.
- A link from the root, like `/guide/install`, leads from the root of the
  site. On GitHub, it leads from the root of the repository.
- If the site is published under a path, its
  [`url`](../getting-started/publishing.md#choosing-the-sites-url) adds the
  path to every link for you.
- A link to a `.md` file that isn't there leads to a page that isn't there
  either. little [warns of it](#checking-links).

## Checking links

Once it's built the site, `little build` checks every link and image in it,
the theme's too. It warns of each broken one: a page, file, or `#fragment`
that isn't there, in the site or on another.

```text
$ little build
built 12 page(s) -> ./dist
warning: broken link guide/install.md#from-sorce in ./docs/README.md: Cannot find fragment
warning: broken link https://github.com/withlittle/litle in ./docs/about.md: Rejected status code: 404 Not Found
```

Each warning names the file to fix, and the link as it's written there, so
you can search for it.

`little build --strict` fails on a broken link, so CI can keep a broken
site from being published. [`little build`](../reference/commands.md#little-build)
lists everything else it fails on. `dist/` is written either way.

`little serve` checks only the links within the site, so a save doesn't
wait for other sites.

- Links to other sites are checked on every build. So without a network,
  each one warns. A site that doesn't answer holds the build up until it
  times out, which can take about a minute.
- Some sites turn link checkers away, so a link to one can warn even though
  it works. There's no way to skip one yet.
- These aren't checked: links that only work on your computer or network,
  like `http://localhost:8080/`; example domains, like `example.com`;
  `mailto:` and `tel:` links; URLs in code; and links in
  [diagrams](diagrams.md).
- Links into the site's
  [repository](../getting-started/configuring.md#choosing-the-sites-repository),
  like the theme's links to edit each page, aren't checked either. A new
  page's file isn't there until you push it, and a link checker can't see
  the files in a private repository.
- The site is checked as it's published. Under the path of its
  [`url`](../getting-started/publishing.md#choosing-the-sites-url), a link
  that leaves the path out is broken. A link to the site's own `url`, like
  one a theme makes with `absolute_url`, is checked in `dist/`, so a new
  page isn't broken before it's published.
- A broken link in the theme, or in an HTML file in `docs/`, is shown as a
  reader follows it, from the root of the domain. A link on many pages,
  like one in the theme, is one warning, which names a few of the pages,
  and counts the rest.
- On a site with [versions](versions.md), every version's links within the
  site are checked. A version from a git ref doesn't have its links to
  other sites checked: they break as the web changes, and a tag can't be
  fixed.
- Only pages and other HTML files are checked, not CSS. A warning names the
  file, not the line.
- Links are checked with [lychee](https://lychee.cli.rs), which is built
  into little, so there's nothing else to install.
