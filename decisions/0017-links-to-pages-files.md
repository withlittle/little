# 0017 — A link to a page's file leads to the page, and a broken link is shown as it's written

- Status: accepted
- Date: 2026-10-02
- Amends: [0015](./0015-files-and-relative-links.md),
  [0016](./0016-checking-links.md)

## Context
On GitHub, a page links to another by its file, like `[Install](install.md)`
or `[Decisions](Decisions/README.md)`, and docs written to be read there do.
little left such a link as it was written
([0015](./0015-files-and-relative-links.md)), so it led to a file the site
doesn't have, and every one was broken: in one set of docs, 87 of the 88
warnings `little serve` gave.

Each warning showed the link as a reader follows it
([0016](./0016-checking-links.md)): from the root of the domain, with the
site's path, like `/little/Decisions/README.md`. The author had written
`./Decisions/README.md`, so they searched their file for the warning's link
and didn't find it, and the warning looked wrong.

## Options considered
- **A link to a page's file:** lead it to the page, as MkDocs, mdBook, and
  Docusaurus do; or leave it, and have authors link by URL, which breaks the
  link on GitHub.
- **Which files lead to a page:** only `.md` files, as only they're pages; or
  `.markdown` and `.MD` files too, which have no page to lead to.
- **A link to a `.md` file that isn't there:** lead it to where its page would
  be, so it's still found broken; or leave it as it's written.
- **How a warning shows a link:** as the Markdown writes it, which an author
  can search for, but which says less of where it leads, as it's relative to
  its file; as a reader follows it, as before; or both, which makes every
  warning longer.

## Decision
- A link in Markdown to a `.md` file, relative to its file or from the root
  of the site, leads to the page built from it: in `guide/README.md`,
  `install.md#from-source` leads to `/guide/install#from-source`, and
  `../README.md` to `/`. A `README.md` leads to its folder's page, whatever
  its case, like `readme.md`. A `?query` and `#fragment` are kept, and the
  site's path is added as to any link
  ([0014](./0014-site-url-and-its-path.md)).
- One to a `.md` file that isn't there leads to where its page would be, so
  it's still warned of.
- Only a link in Markdown to a `.md` file leads to a page. An image doesn't,
  nor does a link to a `.markdown` or `.MD` file, one with a scheme, like
  `https://…/README.md`, or one in the theme, a copied HTML file, or a `nav`
  in `little.yml`.
- A warning shows a broken link in a page's Markdown as it's written there,
  like `install.md#from-sorce`, and any other link, like one in the theme or
  a copied HTML file, as a reader follows it, as before. A link written alike
  in many files is one warning, as before; written two ways, it's a warning
  for each.
- lychee reads the built HTML, so as little renders a page, it notes each
  link as it's written and as it's in the HTML, and matches the links lychee
  finds in the page to those, by where they lead.

## Consequences
- Docs written to be read on GitHub build into a site whose links between
  pages work, unchanged, and are checked as they're built.
- An author finds a broken link by searching their file for the warning's
  link.
- A link that leaves the source, like `../../CONTRIBUTING.md` from
  `docs/README.md`, is still broken, as `..` can't leave the site
  ([0015](./0015-files-and-relative-links.md)).
- A link in the theme that leads where one in the same page's Markdown does
  is shown as the Markdown writes it, as little can't tell them apart.
