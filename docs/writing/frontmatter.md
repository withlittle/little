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
`search: { enabled: false }` leaves a page out of
[search](search.md#leaving-pages-out). Any key of your own reaches the theme
as `page.<key>`, for a theme that shows it.

## A page's title

A page's title is its `title`, else its first `#` heading, else its file's
name, so `getting-started.md` is Getting Started. A `README.md` takes its
folder's name, and `docs/README.md` is Home.

The sidebar, breadcrumbs, and links to the previous and next pages show a
page by its title. If you
[write the nav yourself](navigation.md#writing-the-nav-yourself), they
show the title you give the page there instead.

## A page's dates

A theme can show when a page was created, and when it last changed, which
little reads from git. To set either yourself, as for a page written
before it was in git, write it in the page's frontmatter:

```markdown
---
created: 2019-06-01
modified: 2026-11-19
---
```

[Dates](../reference/template-data.md#dates) says what little reads.

## Frontmatter wins

little works out what a theme shows around a page, each in a group of its
own. To remove one from a page, set its `enabled` to `false`:

```markdown
---
outline:
  enabled: false
---
```

| Group | The page's… |
| ----- | ----------- |
| `outline` | [table of contents](navigation.md#tables-of-contents) |
| `breadcrumb` | [breadcrumbs](navigation.md#breadcrumbs) |
| `previous` | link to the [page before it](navigation.md#previous-and-next-pages) |
| `next` | link to the page after it |
| `repository` | links to [view and edit its file](../getting-started/configuring.md#choosing-the-sites-repository) |
| `source` | [file](pages-and-files.md#a-pages-file), copied beside it |

Frontmatter has the last word on what the theme gets as `page`, even over
what little works out, so you can change it, too:
`previous: { title: Start here }` renames the link to the previous page,
and keeps its URL.

It changes only what the theme sees: `url` doesn't move the page, `type`
doesn't choose its template, and a page can't change the site's settings,
like its [Markdown](markdown.md#choosing-markdowns-extensions).

## For every page

To set a key for every page, put it under `page` in `little.yml`:

```yaml
# little.yml
page:
  breadcrumb:
    enabled: false
```

It does what it would in each page's frontmatter, so here no page has
breadcrumbs. A page's own frontmatter goes over it, so one page gets them
back with `breadcrumb: { enabled: true }`. A theme's `little.yml` can set
`page` too, beneath the site's.

## In another format

A notebook's frontmatter goes in a raw or Markdown cell at its top: see
[Jupyter notebooks](../plugins/jupyter.ipynb).

A page in [AsciiDoc](../plugins/asciidoc.adoc),
[Org mode](../plugins/org-mode.org), or
[reStructuredText](../plugins/restructuredtext.rst) takes its `title` and
`description` from its format's own header, like AsciiDoc's `= Title`. It
can also start with frontmatter between `---` lines, which goes over the
header.
