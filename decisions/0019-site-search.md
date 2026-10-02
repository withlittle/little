# 0019 — Searching a site with Pagefind

- Status: proposed
- Date: 2026-10-02

## Context
A reader who knows what they're looking for, but not where it is, has only
the nav, or a search engine that may not have the site. mdBook, MkDocs,
Sphinx, and Starlight each search a site out of the box, and search is slice
4 of little's [roadmap](../VISION.md). A static site has no server to search,
so its index is built with the site and searched in the browser. It mustn't
need Node, and the theme owns the markup, so the search box is the theme's.

## Options considered
- **What searches:**
  - [Pagefind](https://pagefind.app/), in Rust, which Starlight uses, with an
    [API](https://crates.io/crates/pagefind) for building it into another
    Rust program. It splits its index into small files, so a search
    downloads only the parts it needs, even on a large site; it stems words
    in about 30 languages and finds the headings within a page; and its
    search box works by keyboard and with a screen reader.
  - One JSON file of every page, which little writes, searched by a small
    script in the theme, as mdBook and MkDocs do with elasticlunr and lunr:
    nothing to add to little, but a reader downloads the whole index, which
    grows with the site, before their first search, and the search box and
    the script that searches are code little or each theme would keep.
  - tinysearch needs Rust's WASM toolchain to build each site's index, and
    Stork is no longer maintained.
- **What's indexed:** each page as its theme renders it, or each page's own
  content, as little renders it from the Markdown, under its title. Pagefind
  skips a page's `<nav>` and `<footer>`, but indexes the rest of a theme's
  markup, like its header, on every page, unless the theme marks its content
  with Pagefind's attributes, like `data-pagefind-body`, which ties every
  theme with search to Pagefind. A page's own content is what its author
  wrote, and nothing a theme adds, with nothing for a theme to mark.
- **Where its files go:** `pagefind/`, as Pagefind names it, or `search/`,
  named for what it's for, not what makes it.
- **Which sites have it:** every site, or each one whose theme has a search
  box, as without one the index is never read.
- **The search box:** the theme's, Pagefind's own or one built on Pagefind's
  script; or one built on a small API little owns in JavaScript, which wraps
  Pagefind, so no theme names it. The API frees themes from Pagefind, but
  it's code little keeps, and the built-in theme would need a search box of
  its own.

## Decision
- `search` is a setting in the cascade. little's `little.yml` sets it to
  `false`, and the built-in theme's to `true`, as it has a search box. A site
  sets `search: false` to go without, and a theme reads it as `site.search`.
- When `search` is on, `little build` and `little serve` index the site with
  Pagefind, built in, and write its files to `search/` in the destination,
  like `dist/search/pagefind.js`.
- What's indexed is each page's own content, rendered from its Markdown,
  under its title and its URL with the site's path
  ([0014](./0014-site-url-and-its-path.md)), like `/little/guide/install`.
  The theme's markup around it isn't, nor is the 404 page, so a theme has
  nothing to mark, and nothing it puts around a page, like its header or its
  table of contents, turns up in results.
- Each page is indexed in the language its theme gives it with `<html lang>`,
  so in English, "installing" finds "install". A page without one is indexed
  without stemming.
- A search finds the headings within a page, by the `id` little gives each
  one, as well as the page.
- The theme owns the search box: Pagefind's, or one of its own on
  `search/pagefind.js`, loaded through the `url` filter. Pagefind's box
  can't find its own folder when it's loaded as a module, so a theme points
  it there with `<pagefind-config bundle-path="{{ '/search/' | url }}">`.
- The built-in theme puts Pagefind's search box in its header when
  `site.search` is on, opened with ⌘K or Ctrl+K, and styled to match.
- A file in the source or a theme at the same path as one of the search's
  files stops the build, naming it. A page at `/search`, from
  `docs/search.md`, doesn't clash.
- New crate: `pagefind`, without its default features, which add a web
  server.

## Consequences
- A site is searchable out of the box, with nothing to install.
- little grows from about 25.7 MB to about 38.5 MB, and from about 492 crates
  to about 550. About 3 MB of that is the scripts and WASM Pagefind writes
  into sites.
- A site carries `search/`: about 0.7 MB for little's example site of 5
  pages, and 3.2 MB for one of 457. A reader's browser downloads the
  built-in theme's search box once, about 45 KB compressed, about 100 KB
  more for their first search, and then only the parts of the index each
  search needs.
- A build takes about a second longer for every 450 pages, and so does each
  save in `little serve`.
- little's part, `search`, `search/`, and what's indexed, names no search
  engine. A theme's search box is Pagefind's, though, so if little ever
  searched with something else, every theme with a search box would change
  with it. The built-in theme ships with little, so it always matches the
  Pagefind that built the index.
- Limits, for now:
  - text a theme adds to a page, like a banner on its home page, isn't
    searchable, nor is an HTML file copied from the source;
  - Chinese, Japanese, and Thai, which don't put spaces between words,
    search poorly: splitting them needs Pagefind's extended build, which is
    tens of MB larger;
  - Pagefind writes two older search boxes too, about 160 KB a theme may
    never load;
  - a page can't leave itself out of search.
