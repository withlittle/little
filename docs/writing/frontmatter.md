---
title: Frontmatter
description: A page's own settings, in YAML at its top, which reach the theme as page, and have the last word on it.
---

# Frontmatter

Frontmatter is an optional block of YAML settings at the very top of a page,
between two `---` lines. This page's is:

```markdown
---
title: Frontmatter
description: A page's own settings, in YAML at its top, which reach the theme as page, and have the last word on it.
---
```

Every setting reaches the theme as `page.<key>`, like `description`, which
the built-in theme gives the page as its meta description, for search
engines and link previews. You can add your own, for a theme that shows
them.

## A page's title

little itself reads `title`, for the page's title, which the nav lists it by.
Without one, a page's title is its first `#` heading, then its file's or
folder's name. A [nav written in `little.yml`](navigation.md#writing-the-nav-yourself)
gives each page its own title there.

## Frontmatter wins

A page's frontmatter has the last word on `page`, even over what little works
out: it merges over it the way a `little.yml` merges over the level beneath,
so:

| Frontmatter | Does |
| ----------- | ---- |
| `prev: { title: Start here }` | renames the link to the [previous page](navigation.md#previous-and-next-pages) |
| `next: false` | removes the link to the next page |
| `toc: false` | removes the [table of contents](navigation.md#tables-of-contents) |
| `breadcrumbs: false` | removes the [breadcrumbs](navigation.md#breadcrumbs) |
| `edit_url: false` | removes the link to [edit the page](../getting-started/configuring.md#choosing-the-sites-repository) |

- It changes only what the theme sees: `url` doesn't move the page, and
  `type` doesn't choose its template.
- A key left blank, like `description:`, is ignored.
- A page can't change the site's settings, like its
  [Markdown](markdown.md#choosing-markdowns-extensions), or the levels of
  heading its table of contents lists.

## In a notebook

A notebook's frontmatter is in a raw or Markdown cell at its top, as Quarto
and MyST put it: see [Notebooks](notebooks.ipynb).
