# 0087 — Each page's file, copied beside it

- Status: accepted
- Date: 2026-11-27
- Amends: [0015](./0015-files-and-relative-links.md),
  [0046](./0046-plugin-hooks.md),
  [0060](./0060-other-formats-in-plugins.md),
  [0080](./0080-settings-in-groups-under-site-and-page.md)

## Context
People who build for little, its themes and plugins, mostly work through
AI coding agents now, and so do many who read a site little builds. Two
agents were each given only little, a site, and the address of the manual,
and asked for a theme and a plugin. Both built what was asked, but each
first wrote a script to turn the manual's HTML back into text, as the tool
they fetch pages with sums a page up, and loses its code.

A site's pages are already written as Markdown, which reads better than
HTML to an agent, and to some people. little copies every file in the
source to the site but those it builds pages from
([0015](./0015-files-and-relative-links.md)), so the one form of a page
that needs no work to publish is the one it leaves out. Sphinx copies each
page's source to `_sources/`, and its themes link to it, unless a site
turns that off.

A plugin can't do it alone. Its `source` hook is given each page's file,
and its `page` hook each page's URL, but nothing pairs the two, short of
working out little's URLs again.

## Options considered
- **What to publish:** each page's file, as it's written; or Markdown made
  from the page as it's built, which would cover a page a plugin adds, but
  is a second thing to write, and isn't what the author wrote.
- **Where:** at the file's own path, beside its page, as other files are,
  where the links between files still lead to each other; in a folder of
  their own, as Sphinx has; or at each page's URL with `.md`, which is the
  file's own path for every page but a folder's.
- **What a source is, for a page a plugin makes:** the Markdown the plugin
  gives little, which is made from something else; the one file a page is
  made from, if there is one, like an OpenAPI document; or nothing.
- **Who knows that file:** little, which sees only what a plugin gives it,
  and can't tell a page made from one document from one made from a
  package of code; or the plugin.
- **Who leaves out a file a plugin adds as a page's source, with
  `source` off:** the plugin, which isn't given a site's settings in a
  version from a ref; a setting of the plugin's own, a second switch to
  forget; or little, which knows each page's settings.
- **On or off:** off, until a site asks; on in the built-in theme, as
  search is, though a file is worth reading whatever the theme; or on in
  little, as each of a page's other groups is.
- **Its name:** `page.source`, though `site.source` is the folder a site is
  built from, and `page.repository.source_url` the same file in its
  repository; or another word.

## Decision
- A page's source is the one file it's built from. little knows it for a
  page it reads from a file in the source: a Markdown file, or one a
  plugin's `formats` hook reads, like a notebook.
- little copies that file to its path in the source, as it's written,
  whatever a plugin's `source` hook makes of its text: from
  `docs/guide/install.md` to `dist/guide/install.md`, beside
  `guide/install.html`. So a page's Markdown is at its URL with `.md`, and
  a folder's own page's at `README.md` in the folder.
- `page.source` is a group of every page's settings, as
  [0080](./0080-settings-in-groups-under-site-and-page.md) has them.
  little's `little.yml` sets its `enabled` to `true`, and little works out
  its `url`, where the file is copied to, with the site's path, like
  `/my-project/guide/install.md`. A version's is in its own folder, from
  its ref.
- `enabled: false` leaves the file out, and its `url`, whatever else the
  page's settings write there: for every page, under `page` in a
  `little.yml`, or for one, in its frontmatter.
- A page a plugin's `files` hook adds has no file, so none is copied, and
  it has no `url`. The 404 page and a redirect have none either.
- A page's settings have the last word on what its theme gets, as for any
  group, so a `source.url` they write is what the theme gets, with the
  site's path. That's how a plugin names the one file its page is made
  from, in the frontmatter of the page it adds.
- Where the plugin adds that file too, it's the page's source, as the file
  little reads a page from is that page's. little leaves it out unless a
  page that names it has its `source` on, so the one setting covers every
  page's source. A file of the site's own that a page names stays, as any
  file in the source does, and so does one another plugin adds.
- The built-in theme links to a page's file in its `<head>`, as
  `<link rel="alternate" type="text/markdown">`, if it ends in `.md`.
- The names stay as they are: `site.source` and
  `page.repository.source_url` may be renamed later.
- `wit/plugin.wit` doesn't change, so `little:plugin` stays 0.2.1. No new
  crates.

## Consequences
- Every site little builds can be read as Markdown, with nothing to set,
  and the manual is. A plugin can list a site's pages for an agent, like
  an `llms.txt`, from what its `page` hook is given.
- A site's files are published as they're written: frontmatter a theme
  doesn't show, and what a page doesn't, like a Markdown comment. A site
  that keeps either private turns `source` off.
- A page whose frontmatter has a `source` of its own, which isn't a group,
  stops the build, so little's minor version rises.
- A destination has a file more for each page, and a plugin's
  `destination` hook is given each.
- An API reference a plugin adds has no Markdown to read, until its plugin
  names a source for it.
- GitHub Pages builds a branch with Jekyll, which makes a page of its own
  from each `.md` file, unless the branch has a `.nojekyll` file. The
  manual's workflow publishes with GitHub Actions, which doesn't.
- A page built to the path of its own file, as one a plugin read from an
  `.html` file would be, can't have the file copied there, which stops the
  build, naming both, until its `source` is off. None of little's plugins
  reads one.
- On a site with versions, a theme's file at the path of a page's file is
  the one a link to that path leads to from a version that isn't at the
  root. The page's `url` still leads to its own.
