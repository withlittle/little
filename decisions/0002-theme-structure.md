# 0002 — Theme structure and template lookup

- Status: accepted
- Date: 2026-09-27

## Context
A theme had to have `templates/page.html`, and it was the only template little
loaded: every page got it, and it couldn't `{% extends %}` or `{% include %}`
another. `site.rs` hard-coded an `assets/` folder, copied to `dist/assets/`. A
theme without `page.html` was only noticed after `dist/` had been wiped.

We want a first theme to be one file, a real theme to be able to lay out its
home page, sections, and 404 page differently, and the structure to feel
familiar to anyone coming from Jekyll or Sphinx.

## Options considered
- **Folder names:** `templates/` + `static/` (Zola, Hugo), or `_templates/` +
  `_static/` (Sphinx; Jekyll's `_layouts/`). Underscores sort a theme's core
  folders to the top.
- **Where static files go:** the site's root (Zola, Hugo, Docusaurus), or under
  a fixed prefix like `/_static/` (Sphinx) or `/theme/` (Pelican). The root lets
  a theme ship `favicon.ico` or `robots.txt`, and choose its own URLs.
- **The one required template:** `base.html` is a Jinja convention for a layout
  others extend, not one a generator renders. `index.html` is the fallback in
  WordPress block themes, but the home page in Zola and Pelican. `default.html`
  is Jekyll's name for the layout that pages fall back to.
- **Choosing a page's template:** frontmatter (`layout:` in Jekyll), or the
  type of page. By type, authors write nothing and themes decide. Jekyll's
  jekyll-default-layout plugin, on by default on GitHub Pages, does the same:
  `home`, then `page`, then `default`. Hugo's catch-all `all.html` is the same
  idea as `default.html`.
- **Folders without a `README.md`:** generate a section page for them (Hugo,
  Zola), or build one only where an author writes a README.

## Decision
- A theme is `_templates/` (required), `_static/` (optional), and `little.yml`
  (optional). `_templates/default.html` is required.
- A page's template is chosen by its type, falling back to `default.html`:

  | Type | Pages | Template |
  | ---- | ----- | -------- |
  | `home` | `docs/README.md` | `home.html` |
  | `section` | a folder's `README.md` | `section.html` |
  | `page` | any other `.md` file | `page.html` |
  | `404` | a URL with no page | `404.html`, with no fallback |

  `section`, not `index`, since `index.html` means the home page elsewhere.
  A 404 page has no content for `default.html` to show, so a theme without a
  `404.html` gets no `dist/404.html`, and the host shows its own.
- Templates get `page.type`; home and section pages also get `page.children`,
  their folder's branch of `site.nav`. A section page exists only where an
  author writes a `README.md`; other folders stay labels in the nav.
- Anything a page doesn't have is left out, never null: MiniJinja prints null
  as `None`, and `default` doesn't replace it. A 404 page's `page` has only
  `type` and `url`.
- `_static/` is copied as-is to the root of `dist/`. A static file that would
  take the place of a page is a build error.
- A build renders every page before it touches `dist/`, so one that fails
  leaves the last build in place.

## Consequences
- The smallest theme is one file, `_templates/default.html`, and templates can
  extend and include any others in `_templates/`.
- New types of page can come later without breaking themes: they fall back to
  `default.html`.
- Existing themes must move `templates/page.html` to `_templates/default.html`
  and `assets/` to `_static/assets/`; little names the missing file.
- 404 pages are opt-in, per theme.
- A template error fails the whole build, but `little serve` keeps serving the
  last good one.
