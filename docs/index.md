---
title: Home
---

# Welcome to **little**

little is a tiny static documentation generator: drop Markdown into `docs/` and —
*wham* — you get a site. No boilerplate, no config required to start.

This very page is being rendered by a ~450-line Rust **walking skeleton**, built so
you can *feel* what developing `little` in Rust is like before committing to it.

## What this skeleton already does

- Reads every `.md` file in `docs/`
- Parses YAML frontmatter (that `title:` above)
- Renders CommonMark + GFM — tables, ~~strikethrough~~, task lists — via `comrak`
- Hands **structured data** to a theme template (little bakes in *zero* HTML)
- Merges a **3-level cascading config**: little → theme → site
- Serves it all with **live reload**

> Try it: run `little serve`, edit this sentence, and watch the browser refresh itself.
