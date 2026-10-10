# 0023 — Highlighting code

- Status: accepted; amended by [0027](./0027-drawing-mermaid-diagrams.md),
  which draws a ```` ```mermaid ```` block as a diagram, unless
  `markdown.mermaid` is `false`, and by
  [0033](./0033-the-built-in-themes-design.md), whose built-in theme shows
  code light or dark, as the page is, and colours GitHub's classes too
- Date: 2026-10-04

## Context
Slice 2 of little's [roadmap](../VISION.md), authoring essentials, asks for
code in colour. An author names a code block's language after its fence,
```` ```rust ````, as on GitHub, and expects to see it in colour, as there.
little writes it as comrak does, as plain text in
`<pre><code class="language-rust">`, so a theme that wants colour has to load
a highlighter, like highlight.js or Prism, and run it in every reader's
browser.

comrak can hand each code block to a highlighter as it writes the page.
[syntect](https://crates.io/crates/syntect), which reads Sublime Text's
grammars, is already in little, through comrak, though nothing uses it.

## Options considered
- **Where code is highlighted:** in each reader's browser, by a script the
  theme loads, so the page shows plain code until the script has run; or as
  little builds the page, so it arrives in colour, without JavaScript. The
  highlighters that read VS Code's grammars, like Shiki, need Node.
- **Which languages:** syntect has about 75, but not TOML, TypeScript, or
  Dockerfiles, which docs show often.
  [two-face](https://crates.io/crates/two-face) adds the rest of those
  [bat](https://github.com/sharkdp/bat) has, about 220 in all, in about 1 MB.
- **What the HTML says:**
  - each token's colour, inline, as syntect's own HTML has it, so little
    chooses the colours, not the theme, which can't change them or have a
    dark mode;
  - or a class on each token, like a keyword or a string, for the theme to
    colour, in one of these sets of classes:
    - syntect's own, a `<span>` for each scope, nested, like
      `<span class="source rust"><span class="meta function rust">…`, which
      no stylesheet knows;
    - Pygments', short classes like `k` for a keyword and `s2` for a string
      in `"`, which Pygments, Rouge (Jekyll's), and Chroma (Hugo's) all
      write, so there are hundreds of stylesheets for them, like those
      `pygmentize -S monokai -f html` writes;
    - GitHub's, like `pl-k`, which
      [github-markdown-css](https://github.com/sindresorhus/github-markdown-css)
      colours as GitHub does.
- **How a language is named:** the first word after the fence, up to a
  comma, so ```` ```rust,ignore ```` is Rust, matched to a grammar by its
  name or its files' extension, in any case: `rust`, `Rust`, or `rs`. A few
  names GitHub knows match no grammar's: `shell`, `golang`, `csharp`,
  `jsonc`, `docker`, and `pwsh`. Names like `text`, `console`, and `mermaid`
  aren't languages bat has.

## Decision
- `markdown.highlight` chooses the classes little highlights code with, as
  it builds each page:
  - `pygments`, little's default: Pygments' classes, like `k`, `s2`, and
    `c1`;
  - `github`: GitHub's, each that github-markdown-css colours on its own,
    for the scopes it's named for, like `pl-k` for a keyword, `pl-s` for a
    string, and `pl-en` for a name;
  - `false`: none, as before, for a theme that highlights code itself.

  Anything else stops the build, naming the three.
- With classes, each code block's `<pre>` has `class="highlight"`, so
  Pygments' stylesheets, which colour `.highlight .k`, find its tokens. Its
  `<code>` keeps comrak's class, like `language-rust`. Each token is a
  `<span>` with its class, in a row, never nested, and never over the end of
  a line, as Pygments writes them. Spaces between tokens, and tokens without
  a class, are plain text.
- Languages are bat's, named as above, with `shell` for Bash, `golang` for
  Go, `csharp` for C#, `jsonc` for JSON, `docker` for a Dockerfile, and
  `pwsh` for PowerShell. A block in a language little doesn't know, or
  without one, is plain text in the same `<pre class="highlight">`, so a
  theme's script can still find it by its class, like `language-mermaid`.
- A ```` ```math ```` block isn't highlighted: comrak marks it as math, with
  `math` on or off.
- The built-in theme colours Pygments' classes, on its dark code blocks. A
  theme that chooses `github` brings colours for its classes, like
  github-markdown-css's.
- New crate: `two-face`, with bat's grammars, and syntect through it, as
  comrak has it.

## Consequences
- Code is in colour as soon as a page loads, without JavaScript, in any
  theme with a stylesheet for Pygments' classes, and it's still text, to
  search and to copy.
- A theme that highlights code in the browser, with highlight.js or Prism,
  sets `markdown.highlight: false` in its `little.yml`, so its script gets
  code as comrak writes it.
- little grows by 1.7 MB, to 40 MB: bat's grammars are 1 MB of it, and the
  rest is syntect's code that reads them, and Oniguruma's, which comrak
  brought in but nothing used. The grammars are read in a few milliseconds,
  when little first highlights a block.
- bat's grammars aren't Pygments' lexers, so a token can have a different
  class than Pygments gives it, like Rust's `let`, which is a type to bat's
  grammar.
- Limits, for now:
  - no line numbers, and no highlighted lines;
  - a page can't choose its own highlighting in frontmatter;
  - the built-in theme doesn't colour `github`'s classes;
  - a language bat doesn't have, like Mermaid, stays plain.
