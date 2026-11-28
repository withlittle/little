# 0089 — A page's `view` and `edit` links to its repository

- Status: accepted
- Date: 2026-11-28
- Amends: [0020](./0020-links-to-a-pages-file-in-its-repo.md),
  [0080](./0080-settings-in-groups-under-site-and-page.md),
  [0087](./0087-each-pages-file-copied-beside-it.md)

## Context
Each page gets two links to its file in the site's repository:
`page.repository.source_url`, to view it there, and
`page.repository.edit_url`, to edit it
([0020](./0020-links-to-a-pages-file-in-its-repo.md),
[0080](./0080-settings-in-groups-under-site-and-page.md)). The group's
`enabled` turns off both, so a site can't keep one without the other: one
that takes no changes through its repository's host shows "Edit this page"
on every page, or links to no page's file.

Theirs are the last names that end in what they hold, where 0080's first
rule has a group: a page has `previous.url`, not `previous_url`. And since
[0087](./0087-each-pages-file-copied-beside-it.md), `page.source` is the
page's file, copied to the site, so `page.source.url` and
`page.repository.source_url` are one file in two places. 0087 left the
second to be renamed.

## Options considered
- **Where the links' settings go:** in `site.repository`, beside its `url`
  and `branch`; or in `page.repository`, as what a page has is every
  page's setting, which a `little.yml` sets under `page`, and a page in its
  frontmatter (0080).
- **A group for each link:** `source_url` and `edit_url`, as they are; or a
  group each, with its `url`, and an `enabled` of its own.
- **Their names:** `source` and `edit`, which names one link by what it
  leads to, and the other by what a reader does there, and keeps a `source`
  in `page.repository` beside `page.source`; or `view` and `edit`, as both
  lead to the same file.
- **`enabled` on `repository` itself:** kept, over the two, to turn off
  both at once; or dropped, as it's a second way to write both off, which
  needs a rule for which wins.
- **`repository.enabled`, `source_url`, and `edit_url`, as they were
  written:** read; stopped at, with what to write; or neither, as 0080 has
  it for every name from before.

## Decision
- `page.repository` holds two groups, `view` and `edit`: a page's links to
  where its file is viewed, and where it's edited, in the site's
  repository. little's `little.yml` sets each one's `enabled` to `true`,
  and little works out each one's `url`, as it did `source_url` and
  `edit_url`: on github.com and gitlab.com, for a page it reads from a
  file, and, for `edit`, not in a version from a ref (0020).
- A link's `enabled: false` leaves out its `url`, whatever else the page's
  settings write there, and leaves the other link as it is: for every
  page, under `page` in a `little.yml`, or for one, in its frontmatter.
- `page.repository` has no `enabled` of its own. A page has neither link
  with both turned off.
- Every page has both groups, the 404 page too, so a theme can read
  `page.repository.edit.url` on any page.
- A page's settings have the last word on what its theme gets, as before:
  a `url` they write is the link's, with the site's path, if it's from the
  root of the site.
- `site.repository` is as it was: its `url` and `branch`, and the `owner`
  and `name` little reads from its `url`.
- The built-in theme reads the new names, and shows what it did.
- little reads no name from before (0080). `repository.enabled`,
  `source_url`, and `edit_url` are keys it doesn't know, which go to the
  theme, as any does.
- `wit/plugin.wit` doesn't change, so `little:plugin` stays 0.2.1. No new
  crates.

## Consequences
- A site links each page to its file in its repository without offering to
  edit it there, or the other way round.
- A page's `source` is one thing, its file, copied to the site. Where that
  file is in the repository is `page.repository.view.url`.
- Every theme that links to a page's file in its repository changes, and
  so does a `little.yml`, or a page, that turns the links off, so little's
  minor version rises. A plugin's `page` hook is given the new names.
- A `little.yml`, or a page, that still writes
  `repository: { enabled: false }` has both links, and a theme that still
  reads `source_url` or `edit_url` shows neither. Nothing says so.
- Turning off both links takes two settings.
- Limits, for now: only github.com and gitlab.com get either link, as in
  0020. A pattern for each, in `site.repository`, could give them to other
  hosts.
