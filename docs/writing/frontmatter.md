---
title: Frontmatter
description: A page's own settings, in YAML at its top.
---

# Frontmatter

Frontmatter is a block of YAML at the very top of a page, between two
`---` lines. It's optional. This page's is:

```markdown
---
title: Frontmatter
description: A page's own settings, in YAML at its top.
---
```

The built-in theme uses `description` as the page's meta description, for
search engines and link previews. You can add keys of your own, too. Each
reaches the theme as `page.<key>`, for a theme that shows it.

## A page's title

`title` sets the page's title, which the sidebar lists it by. Without one,
the title is the page's first `#` heading. Without that, it's the name of
the page's file or folder.

A [nav in `little.yml`](navigation.md#writing-the-nav-yourself) gives each
page the title it lists there.

## Leaving a page out of search

`search: false` leaves the page out of [search](search.md#leaving-pages-out).
It's still built, and still in the sidebar.

`search: true` keeps the page in search, even under a URL that the site's
`search.exclude` leaves out.

## Frontmatter wins

Frontmatter has the last word on what the theme gets as `page`, even over
what little works out itself. So you can change, or remove, what the theme
shows around a page:

| Frontmatter | Does |
| ----------- | ---- |
| `prev: { title: Start here }` | renames the link to the [previous page](navigation.md#previous-and-next-pages) |
| `next: false` | removes the link to the next page |
| `toc: false` | removes the [table of contents](navigation.md#tables-of-contents) |
| `breadcrumbs: false` | removes the [breadcrumbs](navigation.md#breadcrumbs) |
| `edit_url: false` | removes the link to [edit the page](../getting-started/configuring.md#choosing-the-sites-repository) |

- Frontmatter merges key by key, as a `little.yml` does. So
  `prev: { title: Start here }` keeps the link's URL.
- It changes only what the theme sees. So `url` doesn't move the page, and
  `type` doesn't choose its template.
- A key left blank, like `description:`, is ignored.
- A page can't change the site's settings, like its
  [Markdown](markdown.md#choosing-markdowns-extensions), or which headings
  its table of contents lists.

## In a notebook

A notebook's frontmatter goes in a raw or Markdown cell at its top, as in
Quarto and MyST. See [Jupyter notebooks](../plugins/jupyter-notebooks.ipynb).

## In another format

A page in [AsciiDoc](../plugins/asciidoc.adoc),
[Org mode](../plugins/org-mode.org), or
[reStructuredText](../plugins/restructuredtext.rst) takes its `title` and
`description` from its format's own header, like AsciiDoc's `= Title`.

It can also start with frontmatter between `---` lines, as a Markdown file
can. That goes over the header, and can set any key, like `search: false`.
