# 0090 — A plugin for sitemaps

- Status: accepted
- Date: 2026-11-28
- Amends: [0051](./0051-semantic-versioning-and-conventional-commits.md),
  [0071](./0071-ordering-and-naming-the-manuals-plugin-pages.md)

## Context
A search engine finds a site's pages by following its links, or from its
sitemap: a file, `sitemap.xml`, that lists each page's address, and can
say when each last changed, as the
[sitemap protocol](https://www.sitemaps.org/protocol.html) sets. little
writes none. [0046](./0046-plugin-hooks.md) gave plugins a hook for each
step of a build, with a sitemap as the example of what `destination` is
for, but none of little's own plugins uses it: they export only `setup`,
`files`, and `formats`.

What the protocol asks of a sitemap:

- each address is whole, with its scheme and host, so a site needs its
  `url` ([0014](./0014-site-url-and-its-path.md)), which a site at the
  root of its domain needn't set;
- it lists only addresses under the folder it's in, 50,000 at most;
- each page can have a `<lastmod>`, a `<changefreq>`, and a `<priority>`.
  Google ignores the last two, and Bing largely disregards them. Both read
  `<lastmod>`, where it's accurate.

A site with versions ([0040](./0040-versions-from-git-refs.md)) is built
a version at a time, each in its own folder, and one from a git ref starts
plugins of its own, whose `destination` hooks see only its folder
([0046](./0046-plugin-hooks.md)).

## Options considered
- **In little, or in a plugin,** by the tests
  [0083](./0083-redirects-from-old-urls.md) lists:
  - who needs it: only a site that's public, and wants search engines to
    read it, not every site;
  - whether a plugin can do it well: one was tried, and wrote a right
    sitemap from the hooks there are;
  - whether the rest of the build needs to know of it: no, as search, link
    checks, the nav, and versions never read it;
  - what it costs every site's binary: next to nothing, as little reads
    URLs and dates already;
  - who writes its markup: built in, little would write XML itself, or
    each theme would need a template for a format that leaves it nothing
    to design.

  Built in, a site with a `url` would get a sitemap with nothing to write,
  and little knows every version at once, and where each page is,
  whatever its frontmatter says. A plugin is three lines of `little.yml`.
- **Which hook tells it the site's pages:**
  - `destination`, alone, from the paths of the files a build writes: but
    a redirect, and an `.html` file that's copied, look like pages there,
    and none has a date;
  - `html`, which is given each page's URL as a reader follows it: but a
    plugin that exports it has `little serve` render every page on every
    save ([0076](./0076-rebuilding-only-what-differs.md)), and it has no
    dates either;
  - or `page`, which is given each page's type, URL, and dates, as the
    theme gets them, and leaves a rebuild to the pages that differ.
- **Where the site's address comes from:** `config`, which a version from
  a git ref doesn't call; or `site`, which every version does.
- **What `<lastmod>` is:** when the site was built, so every page seems to
  change with each build, and a search engine learns to distrust it; or
  `page.modified` ([0082](./0082-the-dates-of-a-site-and-its-pages.md)),
  when git says the page's file last changed, and nothing where it can't.
- **`<changefreq>` and `<priority>`:** a setting for each, for search
  engines to ignore; or neither.
- **A site with versions:**
  - a sitemap for each version, in its folder, of its own pages, which is
    what each version's own plugins write with no code for it, and how the
    protocol scopes a sitemap;
  - an index at the root, listing each version's sitemap, so a site gives
    a search engine one address. But a file is an index or a sitemap, not
    both, so the pages of the version at `/` would go in a second file,
    and a site's files would differ with and without versions; and the
    plugin at `/` can't tell whether another version's is on;
  - or one for the version at `/` alone, which is code to do less, and
    takes away a choice a version's entry already has, in its `plugins`
    ([0043](./0043-a-versions-own-nav-and-plugins.md)).
- **Leaving pages out:** a list of URLs in the plugin's settings, as
  `site.search.exclude` is
  ([0061](./0061-leaving-pages-out-of-search.md)), though each version's
  plugin sees only its own pages, so it couldn't warn only of a URL no
  version has; a key in frontmatter, where a page's content shouldn't know
  what builds it ([0083](./0083-redirects-from-old-urls.md)); or neither,
  as a sitemap is a hint, and leaving a page out of it doesn't keep a
  search engine from the page.
- **A `robots.txt`,** whose `Sitemap` line is how a search engine finds a
  sitemap unasked: written by the plugin, for a site at the root of its
  host that has none, as Jekyll's sitemap plugin does; or left to the
  site, as it's the site's file, which says more than where a sitemap is,
  counts only at the root of a host, and can't be added to by a
  `destination` hook once it's there.
- **A site without a `url`:** addresses without a scheme and host, which
  the protocol doesn't allow; a warning, and no sitemap, which a build
  that isn't `--strict` scrolls past; or a build that stops, and says
  what to set.
- **How it's written:** by hand, for five elements; or with crates little
  builds with already: quick-xml, which escapes what it writes; `url`,
  which little reads `site.url` with, so both agree on its host; and
  `time`, which little's `dateformat` reads dates with.

## Decision
- `plugins/sitemap/` is the plugin, a crate, `little-sitemap`, in its own
  Cargo workspace, which builds `little_sitemap.wasm`:

  ```sh
  cargo build --release --target wasm32-wasip2 --manifest-path plugins/sitemap/Cargo.toml
  ```

  It's named for what it writes. It exports `setup`, `site`, `page`, and
  `destination`.
- A site names it, and sets its `url`:

  ```yaml
  site:
    url: https://example.com/docs
    plugins:
      sitemap:
        wasm: ./plugins/sitemap/target/wasm32-wasip2/release/little_sitemap.wasm
  ```

  It has no settings, like `jupyter`
  ([0060](./0060-other-formats-in-plugins.md)), and one it's given stops
  the build.
- In `site`, it reads `site.url`, and keeps its scheme and host, as little
  reads them: the host in lowercase, without a port that's its scheme's
  own. A site without a `url` stops the build:
  ``plugin `sitemap`'s `site` hook failed: a sitemap lists each page's whole address, so set `site.url` to where the site is published, like https://example.com/docs``.
- In `page`, it notes each page's `url` and `modified`, but those of the
  404 page and of redirects, by their `type`, and gives `page` back as it
  was.
- In `destination`, it adds `sitemap.xml`, which lists each page it noted,
  in order of URL, so a site builds the same sitemap each time:

  ```xml
  <?xml version="1.0" encoding="UTF-8"?>
  <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
    <url>
      <loc>https://example.com/docs/guide/install</loc>
      <lastmod>2026-11-02T21:05:10Z</lastmod>
    </url>
  </urlset>
  ```

  - `<loc>` is the scheme and host, then the page's `url`, which has the
    path of the site's `url`, and of its version, already. What a URL
    can't hold, like a space, is encoded as a browser encodes a link, and
    the whole is escaped for XML.
  - `<lastmod>` is the page's `modified`, where it's a date as
    `dateformat` reads one
    ([0082](./0082-the-dates-of-a-site-and-its-pages.md)): a date and
    time, as RFC 3339 writes it, or a date alone. A page without one is
    listed without it, and one whose `modified` is anything else is too,
    and warned of, by its file.
  - It has no `<changefreq>` or `<priority>`.
- A page whose `url` isn't from the root of the site, as its frontmatter
  can make it ([0006](./0006-frontmatter-and-config-win.md)), is warned
  of, and left out.
- More than 50,000 pages stop the build, which says so. So does a crash:
  little starts the plugin again, without the pages it had noted
  ([0046](./0046-plugin-hooks.md)), and it writes no part of a sitemap.
- On a site with versions, each version has its own sitemap, in its
  folder, like `/1.4/sitemap.xml`, and a version's entry turns it off,
  with `plugins: { sitemap: { enabled: false } }`.
- It writes no `robots.txt`: its page in the manual shows the line a site
  adds to its own.
- The manual names the plugin, so it has a sitemap, and
  `.github/workflows/docs.yml` builds it, and fetches the repository's
  whole history, so the manual's sitemap says when each page last changed.
  Its page, "Sitemap", is `docs/plugins/sitemap.md`, listed after "Writing
  plugins", before the pages on formats
  ([0071](./0071-ordering-and-naming-the-manuals-plugin-pages.md)), as the
  one plugin of little's that any site can use.
- `sitemap` is a scope for commits
  ([0051](./0051-semantic-versioning-and-conventional-commits.md)), and
  it's released from tags like `sitemap-v0.1.0`, with notes that list the
  commits to its folder ([0052](./0052-releasing-plugins-from-tags.md)).

## Consequences
- A public site gets a sitemap from three lines of `little.yml`, built
  offline, in the sandbox, the same each time. little, its theme, and
  `wit/plugin.wit` don't change, so `little:plugin` stays 0.2.1, and
  little needs no release.
- `site`, `page`, and `destination` have a plugin of little's own to
  prove them, and to show how a plugin keeps what it notes from one hook
  for another.
- A build is no slower for it. On the site `scripts/bench.sh` makes, of
  2,041 pages, three builds took 8.2 to 8.8 s without the plugin, and 8.5
  to 8.8 s with it, on an Apple silicon Mac. `little serve` still renders
  only the pages that differ, as the plugin has no `html` hook.
- A site whose source has a `sitemap.xml` of its own doesn't build with
  the plugin, as no `destination` hook adds a file where there's one
  ([0046](./0046-plugin-hooks.md)).
- A theme that names the plugin needs each of its sites to set `url`, or
  to turn the plugin off.
- A site's CI fetches its repository's whole history for its sitemap to
  have dates, as for a theme that shows them
  ([0082](./0082-the-dates-of-a-site-and-its-pages.md)).
- The plugin is about 420 KB.
- Limits, for now:
  - a page whose frontmatter sets `url`, which doesn't move the page, is
    listed at that URL, as the plugin knows only what the theme gets;
  - a site can't leave a page out of its sitemap, but by leaving it out of
    the site;
  - there's no index, so a site gives a search engine the sitemap of each
    version it wants read, and a version of more than 50,000 pages can't
    use the plugin;
  - an `.html` file that's copied isn't listed, as it's no page;
  - a `%` in a page's URL is written as it is, as in little's own links;
  - it lists no images, videos, or translations.
