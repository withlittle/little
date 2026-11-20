# 0084 — Redirects from a folder

- Status: accepted
- Date: 2026-11-20
- Amends: [0083](./0083-redirects-from-old-urls.md)

## Context
`site.redirects` lists each old URL
([0083](./0083-redirects-from-old-urls.md)), so a folder that's renamed is
a line for each page in it, which 0083 took as a limit, for now. A folder
is renamed as often as a page is, and its list goes out of date as pages
are added to it.

A redirect is a page little writes, as a static site has no server to
match a URL as it's asked for. A redirect from a folder is a page for
each URL under it, which little has to list as it builds.

## Options considered
- **In little, or in a plugin:** a plugin's `config` hook can change
  `site.redirects` ([0046](./0046-plugin-hooks.md)), but it's called
  before the pages are read, so it can't list a folder's.
- **How it's written:**
  - a `/` at the end of a URL, for its folder: but `/news` and `/news/`
    are the same URL (0083);
  - a setting beside `to` and `from`, like `folder: true`, which is one
    more to learn;
  - or a `/*` at the end of each, as hosts' own redirects are written,
    like Netlify's and Cloudflare's `_redirects`.
- **Which URLs get a page:** those the old folder had, which little can't
  know, as its pages are gone; or one for each page the new folder has
  now. A page added since gets one it doesn't need, which costs a small
  file.
- **A URL the pattern covers where there's a page or a file:** an error,
  as for a redirect from one URL (0083); or the page stays, as some of a
  folder's pages may not have moved, and a pattern can't list which.
- **A URL that's listed on its own, too:** an error, as for two redirects
  from one URL; or the one that's listed wins, as a page may be renamed
  as its folder is.
- **A folder on another site:** little has no pages to list there.

## Decision
- A redirect whose `to` and each `from` end in `/*` is from every page
  under a folder, to the same page under another:

  ```yaml
  site:
    redirects:
      - to: /tools/*
        from:
          - /widgets/*
  ```

- little writes a redirect for each page under `to`'s folder, the
  folder's own page too, from where the page would be under each `from`:
  `/widgets/this` leads to `/tools/this`, and `/widgets/` to `/tools/`.
  Each is a `redirect` page, as any other is, so nothing changes for a
  theme or a plugin. Only pages get one, not a folder's other files.
- `/*` is every page of the site, as a `from` or as a `to`.
- It gives way, at each URL, to a page or file that's there, on any
  site, and to a redirect from that URL alone.
- On a site with `versions`, each version has one for each of its own
  pages, as it has the other redirects it can.
- These stop the build, and each error says what to write:
  - a `*` anywhere else in a `from`, or in the path of a `to` in the
    site, which can't name a file on every system;
  - a `to` that ends in `/*` with a `from` that doesn't, or a `from` that
    does with a `to` that doesn't, or that's on another site;
  - on a site without `versions`, one that leads to a folder with no
    pages. On a site with them, one that no version writes a page for is
    warned of, as for any redirect.
- The checks 0083 has still hold: one to itself, one that leads to
  another's `from`, like `/a/*` to `/b/*` and `/b/*` to `/c/*`, and two
  from one URL, which clash when they're written.
- No new crates.

## Consequences
- A folder that's renamed is one entry, which stays right as pages are
  added to it.
- A page that's deleted from the new folder loses its redirect, so its
  old URL is not found, as its new one is.
- A `from` with a `*` in a file's name, which 0083 allowed, is now an
  error. 0083 isn't released, so no site has one.
- Limits, for now:
  - every page under the old folder leads to the same path under the new
    one: a page that's renamed too is listed on its own;
  - a folder that's moved to another site is still a line for each page;
  - a folder's other files, like images, aren't redirected.
