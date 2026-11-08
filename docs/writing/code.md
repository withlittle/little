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

## Languages

little knows the 220 languages [bat](https://github.com/sharkdp/bat) has,
by name or by their files' extension, in any case: `rust`, `Rust`, and
`rs` are all Rust. It knows GitHub's names `shell`, `golang`, `csharp`,
`jsonc`, `docker`, and `pwsh` too. A language ends at a comma, so a block
fenced with ```` ```rust,ignore ```` is Rust.

A block with no language, or one little doesn't know, like `plantuml`,
stays plain, unless a [plugin](../plugins/using-plugins.md) renders it.

## Copying code

The built-in theme puts a button to copy the code in the corner of each
code block, except a notebook's output. It shows when you point at the
block or tab to the button, and always on a touch screen.

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
| `pygments` | Pygments', like `k`, `s2`, and `c1`, which Pygments', Rouge's, and Chroma's stylesheets colour. little's default. |
| `github` | GitHub's, like `pl-k`, `pl-s`, and `pl-c`, which [github-markdown-css](https://github.com/sindresorhus/github-markdown-css) colours |
| `false` | none, for a theme that highlights code in the browser |

The built-in theme colours both Pygments' classes and GitHub's, in light
and dark. To colour them in your own theme, see
[Styling Markdown](../themes/styling-markdown.md).
