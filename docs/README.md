---
title: Home
description: little turns a folder of Markdown into a documentation site.
---

# Welcome to **little**

little is a tiny static documentation generator: drop Markdown into `docs/` and —
*wham* — you get a site. No boilerplate, no config required to start.

This very page is `docs/README.md`. Like on GitHub, a folder's `README.md` is its
home page — little serves it at the site root (`/`), and a `README.md` inside any
subfolder becomes that section's landing page.

## What this skeleton already does

- Reads every `.md` file in `docs/`
- Parses YAML frontmatter (that `title:` above)
- Renders CommonMark + GFM — tables, ~~strikethrough~~, task lists — via `comrak`
- Builds a **nested nav** from the folder structure, with clean, extensionless URLs
- Adds an **on-this-page TOC** from each page's headings
- Hands **structured data** to a theme template (little bakes in *zero* HTML)
- Merges a **3-level cascading config**: little → theme → site
- Indexes every page for **search**: press ⌘K or Ctrl+K
- Serves it all with **live reload**

> Try it: run `little serve`, edit this sentence, and watch the browser refresh itself.
