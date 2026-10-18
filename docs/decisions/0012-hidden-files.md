# 0012 — Hidden files and folders are left out

- Status: accepted
- Date: 2026-10-01

## Context
Every `.md` file in the source became a page, hidden ones too. Jupyter keeps
a copy of each file it opens in a hidden `.ipynb_checkpoints/` folder beside
it, so a site edited in Jupyter got a page, and a section in its nav, for
each copy. With `source: .`, the Markdown in `.github/`, or in a theme
downloaded into `.little/`, became pages too
([0004](./0004-downloading-themes.md)). And `little serve` rebuilt the site
whenever anything in the source changed, an editor's swap file or `.git/`
included.

## Options considered
- **Which names are hidden:** those that start with `.`, as Unix hides them,
  and as MkDocs (by default), Hugo, and Jekyll leave them out; or those that
  start with `_` too, as Jekyll leaves them out, since its own folders are
  named so. Hugo leaves out Emacs's and Vim's backups too (`#page.md#`,
  `page.md~`), but those don't end in `.md`, so they were never pages.
- **Where the rule applies:** to the source, or to a theme's `_static/` too.
  A theme can ship hidden files a host reads: GitHub Pages reads `.nojekyll`,
  Apache reads `.htaccess`, and other services look in `/.well-known/`.
- **Which names count:** every name in a file's path, or only those inside
  the source. The source can be a hidden folder, like `.github`, and a site
  can be kept in one, and neither makes its pages hidden.
- **`little serve`:** rebuild for any change in the source, as before, or not
  for a hidden file's, since a build leaves it out.

## Decision
- A file or folder in the source whose name starts with `.` is hidden. A
  hidden file isn't a page, and nothing in a hidden folder is.
- Only names inside the source count, so the source can be hidden itself, as
  can the folders it's in.
- A theme's `_static/` is still copied whole, hidden files included.
- `little serve` doesn't rebuild when only hidden files in the source change.

## Consequences
- Jupyter's checkpoints, an editor's hidden swap files, and anything an author
  hides, like a `.drafts/` folder, stay out of the site.
- With `source: .`, `.git/`, `.github/`, and `.little/` are left out, so a
  downloaded theme's Markdown no longer becomes pages.
- A page can't be published from a hidden folder in the source. To publish
  one, rename the folder, or make it the source.
- Saving a hidden file in the source doesn't rebuild the site, but renaming
  one over a page does, as some editors save that way.
