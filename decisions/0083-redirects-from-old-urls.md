# 0083 — Redirects from old URLs

- Status: accepted; amended by
  [0084](./0084-redirects-from-a-folder.md) (a redirect can be from every
  page under a folder, written with `/*`), and by
  [0085](./0085-each-page-in-a-file-named-for-it.md) (a redirect is
  still written in a folder, where a page no longer is, and one to a page
  with a `/` at the end of its URL stops the build)
- Date: 2026-11-20
- Amends: [0002](./0002-theme-structure.md),
  [0013](./0013-pages-that-clash.md),
  [0040](./0040-versions-from-git-refs.md),
  [0046](./0046-plugin-hooks.md),
  [0072](./0072-the-manuals-concise-style.md)

## Context
As a site grows, its pages are moved and renamed, and each one's old URL
leads nowhere: links from other sites break, and so do bookmarks, and
search engines' results. little has no redirects, which this manual has
paid for three times: its records left it
([0065](./0065-decisions-at-the-root.md)), then little's API did
([0070](./0070-littles-api-out-of-the-manual.md)), and its style now has
each page keep its file's name, whatever it's about
([0072](./0072-the-manuals-concise-style.md)).

little keeps its core small, and leaves what only some sites need to
plugins ([VISION.md](../VISION.md)). That's the one rule it states for
what's built in, and earlier records have judged by more:

- **Who needs it:** every site, as every site writes Markdown
  ([0060](./0060-other-formats-in-plugins.md)), or only some, as only
  some write notebooks.
- **Whether a plugin can do it well,** with the hooks there are
  ([0049](./0049-an-api-references-url-and-title.md)).
- **Whether the rest of the build needs to know of it,** as search, link
  checks, and versions each work on the whole site.
- **What it costs every site's binary,** as a parser for each format
  would ([0060](./0060-other-formats-in-plugins.md)).
- **Who writes its markup,** which is the theme's, all of it.

A static site has no server to answer an old URL with a redirect, so a
redirect is a page there, which sends its reader on: a refresh, in its
`<head>`, and a link.

## Options considered
- **In little, or in a plugin:** a plugin could read a list from its
  settings, and add a file for each old URL in its `destination` hook
  ([0046](./0046-plugin-hooks.md)). But that hook is after the theme, so
  the plugin would write the page's HTML itself, which no theme could
  change; nothing else in the build would know the file is a redirect,
  or check where it leads before it's written; and every site that lives
  long enough moves a page. Built in, a redirect needs no new crate.
- **Who writes the page:** little, as 0040 and
  [0041](./0041-version-titles-and-urls.md) took a redirect to be, and
  counted against it; or the theme, from a template of its own, as it
  writes the 404 page ([0002](./0002-theme-structure.md)).
- **Where redirects are listed:**
  - in the frontmatter of the page each leads to, as Jekyll's
    `redirect_from` and Hugo's `aliases` are: the list moves with the
    file, and can't lead nowhere. But it's a setting of little's in a
    page's content, which shouldn't know what builds it, and it can't
    lead to another site, or from a page that's gone;
  - or in `little.yml`, where `site.search.exclude` lists URLs already
    ([0061](./0061-leaving-pages-out-of-search.md)), as MkDocs', Sphinx's,
    and Astro's redirects are.
- **How each is written:** a mapping of old URLs to new, which is
  shortest; or a list, each entry the URL it leads `to`, and those it's
  `from`, as an author thinks of a page and where it's been. `from` could
  be one URL or a list, but each setting is written one way
  ([0080](./0080-settings-in-groups-under-site-and-page.md)).
- **A redirect that leads nowhere in the site:** left to the link check,
  as its page links there ([0016](./0016-checking-links.md)), which
  warns; or an error, as a redirect to nowhere is worse than none.
- **A redirect to a URL that's redirected:** followed, by little, to its
  end; or an error, which says where to lead it, so the list says what
  the site does.
- **A redirect where there's a page or a file:** the redirect wins, the
  page wins, or the build stops, as it does for two files that would be
  one page ([0013](./0013-pages-that-clash.md)).
- **Versions:** every version is built with the site's settings
  ([0042](./0042-versions-share-the-sites-config-and-theme-files.md)),
  so each has the site's redirects. But in an old version, a page that's
  moved since is still at its old URL, and the page it moved to isn't
  there.
  - Each version could list its own, in its entry: a setting for each
    tag, written long after it was cut.
  - Or each takes those it can, as each leaves out the pages
    `site.search.exclude` lists that it has: a redirect is written where
    what it leads to is there, and its old URL is free.
- **A theme without the template:** the build stops, so a site can't use
  redirects until its theme has one; `default.html` in its place, which
  has no content to show; or a warning, with no redirects written.

## Decision
- `site.redirects` lists the site's redirects, each with `to`, the URL it
  leads to, and `from`, a list of the URLs that lead there:

  ```yaml
  site:
    redirects:
      - to: /guide/install
        from:
          - /guide/setup
          - /setup.html
      - to: https://example.com/blog
        from:
          - /news/
  ```

- little has no default for it, and a site's list replaces its theme's,
  as any list does. An entry in `site.versions` can't set it.
