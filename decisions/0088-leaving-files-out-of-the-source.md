# 0088 — Leaving files out of the source

- Status: accepted
- Date: 2026-11-28
- Amends: [0012](./0012-hidden-files.md),
  [0015](./0015-files-and-relative-links.md),
  [0042](./0042-versions-share-the-sites-config-and-theme-files.md),
  [0046](./0046-plugin-hooks.md),
  [0080](./0080-settings-in-groups-under-site-and-page.md),
  [0087](./0087-each-pages-file-copied-beside-it.md)

## Context
little builds a page from each Markdown file in a site's source, and
copies every other file there
([0015](./0015-files-and-relative-links.md)), and each page's own file too
([0087](./0087-each-pages-file-copied-beside-it.md)). The only way to keep
a file out of a site is to hide it, with a `.` before its name
([0012](./0012-hidden-files.md)). So a folder of drafts, or a file written
for the repository, is renamed, or kept somewhere else, and a site whose
source is its own folder publishes its `little.yml` and its theme, as 0015
noted.

Sphinx has `exclude_patterns`, MkDocs `exclude_docs`, and Jekyll `exclude`.
VitePress has `srcExclude`, beside its `srcDir`.

`site.source` is the path of the folder, so a second setting has nowhere to
go: no value stands in for a group
([0080](./0080-settings-in-groups-under-site-and-page.md)). And 0087 gave
every page a `source` that's a group, and left `site.source`, a path, to be
renamed later.

## Options considered
- **Where the list goes:** beside `source`, under a name that starts as it
  does, as VitePress has it; or in a group, with the folder, as 0080's
  rules have it.
- **The group's name:** another word, as `page.source` is a page's file; or
  `source` for both, as a page's is the file it's built from, and a site's
  the files it's built from.
- **The folder's name in it:** `folder`, the manual's word; `path`; or
  `directory`, as programming languages, command-line tools, and operating
  systems name it, like `mkdir` and `readdir`. `folder` is a desktop's
  word.
- **`destination`:** a group too, so the two folders are written alike,
  though it has one setting; or a path, as `site.url` is an address.
- **What an entry is:** a pattern, as a `.gitignore` has, and the other
  generators take; or a path, with all that's under it, as
  `site.search.exclude` takes a URL
  ([0061](./0061-leaving-pages-out-of-search.md)).
- **What leaving a file out does:** builds its page, but doesn't copy its
  file, as a page left out of search is still built; or doesn't read it,
  as a hidden file isn't.
- **A path that leaves nothing out:** nothing; or a warning, as for a URL
  in `site.search.exclude` that matches no page.
- **A setting the group hasn't, like `excludes`:** passed to the theme, as
  most of `site` is; or an error, as in `site.search`, as it would leave
  nothing out.
- **A version's own `source`:** in place of the one at its ref, as before;
  or over it, setting by setting, as its `plugins` go over the site's.
- **`source: ./docs`, as it was written:** read as the folder; or not, as
  0080 reads no form from before.

## Decision
- `site.source` is a group, of `directory`, the folder a site is built
  from, which little sets to `./docs`, and `exclude`, a list of paths,
  which little leaves empty. Any other setting there stops the build.
- Each path in `exclude` is from the root of the directory, and starts with
  `/`, like `/guide/old.md` or `/drafts/`. It leaves out the file at that
  path, or the folder, with everything in it, written with the `/` at its
  end or without. It's matched as it's written, so `/Drafts/` doesn't leave
  out `drafts/`. Anything else stops the build, as does `/` alone, which
  would leave out all of the source.
- A file that's left out is as a hidden one is: not a page, not copied, not
  among the paths a plugin's `files` hook is given, and `little serve`
  doesn't rebuild when only such files change. `exclude` is about the files
  in the directory, so a file a plugin adds stays.
- little warns of a path that leaves nothing out, as it may be mistyped,
  which publishes the file it meant. On a site with versions, it warns of
  one only if it leaves nothing out in every version that lists it.
- A version from a ref has the `site.source` of the `little.yml` at its
  ref, over little's and the themes', or the site's, without one, as in
  [0042](./0042-versions-share-the-sites-config-and-theme-files.md). The
  `source` in its entry is the same group, and goes over that, setting by
  setting.
- `site.destination` stays a path, and `site.theme.source` a path or a URL,
  as a plugin's `wasm` is.
- A plugin's `config` hook can't change `site.source`, as before: neither
  setting. In `wit/plugin.wit`, only a comment changes, so `little:plugin`
  stays 0.2.1.
- little reads no form from before. A path where the group goes stops the
  build, as any value there does.
- No new crates.

## Consequences
- A site keeps its drafts, or a file written for its repository, beside its
  pages, and out of the site, without renaming either. One whose source is
  its own folder can leave out its `little.yml` and its theme.
- Every `little.yml` that sets `source` changes, and every version's entry
  that does, so little's minor version rises.
- A link to a file that's left out is broken, and `little build` warns of
  it, as of any.
- A list replaces the one beneath it, as in any `little.yml`, so a site's
  `exclude` replaces its theme's.
- A `little.yml` at a ref that sets `source` to a path stops its version's
  build, as the site's would, unless the version's entry sets its own
  `source`.
- Limits, for now:
  - `exclude` takes no patterns, like `*.tmp`. Each path it takes means
    what it would in a `.gitignore`, so they could be added;
  - a theme can still set `site.source`, as 0080 noted, and so its
    `exclude`.
