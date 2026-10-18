# 0021 — Search's files go in `pagefind/`

- Status: accepted
- Date: 2026-10-03

## Context
little wrote the files that search a site to `search/`, named for what
they're for, not what makes them ([0019](./0019-site-search.md)). Pagefind
writes them to `pagefind/`, and its docs and search boxes expect them there.
So a theme couldn't use Pagefind's markup as it is: it had to tell the search
box where the index was, with `<pagefind-config bundle-path>`.

Pagefind's search box can find the index itself, beside its own script, from
the script's URL. But it can only do that if its script runs once the
page's search box is on the page, as a deferred script does. Loaded as a
module, as Pagefind's docs show, it can't tell where its script came from,
and looks in `/pagefind/` at the root of the domain. On a site under a path,
like `https://withlittle.github.io/little`, that's the wrong place, whatever
the folder is called.

Pagefind also puts a path before each result's URL: the one above the
folder it finds the index in, like `/little/` for `/little/pagefind/`. The
index in `search/` wasn't in a folder named `pagefind`, so it put `/` there,
and little indexed each page at its URL with the site's path. In
`pagefind/`, that path would come twice, as `/little/little/guide/install`.

## Options considered
- **`search/`**, as it was, with `bundle-path` in every theme with a search
  box.
- **`pagefind/`, with the script as a module**, as Pagefind's docs show:
  Pagefind's markup works as it is at the root of a domain, but a site under
  a path still needs `bundle-path`, so every theme would keep it.
- **`pagefind/`, with the script deferred:** the box finds the index under
  any path, with nothing to set.
- **Each page's URL in the index:** with the site's path, as it was, and
  `<pagefind-config base-url="/">` in every theme, so Pagefind adds nothing;
  or from the root of the site, as Pagefind's own CLI indexes a site, so
  Pagefind adds the path itself.

## Decision
- little writes search's files to `pagefind/` in the destination, like
  `dist/pagefind/pagefind.js`, as Pagefind does.
- The built-in theme loads Pagefind's search box with `defer`, not as a
  module, and has no `<pagefind-config>`: the box finds the index beside its
  script, under the path of the site's `url` too.
- Each page is indexed at its URL from the root of the site, like
  `/guide/install`, not with the site's path, as 0019 had it. Pagefind puts
  the path back, from where it finds the index, so on a site at
  `https://withlittle.github.io/little`, a result links to
  `/little/guide/install`.
- A page, or a file in the source or a theme, where search writes a file, or
  a folder of them, still stops the build, naming both, but now in
  `pagefind/`. A page at `/pagefind`, from `docs/pagefind.md`, doesn't clash,
  and nothing in `search/` does any more.

## Consequences
- A theme's search box is Pagefind's own markup, with `defer` in place of
  `type="module"` and its URLs through the `url` filter, and nothing to
  configure.
- A box of a theme's own, on Pagefind's JavaScript API, still gets each
  result's `url` with the site's path, as long as it loads `pagefind.js`
  from where little writes it.
- A theme that loads the script as a module, or before its search box
  without `defer`, still needs `bundle-path` on a site under a path. That's
  noted in the README, not handled.
- A theme that loads its search box from `/search/` has none until it moves
  to `/pagefind/`. Search shipped the day before, so only the built-in theme
  did, and little keeps no alias.
- The folder now names the search engine, which 0019 avoided. Every theme's
  search box is Pagefind's already, so if little ever searched with
  something else, they'd all change anyway.
