---
title: Frontmatter
description: A page's own settings, in YAML at its top.
---

# Frontmatter

Frontmatter is a block of YAML at the very top of a page, between two
`---` lines. This page's is:

```markdown
---
title: Frontmatter
description: A page's own settings, in YAML at its top.
---
```

The built-in theme makes `description` the page's meta description, for
search engines and link previews, and gives a page without one the site's
[`description`](../getting-started/configuring.md#the-built-in-themes-settings).
`search: false` leaves a page out of [search](search.md#leaving-pages-out).
Any key of your own reaches the theme as `page.<key>`, for a theme that
shows it.

## A page's title

A page's title is its `title`, else its first `#` heading, else its file's
name, so `getting-started.md` is Getting Started. A `README.md` takes its
folder's name, and `docs/README.md` is Home.

The sidebar, breadcrumbs, and links to the previous and next pages show a
page by its title. If you
[write the nav yourself](navigation.md#writing-the-nav-yourself), they
show the title you give the page there instead.

## Frontmatter wins

Frontmatter has the last word on what the theme gets as `page`, even over
what little works out, so you can change or remove what the theme shows
around a page:

| Frontmatter | Does |
| ----------- | ---- |
| `prev: { title: Start here }` | renames the link to the [previous page](navigation.md#previous-and-next-pages), keeping its URL |
| `next: false` | removes the link to the next page |
| `toc: false` | removes the [table of contents](navigation.md#tables-of-contents) |
| `breadcrumbs: false` | removes the [breadcrumbs](navigation.md#breadcrumbs) |
| `edit_url: false` | removes the link to [edit the page](../getting-started/configuring.md#choosing-the-sites-repository) |

It changes only what the theme sees: `url` doesn't move the page, `type`
doesn't choose its template, and a page can't change the site's settings,
like its [Markdown](markdown.md#choosing-markdowns-extensions).

## In another format

A notebook's frontmatter goes in a raw or Markdown cell at its top: see
[Jupyter notebooks](../plugins/jupyter.ipynb).

A page in [AsciiDoc](../plugins/asciidoc.adoc),
[Org mode](../plugins/org-mode.org), or
[reStructuredText](../plugins/restructuredtext.rst) takes its `title` and
`description` from its format's own header, like AsciiDoc's `= Title`. It
can also start with frontmatter between `---` lines, which goes over the
header.
