---
title: Links and images
description: Link to pages by their files, show images, and have little check every link.
---

# Links and images

Write links and images relative to the file they're in:

```markdown
<!-- guide/install.md -->
See [Usage](usage.md#options).

![Setup](images/setup.png)
```

A relative link or image leads from the folder of the file it's in,
whatever URL its page is served at, so here the image is
`guide/images/setup.png`, and the link leads to the page built from
`guide/usage.md`. The same links work in the site and wherever else your
Markdown is read, like in your repository.

## Links to pages' files

A link to a page's file leads to the page: `usage.md#options` leads to
`usage#options`, and `guide/README.md` to `guide/`.

- A link to a file that a
  [plugin](../plugins/using-plugins.md#pages-in-other-formats) reads leads
  to its page too, like `analysis.ipynb#results` for a notebook.
- A link to a page's URL works too, but only in the site, like
  `[Usage](usage)`, or `/guide/install` from the root of the site.
- If the site is published under a path, its
  [`url`](../getting-started/publishing.md#choosing-the-sites-url) adds the
  path to every link for you.
- A link or image in HTML, like `<a href="usage.md">`, or in a
  [diagram](diagrams.md), is left as it is, so it leads where a browser
  takes it.

## Checking links

`little build` checks every link and image in the built site, the theme's
too, and warns of each broken one: a page, file, or `#fragment` that isn't
there, in the site or on another.

```text
$ little build
built 12 page(s) -> ./dist
warning: broken link guide/install.md#from-sorce in ./docs/README.md: Cannot find fragment
warning: broken link https://github.com/withlittle/litle in ./docs/about.md: Rejected status code: 404 Not Found
```

Each warning names the file to fix, and the link as it's written there, so
you can search for it. A link in HTML, like one in the theme, is shown as
the URL a browser follows, and one on many pages is a single warning that
names a few and counts the rest. To fail the build on a broken link, use
[`--strict`](../reference/commands.md#little-build).

`little serve` checks only the links within the site, so a save doesn't
wait for other sites. `little build` checks links to other sites every
time: without a network, each one warns, and a site that turns link
checkers away can warn of a link that works.

little doesn't check:

- links that work only on your computer or network, like
  `http://localhost:8080/`
- example domains, like `example.com`
- `mailto:` and `tel:` links
- URLs in code, and links in diagrams
- links into the site's
  [repository](../getting-started/configuring.md#choosing-the-sites-repository),
  like the theme's links to edit each page
- links to other sites from a [version](versions.md) built from a git ref
