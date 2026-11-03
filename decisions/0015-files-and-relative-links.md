# 0015 — Images and other files are published, and relative links work as on GitHub

- Status: accepted; amended by [0017](./0017-links-to-pages-files.md),
  which leads a link to a page's file, like `install.md`, to the page
- Date: 2026-10-01
- Amends: [0014](./0014-site-url-and-its-path.md)

## Context
little built a page from each `.md` file in the source and left every other
file out, so the images kept beside the pages, like
`docs/guide/images/setup.png`, weren't published, and each one a page showed
on GitHub was broken in the site. The only place for them was a theme's
`_static/`, away from the pages that show them.

A link relative to the page it's in, like `images/setup.png`, was left as it
was written ([0014](./0014-site-url-and-its-path.md)), so the browser read it
from the page's URL, not from the folder of the file it was written in. Each
page is built into a folder, `guide/install.md` into
`guide/install/index.html`, at `/guide/install`. `little serve` serves it
there, so the image is at `/guide/images/setup.png`, as on GitHub. But GitHub
Pages, like most hosts, sends the browser on to `/guide/install/`, where the
image is at `/guide/install/images/setup.png`, which doesn't exist. A link
that worked while it was written broke once it was published, on every page
but a folder's `README.md`.

A link that works when the Markdown is read on GitHub should work in the site
little builds from it.

## Options considered
- **Which files to publish:** every file in the source but its Markdown, as
  MkDocs, Jekyll, and mdBook do; only the files a page links to, as Sphinx and
  VitePress do, which means finding every link, and leaves out a file that
  only a theme, a script, or another site links to; or only the files git
  tracks, which needs git, or a `.gitignore` reader, and publishes something
  else when the folder is copied without its repository.
- **Where a site's file and a theme's have the same path:** the site's wins,
  as the site has the last word on the config, and a theme's own file wins
  over that of the theme it builds on
  ([0003](./0003-choosing-and-layering-themes.md)), so an author can replace
  one of a theme's files, like its stylesheet, without a theme of their own;
  the theme's wins; or the build stops.
- **A destination inside the source,** like `docs/_build`, which a build would
  copy into itself, a folder deeper each time: skip it, as mdBook skips its
  own; stop the build; or leave the author to hide it, as `.build`.
- **How a relative link leads from its file's folder:** little works out where
  it leads as it builds the page, and writes it from the root of the site,
  where it gets the site's path like any other; each page is built at a URL
  that ends with `/`, like `/guide/install/`, as MkDocs builds them, and its
  relative links are written a `../` deeper, which moves every page but a
  folder's `README.md`, and breaks where a host serves the page without the
  `/`; a `<base>` tag, which is markup the theme owns, and sends every
  `#fragment` link to the folder's page; or authors write every link from the
  root of the site, as before, which on GitHub leads from the root of the
  repository instead.

## Decision
- Every file in the source that isn't hidden
  ([0012](./0012-hidden-files.md)) and isn't a `.md` file is copied as it is
  to the same path in the destination, beside the pages: from
  `docs/guide/images/setup.png` to `dist/guide/images/setup.png`.
- Where a file in the source and a theme's static file have the same path,
  the site's is copied. One that would take the place of a page, or of the
  folder it's in, stops the build and names both, as a theme's does.
- A destination inside the source isn't copied, however its path is written.
- A relative link or image in Markdown, like `images/setup.png` or
  `../getting-started`, leads from the folder of the file it's written in, as
  on GitHub, whatever URL its page is served at: in `guide/install.md`,
  `images/setup.png` is `/guide/images/setup.png`. `.` and `..` lead where a
  browser takes them, so `..` can't leave the site, and a `?query` or
  `#fragment` is kept. little writes the URL from the root of the site, so it
  gets the site's path ([0014](./0014-site-url-and-its-path.md)), like
  `/little/guide/images/setup.png`.
- A URL from the root of the site, like `/guide/`, leads from there, as
  before. One with a scheme, like `https://…` or `mailto:…`, or that starts
  with `#` or `?`, is left as it is.

## Consequences
- Images, and anything else kept beside the pages, like a PDF to download, are
  published, and a page shows its images as it does on GitHub, wherever the
  site is published, on any host.
- Relative links are the ones that work in both places: on GitHub, a link from
  the root, like `/guide/`, leads from the root of the repository, not of the
  source.
- A link to a page's file, like `install.md`, leads from its folder too, but
  isn't changed to the page's URL, so it's still broken in the site. Changing
  it is left for later.
- With `source: .`, everything in the site's folder that isn't hidden is
  published but the destination: its `little.yml`, a theme kept there, and,
  in a code repository, folders like `target/` or `node_modules/`. Keep the
  source to what the site publishes, like `docs/`.
- A file that isn't hidden but isn't meant for the site is published too, like
  an editor's backup, `page.md~`, which 0012 noted was never a page. Only
  `.md` files are pages: a `.markdown` file is copied as it is.
- A site replaces one of its theme's files, like `assets/style.css`, by
  keeping its own at the same path in the source.
- Each file is read into memory as the site is built, as each page is, so a
  source with large videos builds slower, until builds write only what's
  changed.
- `little serve` already rebuilds when a file in the source changes, so a
  saved image shows at once.
- Files whose paths differ only in case aren't checked, as pages aren't
  ([0013](./0013-pages-that-clash.md)).
