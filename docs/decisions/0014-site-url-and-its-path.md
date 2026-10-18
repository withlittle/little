# 0014 — A site's `url` puts it under its path

- Status: accepted; amended by [0015](./0015-files-and-relative-links.md),
  which leads a relative link, like `install`, from the folder of the file
  it's written in, and gives it the path too, and by
  [0040](./0040-versions-from-git-refs.md), which builds each version of a
  site but the first under the path, with its folder after it, like
  `/1.4`, and by
  [0042](./0042-versions-share-the-sites-config-and-theme-files.md), which
  leads each version's links to the theme's files to the root of the site
- Date: 2026-10-01

## Context
little builds every link from the root of the domain, like `/guide/install`,
and the built-in theme links to `/assets/style.css` and `/`. A site published
under a path, like a GitHub Pages project site at
`https://withlittle.github.io/little`, was broken: every link left out
`/little`. The README said to publish at the root of a domain, but a
project's GitHub Pages site is where many docs are published first. And a
theme had no way to write an absolute URL, like the one a link preview's
`og:image` needs.

## Options considered
- **How a site says where it's published:** one setting with the path in it,
  like Hugo's `baseURL`, or two, like Jekyll's `url` and `baseurl`, which
  split one address in two.
- **Who adds the path:** the theme, to every URL, with filters like Jekyll's
  `relative_url` and `absolute_url`, so every theme has to remember them on
  every link, and an author can't write a link in Markdown that works both
  locally and once published; or little, to every URL it works out and every
  link in Markdown, as VitePress does with its `base`, with a helper,
  `withBase`, for the URLs a theme writes itself.
- **How little adds it:** to the URLs it gives the theme and to the links in
  Markdown, so a theme needs a filter only for its own; by rewriting every
  URL in the HTML the theme renders, like Eleventy's HTML base plugin, which
  needs an HTML parser, changes markup the theme owns, and can't reach its
  CSS; with a `<base>` tag, which sends every `#fragment` link to the home
  page; or with URLs relative to each page, like `../guide/`, which break on
  a 404 page, which a host serves at any address.
- **Where `little serve` serves:** at the root, as before, so a link that
  leaves the path out works while it's written and breaks only once
  published; or under the path, as it's published, with the root sending the
  browser there.
- **What `absolute_url` gives under `little serve`:** the local address, or
  the published one, always, since absolute URLs are for whatever reads a
  page outside the browser: link previews, canonical links, and feeds.

## Decision
- `url` is the address a site is published at, like
  `https://withlittle.github.io/little`. little has no default: without it, a
  site is at the root of whichever domain serves it, as before.
- little puts the path of `url` before every URL from the root of the site
  that it gives the theme: in `site.nav`, `page.url`, `page.prev`,
  `page.next`, `page.breadcrumbs`, `page.children`, and `page.toc`, and in
  links and images in Markdown. Pages, and the links between them, are worked
  out without it; only what the theme gets has it. A URL that isn't from the
  root of the site, like `https://…`, `#…`, or `install`, is left as it is.
- A theme has two filters for URLs it writes itself: `url` adds the path, and
  `absolute_url` adds the scheme and host to a URL that already has it, so
  `{{ page.url | absolute_url }}` and `{{ '/og.png' | url | absolute_url }}`
  both work. In an HTML template, their output is escaped, but for its `/`s.
- `little serve` serves the site under the path. The root of the domain, and
  the path without its `/`, send the browser to the site's home page with a
  302, which a browser doesn't remember after the path changes. Anything else
  outside the path gets the site's 404 page.
- `url` must start with `https://` or `http://` and have a host, with no `?`,
  `#`, or spaces, or the config doesn't load. A `/` at its end is left off.

## Consequences
- A site published under a path sets `url` once, and authors write links as
  before, from the root of the site.
- A link that leaves the path out is broken under `little serve` too, while
  it's written, not after it's published.
- A theme must put its own URLs from the root, like its stylesheet's, through
  `url`; one that doesn't works only at the root of a domain. A theme that
  puts little's URLs through it gets the path twice, as does an author who
  writes it in a link, like `/little/guide/`.
- A theme's CSS can't use the filter, so it should link to its fonts and
  images relative to the stylesheet.
- Frontmatter still has the last word on `page`
  ([0006](./0006-frontmatter-and-config-win.md)), but the URLs it sets in the
  keys little works out, like `prev: { url: /start/ }`, get the path, as do
  those in a `nav` in `little.yml`, so an author always writes URLs from the
  root of the site. Any other key, like a theme's `logo: /logo.png`, and
  `site.url` itself, reach the theme as written.
- The files a build writes don't move: `dist/index.html` is the home page
  wherever the site is published, since the host puts it under the path.
- Changing `url` while `little serve` runs moves the site to its new path at
  the next rebuild, so tabs open at the old one get the 404 page.
- A site whose address is known only when it's deployed, like a fork's GitHub
  Pages site, has to set it in `little.yml` for now. A `little build --url`
  flag could come later.