- Each `from` is a URL from the root of the site, as
  `site.navigation.items` writes them. `/news` and `/news/` are the same,
  and neither redirects the URLs under it. little writes a page for each,
  where a page at that URL would be, like `guide/setup/index.html`, or,
  for one that ends in `.html`, as a site that was built by another tool
  may have had, at that file's own path.
- `to` is a URL from the root of the site, with a `#fragment` if it
  leads to a heading, or another site's address, starting with `https://`
  or `http://`.
- The theme writes each page, as it writes every other
  ([0002](./0002-theme-structure.md)). A redirect is a type of page,
  `redirect`, whose template is `redirect.html`, with no fallback, as
  for `404.html`. Its `page` has:
  - `page.type`, `redirect`, and `page.url`, the URL it's from;
  - `page.redirect.url`, where it leads, with the path of the site's
    `url`, as little's other URLs have, and `page.redirect.title`, the
    title of the page there, if it's one of the site's;
  - what a 404 page has: `page`'s settings, each group, with nothing in
    it, and, on a site with versions, `page.versions`, with each
    version's home.
- It's in no nav, has no page before or after it, and search doesn't
  index it. It isn't counted among the pages a build says it built.
- The built-in theme's `redirect.html` is a page of its own, without
  the site's header and nav, as a reader sees it for a moment, if at
  all. It sends them on at once, with a `<meta http-equiv="refresh">`,
  which needs no script; tells search engines which page to index, with
  a canonical link and `noindex`; and links to where it leads, for a
  browser that doesn't follow it.
- A theme without a `redirect.html` writes none of them, and
  `little build` warns of it, once, which `--strict` fails on.
- These stop the build, and each error says what to write:
  - a `to` or a `from` that isn't such a URL: a `from` with a `?` or a
    `#`, or with a part little can't name a folder for, like `..`;
  - a `to` that's a `from`, its own or another's, as a reader would be
    sent on twice: the error says where to lead the first;
  - two redirects from one URL, or one where a page couldn't be written
    beside the site's files, like in a version's folder
    ([0013](./0013-pages-that-clash.md)).
- On a site without `versions`, so do a `to` in the site with no page or
  file there, and a `from` where the site has a page or a file.
- On a site with `versions`, each version has the redirects it can
  ([0040](./0040-versions-from-git-refs.md)), each URL from its own
  root, as for `site.search.exclude`: one is written in a version that
  has a page or file where it leads, and none where it's from. A
  redirect that no version has is warned of, not an error, and
  `--strict` fails on it.
- The page's links are checked as any page's are, so a `to` on another
  site that's broken is warned of, and so is a `#fragment` that isn't in
  the page it leads to.
- Plugins get `site.redirects` in their `config` hooks, which can change
  it, and in their `site` hooks. Their `page` and `html` hooks are called
  for each redirect, as for the 404 page, and their `destination` hooks
  see its file. Only comments change in `wit/plugin.wit`, so
  `little:plugin` stays 0.2.1.
- `little serve` serves a redirect's page as a host would.
- The manual's style has a page that moves listed in `site.redirects`,
  where it had each page keep its file's name
  ([0072](./0072-the-manuals-concise-style.md)).
- No new crates.

## Consequences
- A site moves or renames a page with a line in `little.yml`, and its
  old URL still leads to it, whichever host serves the site. A page that's
  gone can lead to another, or to another site.
- A page's content says nothing of little's redirects, so it reads the
  same wherever else it's built. The list is one more thing to change
  when a page moves again: the build stops until its `to` is right.
- A theme chooses how a redirect looks, and what it says. A theme made
  before this has no `redirect.html`, so a site that lists redirects
  gets a warning, and none of them, until its theme has one, or builds
  on the built-in theme.
- `redirects`, under `site`, is now little's, so a site that used the
  name for a setting of its own, for its theme, renames it, and
  `page.type` can be `redirect`. Nothing else the manual documents
  changes, and little's minor version rises, as for any feature
  ([0058](./0058-features-raise-the-minor-version.md)).
- A plugin can write the redirects a host reads, like a `_redirects`
  file, from `site.redirects`, in its `destination` hook, with no hook
  of its own.
- The core-or-plugin tests in this record's context are written down
  for the first time, here. The next such choice can be judged by them.
- Limits, for now:
  - a redirect is a page, not an HTTP redirect, so a browser shows it
    for a moment, and a program that doesn't read HTML isn't sent on;
  - a `#fragment` or `?query` on an old URL isn't carried over, unless a
    theme's template does it, with a script;
  - each old URL is listed, so a folder that's renamed is a line for
    each page in it;
  - a redirect leads within its own version, or to a full address: it
    can't lead from one version into another by a URL from the root of
    the site, so there's still no `/latest/`
    ([0040](./0040-versions-from-git-refs.md));
  - the menu of versions doesn't follow redirects, so a reader who
    switches versions on a page that has moved lands on the other's
    home, as before;
  - a link in the site to an old URL isn't warned of, as its page is
    there;
  - a redirect can't lead to a file a plugin's `destination` hook adds,
    which is added after it's checked.
