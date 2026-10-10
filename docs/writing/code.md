---
title: Code
description: Code blocks, highlighted as each page is built in any of bat's 220 languages, and choosing the classes they're highlighted with.
---

# Code

Name a code block's language after its fence, like ```` ```rust ````, and
little highlights it as it builds the page, in any of the 220 languages
[bat](https://github.com/sharkdp/bat) has, so code is in colour without
JavaScript:

```rust
fn main() {
    let name = "little";
    println!("Hello, {name}!");
}
```

```python
def greet(name: str) -> str:
    """Say hello to someone."""
    return f"Hello, {name}!"
```

```yaml
# little.yml
name: My Project
markdown:
  highlight: github
```

## Languages

- A block's language is the first word after its fence, up to a comma, so
  ```` ```rust,ignore ```` is Rust. It's matched to a language by its name or
  its files' extension, in any case, and `shell`, `golang`, `csharp`,
  `jsonc`, `docker`, and `pwsh` are matched as on GitHub.
- A block in a language little doesn't have, like `plantuml`, or without
  one, stays plain, unless a [plugin](../plugins/using-plugins.md) renders
  it.
- The languages are bat's, not Pygments', so a token can have another class
  than Pygments would give it, like Rust's `let`, a type to bat.
- There are no line numbers, or highlighted lines, yet.

## Copying code

The built-in theme puts a button in the corner of each code block that
copies its code: it shows when the block is hovered, or the button has focus,
and always on a touch screen. It needs JavaScript, and the browser's
clipboard, which a site has only on HTTPS, or at `localhost` or `127.0.0.1`,
where `little serve` serves it. So a phone that opens `little serve` by your
computer's address, with
[`--host 0.0.0.0`](../reference/commands.md#choosing-where-to-serve), has no
copy buttons. A notebook's output isn't code, so it has none.

## Choosing how code is highlighted

little puts a class on each token for the theme to colour, like `k` for a
keyword. `markdown.highlight` chooses the classes:

| Value | Classes |
| ----- | ------- |
| `pygments` | Pygments', like `k`, `s2`, and `c1`, which Pygments, Rouge, and Chroma write too, so their stylesheets colour them, like the one `pygmentize -S monokai -f html -a .highlight` writes. little's default. |
| `github` | GitHub's, like `pl-k`, `pl-s`, and `pl-c`, which [github-markdown-css](https://github.com/sindresorhus/github-markdown-css) colours as GitHub does |
| `false` | none: code is plain, for a theme that highlights it in the browser, with a script like highlight.js or Prism |

```yaml
# little.yml
markdown:
  highlight: github
```

- A theme sets it in its `little.yml`, for the classes its CSS colours, and
  the site's `little.yml` has the last word. The built-in theme colours
  Pygments' classes and GitHub's, light or dark, as the page is. A page
  can't choose its own highlighting in its frontmatter.
- Anything but `pygments`, `github`, or `false` is an error, and every
  command stops and says why.
- [Styling Markdown](../themes/styling-markdown.md) shows the HTML a
  highlighted block is.
