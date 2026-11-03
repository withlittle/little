# 0009 — The config chooses which headings a table of contents lists

- Status: accepted
- Date: 2026-09-30

## Context
`page.toc` held a page's `##` and `###` headings, and no others
([0007](./0007-page-nav-shaped-like-site-nav.md)): little's code chose them.
But how much of each page's outline to list is a choice about how its pages
look, which is the theme's and the site's ([VISION](../VISION.md)). A theme
with room for more couldn't list a `####`, and a site with long pages couldn't
ask for fewer.

## Options considered
- **Who chooses:** little, as before; each theme's templates, from every
  heading little hands them, as in Zola; or the config, in settings little
  reads, which cascade like any other: little sets a default, and a theme's
  and a site's `little.yml` override it, as in Docusaurus
  (`themeConfig.tableOfContents`), Starlight (`tableOfContents`), Hugo
  (`markup.tableOfContents`), and MkDocs (`toc_depth`). In the templates,
  every theme must cut the list short itself, and a site's setting does only
  what its theme makes of it, so changing themes can lose it. In the config,
  it works with every theme, and every template just lists `page.toc`.
- **What to count:** heading levels, from a min to a max, like Docusaurus's
  `minHeadingLevel` and `maxHeadingLevel` and Hugo's `startLevel` and
  `endLevel`; or levels of the list, like the `:depth:` of docutils'
  `contents`. The two differ only where a page skips a level: a `####` right
  under a `##` is on the list's second level.
- **The names:** a `toc` setting holding `min_depth` and `max_depth`, or flat
  `toc_min_depth` and `toc_max_depth`. But in a page's frontmatter, `toc` is
  already its table of contents, which `toc: false` turns off
  ([0008](./0008-page-toc.md)), and in a template, `site.toc` reads as a table
  of contents for the whole site. A depth counts heading levels, as MkDocs'
  `toc_depth` and Pandoc's `--toc-depth` do.
- **little's defaults:** `##` and `###`, as before and as in Docusaurus, or
  every heading but a page's `#`, which is its title. MkDocs lists every
  heading.

## Decision
- `toc_min_depth` and `toc_max_depth` are the levels of heading, from 1 for
  `#` to 6 for `######`, that each page's table of contents lists. little
  reads them from the merged config, as it does `source` and `destination`,
  and `page.toc` holds just those headings, nested as 0007 says.
- little's `little.yml` sets them to 2 and 6: every heading but a page's
  title. The built-in theme's `little.yml` sets `toc_max_depth: 3`, so it
  lists `##` and `###`, as before. A site can set either in its own.
- A setting that isn't a level from 1 to 6, or a `toc_min_depth` over the
  `toc_max_depth`, stops the build, and the error names it.

## Consequences
- A site chooses its tables of contents in one place, whatever its theme, and
  a theme lists `page.toc` whole, without reading any setting.
- The built-in theme lists what it did. A theme without a `little.yml` now
  lists every heading but the title, down to `######`, where it listed `##`
  and `###`.
- A heading is listed by its level, not where it is: with `toc_max_depth: 3`,
  a `####` is left out, even right under a `##`.
- With `toc_min_depth: 1`, a page's `#` headings are listed too, each with
  the headings under it as its `children`. Items don't carry their level
  (0007), so a theme can't tell a `#` from a `##`.
- The min and max can come from different levels of the cascade: a site that
  sets `toc_min_depth: 4` over the built-in theme's `toc_max_depth: 3` gets an
  error, not empty tables of contents.
- A page can't choose its own levels, only turn its table of contents off
  with `toc: false`: its frontmatter reaches `page`, not the config.
