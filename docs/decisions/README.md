---
title: Design decisions
description: Why little works as it does, in a record of each notable choice in its design, as it was made.
---

# Design decisions

Each notable choice in little's design has a record here, an architecture
decision record, or ADR: what led to it, the options weighed, what was
decided, and what follows from it. They're numbered in the order they were
made, and link to each other.

> [!NOTE]
> A record says what was decided when it was made. One that a later record
> changes says so in its status, at its top, and names it. The rest of this
> manual describes little as it is now.

- [0001 — Build little in Rust](0001-language-rust.md)
- [0002 — Theme structure and template lookup](0002-theme-structure.md)
- [0003 — Choosing themes, and themes that build on others](0003-choosing-and-layering-themes.md)
- [0004 — Downloading themes](0004-downloading-themes.md)
- [0005 — Previous and next pages](0005-previous-and-next-pages.md)
- [0006 — Frontmatter and config have the last word](0006-frontmatter-and-config-win.md)
- [0007 — A page's table of contents is shaped like the nav](0007-page-nav-shaped-like-site-nav.md)
- [0008 — A page's table of contents is `page.toc`](0008-page-toc.md)
- [0009 — The config chooses which headings a table of contents lists](0009-toc-depth-in-config.md)
- [0010 — Breadcrumbs](0010-breadcrumbs.md)
- [0011 — The config chooses where `little serve` serves](0011-serve-host-and-port-in-config.md)
- [0012 — Hidden files and folders are left out](0012-hidden-files.md)
- [0013 — Two files can't be the same page](0013-pages-that-clash.md)
- [0014 — A site's `url` puts it under its path](0014-site-url-and-its-path.md)
- [0015 — Images and other files are published, and relative links work as on GitHub](0015-files-and-relative-links.md)
- [0016 — Checking links with lychee](0016-checking-links.md)
- [0017 — A link to a page's file leads to the page, and a broken link is shown as it's written](0017-links-to-pages-files.md)
- [0018 — Sass in themes](0018-sass-in-themes.md)
- [0019 — Searching a site with Pagefind](0019-site-search.md)
- [0020 — Linking to a page's file in its repository](0020-links-to-a-pages-file-in-its-repo.md)
- [0021 — Search's files go in `pagefind/`](0021-search-files-in-pagefind.md)
- [0022 — Choosing Markdown's extensions, and callouts](0022-markdown-extensions.md)
- [0023 — Highlighting code](0023-highlighting-code.md)
- [0024 — Jupyter notebooks as pages](0024-jupyter-notebooks.md)
- [0025 — Rendering math as MathML](0025-rendering-math-as-mathml.md)
- [0026 — The year a site is built](0026-the-year-a-site-is-built.md)
- [0027 — Drawing Mermaid diagrams as SVG](0027-drawing-mermaid-diagrams.md)
- [0028 — How diagrams look](0028-how-diagrams-look.md)
- [0029 — Plugins that render code blocks](0029-plugins-that-render-code-blocks.md)
- [0030 — Plugins that add pages](0030-plugins-that-add-pages.md)
- [0031 — A plugin for Python's docstrings](0031-a-plugin-for-pythons-docstrings.md)
- [0032 — little's docs are its manual](0032-docs-are-the-manual.md)
- [0033 — The built-in theme's design](0033-the-built-in-themes-design.md)
- [0034 — The built-in theme's script](0034-the-built-in-themes-script.md)
- [0035 — Numbers in the nav's order](0035-numbers-in-the-navs-order.md)
- [0036 — Folding the built-in theme's nav](0036-folding-the-built-in-themes-nav.md)
- [0037 — Wrapping long names in the built-in theme](0037-wrapping-long-names-in-the-built-in-theme.md)
- [0038 — The built-in theme's page and tables](0038-the-built-in-themes-page-and-tables.md)
- [0039 — A long site name in the built-in theme's header](0039-a-long-site-name-in-the-built-in-themes-header.md)
- [0040 — Versions of a site, built from git refs](0040-versions-from-git-refs.md)
- [0041 — A version's title and URL](0041-version-titles-and-urls.md)
- [0042 — Versions share the site's config, and its theme's files](0042-versions-share-the-sites-config-and-theme-files.md)
- [0043 — A version's own nav, and its plugins' settings](0043-a-versions-own-nav-and-plugins.md)
- [0044 — A plugin for Rust's doc comments](0044-a-plugin-for-rusts-doc-comments.md)
- [0045 — Limits on downloads](0045-limits-on-downloads.md)
- [0046 — Plugin hooks](0046-plugin-hooks.md)
- [0047 — Naming API reference plugins](0047-naming-api-reference-plugins.md)
- [0048 — A plugin for JavaScript's doc comments](0048-a-plugin-for-javascripts-doc-comments.md)
- [0049 — An API reference's URL and title](0049-an-api-references-url-and-title.md)
- [0050 — One repository for little, its themes, and its plugins](0050-one-repository-for-little-its-themes-and-plugins.md)
- [0051 — Semantic Versioning and Conventional Commits](0051-semantic-versioning-and-conventional-commits.md)
- [0052 — Releasing plugins from tags](0052-releasing-plugins-from-tags.md)
- [0053 — little's decisions are in its manual](0053-decisions-in-the-manual.md)
- [0054 — Third-party notices](0054-third-party-notices.md)
- [0055 — Releasing little](0055-releasing-little.md)
- [0056 — Publishing the manual](0056-publishing-the-manual.md)
- [0057 — Starting a site with `little init`](0057-starting-a-site-with-little-init.md)
- [0058 — Features raise the minor version](0058-features-raise-the-minor-version.md)
