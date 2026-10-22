---
title: Links and images
description: Link to pages by their files, and show images, as on GitHub, and how little checks every link.
---

# Links and images

Write links and images relative to the file they're in, as on GitHub, and
they work wherever the site is published.

## Relative links

A relative link or image leads from the folder of the file it's in, whatever
URL its page is served at: in `guide/install.md`,
`![Setup](images/setup.png)` shows `guide/images/setup.png`, as on GitHub.
The picture on [Pages and files](pages-and-files.md#images-and-other-files)
is shown so.

## Links to pages' files

Link to another page by its file, as on GitHub, and the link leads to the
page: `[Usage](usage.md#options)` to `usage#options`, and
`[Guide](guide/README.md)` to `guide/`. A link to a file a
[plugin](../plugins/using-plugins.md#pages-in-other-formats) reads, like a
notebook, `[Analysis](analysis.ipynb#results)`, leads to its page too. This
manual's pages link to each other so, which is why their links work on
GitHub as well.

- A link to a page's URL, like `[Usage](usage)`, works too, but only in the
  site.
- A link from the root, like `/guide/install`, leads from the root of the
  site, but on GitHub, from the root of the repository.
- If the site is published under a path, its
  [`url`](../getting-started/publishing.md#choosing-the-sites-url) adds it to
  every link for you.
- A link to a `.md` file that isn't there leads to a page that isn't either,
  so it's [warned of](#checking-links).

## Checking links

Once it's built the site, `little build` checks every link and image in it,
the theme's too, and warns of each one that's broken: a page, file, or
`#fragment` that isn't there, in the site or on another.

```text
$ little build
built 12 page(s) -> ./dist
warning: broken link guide/install.md#from-sorce in ./docs/README.md: Cannot find fragment
warning: broken link https://github.com/withlittle/litle in ./docs/about.md: Rejected status code: 404 Not Found
```

- Each warning names the file to fix, and the link as it's written there, so
  you can search for it. One in the theme, or in an HTML file in `docs/`, is
  shown as a reader follows it, from the root of the domain. A link on many
  pages, like one in the theme, is one warning, which names a few of them
  and counts the rest.
- `little build --strict` fails on a broken link, a formula little
  [couldn't render](math.md#choosing-how-math-is-written), a diagram it
  [couldn't draw](diagrams.md#choosing-how-diagrams-are-drawn), a code block
  a [plugin](../plugins/using-plugins.md) couldn't render, or a plugin's
  warning, so CI can keep a broken site from being published. `dist/` is
  written either way.
- `little serve` checks only the links within the site, so a save needn't
  wait for other sites.
- The site is checked as it's published. Under the path of its
  [`url`](../getting-started/publishing.md#choosing-the-sites-url), a link
  that leaves the path out is broken. A link to the site's own `url`, like
  one a theme makes with `absolute_url`, is checked in `dist/`, so a new
  page isn't broken before it's published.
- Links to other sites are checked on every build, so without a network,
  each one warns. One to a site that doesn't answer holds the build up until
  it times out, which can take about a minute. Some sites turn link checkers
  away, so a link to one can warn even though it works; there's no way to
  skip one yet.
- Links that only work on your computer or network, like
  `http://localhost:8080/`, and example domains, like `example.com`, aren't
  checked. Nor are `mailto:` and `tel:` links, URLs in code, or links in
  [diagrams](diagrams.md).
- Links into the site's
  [repository](../getting-started/configuring.md#choosing-the-sites-repository),
  like the theme's links to edit each page, aren't checked either: a new
  page's file isn't there until you push it, and a link checker can't see
  the files in a private repository.
- On a site with [versions](versions.md), every version's links within the
  site are checked, but not the links to other sites of a version from a
  ref: they break as the web changes, and a tag can't be fixed.
- Only pages and other HTML files are checked, not CSS. A warning names the
  file, not the line.
- Links are checked with [lychee](https://lychee.cli.rs), which is built into
  little, so there's nothing else to install.
