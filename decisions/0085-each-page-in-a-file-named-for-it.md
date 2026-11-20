# 0085 — Each page in a file named for it

- Status: accepted
- Date: 2026-11-20
- Amends: [0013](./0013-pages-that-clash.md),
  [0016](./0016-checking-links.md),
  [0083](./0083-redirects-from-old-urls.md)

## Context
little built each page into a folder named for its URL: `guide/install.md`,
at `/guide/install`, into `guide/install/index.html`
([0013](./0013-pages-that-clash.md)). A host serves a folder's `index.html`
at the folder's URL, which ends with `/`, so GitHub Pages, like most hosts,
answers `/guide/install` with a redirect to `/guide/install/`
([0015](./0015-files-and-relative-links.md)). `little serve` serves the page
at the URL it's asked for, so a site looks one way while it's written, and
another once it's published.

Nothing breaks by it, as each relative link leads from its file's folder
(0015). But every link in a published site is followed twice, and the
address a reader has for a page, with its `/`, isn't the page's `page.url`,
so a canonical link a theme makes from it names a URL that redirects.

## Options considered
- **Where a page is built:**
  - a folder, as before, with links as before: it works on every host, at
    the cost above;
  - a folder, at a URL that ends with `/`, like `/guide/install/`, as
    MkDocs, Hugo, and Zola have: every host serves it with no redirect.
    But a page's URL then looks like a folder's, and a relative URL that
    little doesn't lead, like one in raw HTML, is read from a folder deeper
    than its file's (0015);
  - a file named for the page, `guide/install.html`, which a host serves
    at `/guide/install`, with no redirect, if it serves a `.html` file at
    its path without the extension. GitHub Pages does, and the
    [trailing-slash guide](https://github.com/slorber/trailing-slash-guide)
    lists Netlify and Cloudflare Pages as doing so too, and Vercel with
    its `cleanUrls` on. A plain file server, like Python's `http.server`,
    doesn't, so there every link to a page is not found;
  - or a setting to choose, as MkDocs, Docusaurus, and Astro have, which is
    one more to learn, and two layouts for every theme and plugin to work
    with.
- **A page beside a folder of its name,** like `guide.md` beside `guide/`,
  which was fine while the page was built into that folder (0013). Now the
  page is at `/guide` and the folder at `/guide/`, which hosts don't tell
  apart alike:
  [GitLab Pages](https://docs.gitlab.com/user/project/pages/introduction/)
  sends `/guide` on to `/guide/`, where the page isn't. little could leave
  it to the host; build such a page into the folder, at a URL of another
  shape, which would change as the folder comes and goes; or stop the
  build.
- **Where a redirect is written** (0083): in a file named for its URL, as
  a page there would be, or in a folder, as before. A page's URL is the
  one little links to. A redirect's is one readers already have, with or
  without a `/` at its end, as a host showed it to them, and a folder's
  `index.html` is served for both, on every host.
- **A redirect's `to`,** which named a page with or without a `/` (0083):
  only one of the two is a page's URL now, and a host may find nothing at
  the other.
- **How the link check finds a page's file:** lychee can try `.html` on a
  link with no file, but it swaps the extension it takes a name to have,
  so `releases/v1.2` is looked for in `releases/v1.html`. Or little leads
  each link to its page's file before lychee looks.
- **A link to a page with a `/` at its end,** like `/guide/install/`,
  which worked before, and which GitHub Pages doesn't find now. lychee
  reads a link to a file without its `/`, so can't tell it from the page.
  little has each link in a page's Markdown as it's written
  ([0017](./0017-links-to-pages-files.md)), but not those in the theme,
  or in raw HTML.

## Decision
- A page is built into a file named for it: `guide/install.md`, at
  `/guide/install`, into `guide/install.html`. Its URL is as before, and
  whatever dots its name has, the file's name is the page's with `.html`.
  A folder's own page, from its `README.md`, is the folder's `index.html`,
  at a URL that ends with `/`, as before. There's no setting.
- A host must serve a page's file at the page's URL, without the `.html`,
  which the manual says where it says how to publish.
- `little serve` serves it there, as it did, and finds nothing at a page's
  URL with a `/` at its end, as GitHub Pages doesn't.
- A page beside a file or a folder of its name stops the build, with an
  error that names both, and says to make the page the folder's README, or
  to rename one: `guide.md` beside `guide/install.md`, or beside an image
  in `guide/`, a theme's file there, search's folder, or another version's.
  A `guide.md` and a `guide/README.md` still stop it, as two files that
  would be one page (0013): `/guide` and `/guide/` are one page to a
  reader.
- A redirect is still written in a folder at the URL it's from, like
  `guide/setup/index.html`, so `/news` and `/news/` are still the same,
  and a host finds it with or without its `/`. One from a URL where a page
  is still stops the build, or, on a site with versions, is left out of
  that version.
- A redirect's `to` leads to a page at its URL; to a folder's own page,
  with or without its `/`, as a host leads a reader from one to the other;
  or to a file. One to a page with a `/` at the end of its URL stops the
  build, and the error says where the page is.
- The link check leads each link to a page to the page's file, and looks
  there for an anchor
  ([0075](./0075-finding-anchors-in-the-sites-pages.md)). It warns of a
  link in a page's Markdown that leads to a page with a `/` at its end, in
  words of its own, which say where the page is.
- Plugins' `destination` hooks are given each page's file where it is now
  ([0046](./0046-plugin-hooks.md)). Only a comment changes in
  `wit/plugin.wit`, so `little:plugin` stays 0.2.1.
- No new crates.

## Consequences
- A page is served at its `page.url`, with no redirect, while it's written
  and once it's published, and the destination has a file for each page,
  where the source has one.
- On a host that doesn't serve a `.html` file at its path without the
  extension, a site has no page at any of its links, until the host is set
  to, or the site moves. little says nothing of it, as it can't know the
  host.
- The files a build writes change, which is breaking
  ([0051](./0051-semantic-versioning-and-conventional-commits.md)), so
  little's minor version rises
  ([0058](./0058-features-raise-the-minor-version.md)):
  - a page that a host served at a URL with a `/` at its end, as GitHub
    Pages did, isn't found there now, so a link to it from another site,
    or a bookmark, breaks once, and a redirect can't be written where the
    page is;
  - a site with a page beside a folder of its name doesn't build until the
    page is the folder's README, or one is renamed, which 0013 took to be
    fine;
  - a plugin that reads the paths its `destination` hook is given, like
    one that writes a sitemap, finds each page's file at another path.
- Limits, for now:
  - a link to a page with a `/` at its end is warned of only where it's
    written in Markdown: one in a theme, in `site.navigation.items`, or in
    raw HTML isn't;
  - `little serve` serves a folder's own page at its URL without the `/`,
    where a host sends the browser on to the URL with it.
