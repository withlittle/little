---
title: Getting Started
---

# Getting Started

Three commands, that's the whole CLI so far:

```sh
little serve      # live-reloading dev server at http://127.0.0.1:8080
little build      # write the static site to dist/
little config     # print the site's config, merged
```

## A table, rendered by comrak

| Feature            | Status |
| ------------------ | :----: |
| Markdown + GFM     | ✅     |
| Jekyll-style theme | ✅     |
| Cascading config   | ✅     |
| Live reload        | ✅     |
| Search             | ✅     |
| Plugins (WASM)     | 🔜     |

## A task list

- [x] Install the Rust toolchain
- [x] Render a Markdown page
- [x] Feel the dev loop
- [ ] Decide on little's language 😉

## An alert, as on GitHub

> [!TIP]
> Turn on more of Markdown, like `math` or `smart_punctuation`, under
> `markdown` in `little.yml`.

## Emoji, footnotes, and HTML

GitHub's emoji shortcodes work :tada:, and so do footnotes.[^cascade]

<details>
<summary>And HTML, like this <code>&lt;details&gt;</code></summary>

Its Markdown is rendered too, after a blank line.

</details>

[^cascade]: Like every setting, `markdown` cascades: little sets it, then the
    theme, then the site.
