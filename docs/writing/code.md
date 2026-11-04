---
title: Code
description: Code blocks, highlighted as each page is built, in any of 220 languages.
---

# Code

Name a code block's language after its fence:

````markdown
```rust
fn main() {
    let name = "little";
    println!("Hello, {name}!");
}
```
````

little highlights it as it builds the page, so it's in colour without
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

## Languages

little knows the 220 languages [bat](https://github.com/sharkdp/bat) has.
It matches a block's language by its name, or its files' extension, in any
case. So `rust`, `Rust`, and `rs` are all Rust. little also knows GitHub's
names `shell`, `golang`, `csharp`, `jsonc`, `docker`, and `pwsh`.

- The language ends at a comma, so ```` ```rust,ignore ```` is Rust.
- A block with no language, or one little doesn't know, like `plantuml`,
  stays plain. A [plugin](../plugins/using-plugins.md) can render it
  instead.
- The languages are bat's, not Pygments'. So a token can get a different
  class than Pygments would give it, like Rust's `let`, which bat calls a
  type.
- There are no line numbers, or highlighted lines, yet.

## Copying code

The built-in theme puts a button in the corner of each code block, to copy
its code. It shows when you hover over the block, or tab to the button, and
always on a touch screen.

- It needs JavaScript, and the browser's clipboard. A browser has one only
  for a site on HTTPS, or at `localhost` or `127.0.0.1`, where
  `little serve` serves it. So a phone that opens `little serve` by your
  computer's address, with
  [`--host 0.0.0.0`](../reference/commands.md#choosing-where-to-serve), has
  no copy buttons.
- A notebook's output isn't code, so it has none.

## Choosing how code is highlighted

little puts a class on each token, like `k` for a keyword, and the theme
colours each class. `markdown.highlight` chooses which classes:

```yaml
# little.yml
markdown:
  highlight: github
```

| Value | Classes |
| ----- | ------- |
| `pygments` | Pygments', like `k`, `s2`, and `c1`. Rouge and Chroma write them too, so their stylesheets work, like the one `pygmentize -S monokai -f html -a .highlight` writes. little's default. |
| `github` | GitHub's, like `pl-k`, `pl-s`, and `pl-c`, which [github-markdown-css](https://github.com/sindresorhus/github-markdown-css) colours as GitHub does |
| `false` | None. Code is plain, for a theme that highlights it in the browser, with a script like highlight.js or Prism. |

The built-in theme colours both Pygments' classes and GitHub's, in light
and dark. Another theme sets the classes its CSS colours in its
`little.yml`, and the site's `little.yml` has the last word.

- A page can't choose its own highlighting in its frontmatter.
- Anything but `pygments`, `github`, or `false` is an error. Every command
  stops, and says why.
- [Styling Markdown](../themes/styling-markdown.md) shows the HTML of a
  highlighted block.
