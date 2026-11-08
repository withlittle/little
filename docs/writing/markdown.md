---
title: Markdown
description: GitHub's Markdown, with callouts, footnotes, emoji, and HTML, and turning Markdown's extensions on or off.
---

# Markdown

little reads Markdown as GitHub does, with tables, ~~strikethrough~~, task
lists, autolinks, footnotes, callouts, emoji shortcodes, HTML,
[math](math.md), and [diagrams](diagrams.md). Other extensions, like
`==marked==` text, are off until you
[turn them on](#choosing-markdowns-extensions).

## GitHub's extensions

| You write | You get |
| --------- | ------- |
| `~~struck through~~` | ~~struck through~~ |
| `:tada:` | :tada: |
| `www.example.com` | www.example.com |
| `\| a \| table \|`, with `\| --- \| --- \|` below | this table |

A colon in the line below a table's headings aligns its column:

| Left | Centre | Right |
| :--- | :----: | ----: |
| `:---` | `:----:` | `----:` |

- [x] A task list, with `- [x]` for a done task
- [ ] and `- [ ]` for one to do

A footnote, like this one,[^note] is numbered, with its note at the end of
the page.

[^note]: Written `[^note]` in the text, and `[^note]: the note` on a line of
    its own.

## Callouts

A callout makes a note or a warning stand out. Write it as a quote whose
first line is its kind:

```markdown
> [!TIP]
> `little serve` rebuilds the site each time you save.
```

There are five kinds, each with its own colour in the built-in theme:

> [!NOTE]
> `[!NOTE]`, for something readers should know, even if they skim.

> [!TIP]
> `[!TIP]`, for a better way to do something.

> [!IMPORTANT]
> `[!IMPORTANT]`, for something readers need to know to succeed.

> [!WARNING]
> `[!WARNING]`, for something that needs readers' attention straight away.

> [!CAUTION]
> `[!CAUTION]`, for what could go wrong, like a build wiping a folder.

A callout can have a title of its own after its kind, like
`> [!WARNING] Before you upgrade`.

## HTML

HTML in Markdown is rendered as it is, like a `<details>`, which a reader
opens:

<details>
<summary>Is HTML safe?</summary>

Only in Markdown you trust: see below.

</details>

`<script>` is rendered as it is too, and so is the HTML in a notebook's
outputs. For Markdown or notebooks you don't trust, set
[`raw_html: false`](#choosing-markdowns-extensions) and `mermaid: false`,
since a diagram can carry HTML too.

## Headings

Every heading gets an `id` made from its text, like
`#choosing-markdowns-extensions` for the last one on this page, so you can
link straight to it: [this one](#headings), or
`[Callouts](markdown.md#callouts)` from another page. The built-in theme
shows a `#` beside a heading you point at, which links to it. A page's
headings also make its
[table of contents](navigation.md#tables-of-contents).

## Choosing Markdown's extensions

`markdown` turns each of little's extensions to CommonMark on or off, for
every page. To turn smart punctuation on, say, and HTML off, with the
diagrams that can carry it:

```yaml
# little.yml
markdown:
  smart_punctuation: true
  raw_html: false
  mermaid: false
```

| Key | Default | Lets you write… |
| --- | ------- | --------------- |
| `tables` | `true` | tables, with `\|` between cells |
| `strikethrough` | `true` | `~~struck through~~` text |
| `autolinks` | `true` | links without `<…>`, like `www.example.com` |
| `task_lists` | `true` | `- [ ] tasks`, and `- [x] done` ones |
| `footnotes` | `true` | footnotes, like `[^1]`, with `[^1]: the note` below |
| `alerts` | `true` | callouts, like `> [!NOTE]` |
| `emoji` | `true` | emoji shortcodes, like `:tada:` for 🎉 |
| `raw_html` | `true` | HTML, like `<details>`, rendered as it is |
| `math` | `mathml` | math, like `$x^2$`, `$$…$$`, and ```` ```math ```` blocks, [rendered as MathML](math.md#choosing-how-math-is-written) |
| `mermaid` | `svg` | diagrams, in ```` ```mermaid ```` blocks, [drawn as SVG](diagrams.md#choosing-how-diagrams-are-drawn) |
| `highlight` | `pygments` | code blocks, [highlighted](code.md#choosing-how-code-is-highlighted) with Pygments' classes |
| `smart_punctuation` | `false` | `"quotes"` that curl, `--` and `---` for – and —, and `...` for … |
| `superscript` | `false` | `x^2^` |
| `subscript` | `false` | `H~2~O`, so only `~~` strikes through |
| `mark` | `false` | `==marked==` text |
| `insert` | `false` | `++inserted++` text |
| `description_lists` | `false` | a term, with `: its description` on the line below |
| `inline_footnotes` | `false` | footnotes in the text, like `^[a note]` |
| `fenced_divs` | `false` | `:::name`, then a line `:::` below, for a `<div class="name">` |
| `multiline_blockquotes` | `false` | a quote between two `>>>` lines |
| `hard_breaks` | `false` | a line break, kept wherever a paragraph has one |
| `cjk_friendly_emphasis` | `false` | `**` and `*` beside Chinese, Japanese, and Korean punctuation |
