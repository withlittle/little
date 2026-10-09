---
title: Markdown
description: GitHub's Markdown, with callouts, footnotes, emoji, and HTML, and turning Markdown's extensions on or off.
---

# Markdown

little reads CommonMark, with GitHub's extensions to it, so Markdown written
for GitHub renders as it does there: tables, ~~strikethrough~~, task lists,
autolinks, footnotes, callouts, emoji shortcodes, HTML, [math](math.md), and
[diagrams](diagrams.md). Others, like `==marked==` text, are off until you
[turn them on](#choosing-markdowns-extensions).

## GitHub's extensions

| You write | You get |
| --------- | ------- |
| `~~struck through~~` | ~~struck through~~ |
| `:tada:` | :tada: |
| `www.example.com` | www.example.com |
| `\| a \| table \|`, with `\| --- \| --- \|` below | this table |

- [x] A task list, with `- [x]` for a done task
- [ ] and `- [ ]` for one to do

A footnote, like this one,[^note] is numbered, with its note at the end of
the page.

[^note]: Written `[^note]` in the text, and `[^note]: the note` on a line of
    its own.

## Callouts

GitHub's alerts make a note or a warning stand out from the text around it.
There are five kinds, each with its own colour in the built-in theme:

> [!NOTE]
> `[!NOTE]`, for something readers should know, even if they skim.

> [!TIP]
> `[!TIP]`, for a better way to do something, like `little serve`, which
> rebuilds the site each time you save.

> [!IMPORTANT]
> `[!IMPORTANT]`, for something readers need to know to succeed.

> [!WARNING]
> `[!WARNING]`, for something that needs readers' attention straight away.

> [!CAUTION]
> `[!CAUTION]`, for what could go wrong, like a build wiping a folder.

Write one as a quote whose first line is its kind:

```markdown
> [!TIP]
> `little serve` rebuilds the site each time you save.
```

- Where Markdown doesn't have them, an alert reads as a quote.
- An alert can have a title of its own, after its kind, like
  `> [!WARNING] Before you upgrade`, but on GitHub, that's a quote.

## HTML

HTML in Markdown is rendered as it is, like a `<details>`, which a reader
opens:

<details>
<summary>Is HTML safe?</summary>

Only in Markdown you trust: see below.

</details>

- HTML is rendered as it is, `<script>` too, and so is the HTML a notebook's
  code showed, so a site built from Markdown or notebooks it doesn't trust
  should set [`raw_html: false`](#choosing-markdowns-extensions), and
  `mermaid: false` with it, as a diagram can carry HTML too.
- A link or image in HTML leads where a browser takes it: little doesn't
  lead it from the folder of the file it's in, to a page from its `.md` file,
  or under the path of the site's
  [`url`](../getting-started/publishing.md#choosing-the-sites-url), and warns
  of a broken one as a reader follows it.

## Headings

Every heading gets an `id`, as on GitHub, so you can link straight to a
section, like [this one](#headings), or this page's callouts, from another
page, with `[Callouts](markdown.md#callouts)`. The built-in theme shows a `#`
that links to it when you point at the heading. A page's `##` and `###`
headings are its table of contents: see
[Navigation](navigation.md#tables-of-contents).

## Choosing Markdown's extensions

`markdown` turns each of little's extensions to CommonMark on or off. little
turns on GitHub's, so Markdown written for GitHub renders as it does there:

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
| `inline_footnotes` | `false` | footnotes in the text, like `^[a note]`; needs `footnotes` |
| `fenced_divs` | `false` | `:::name`, then a line `:::` below, for a `<div class="name">` |
| `multiline_blockquotes` | `false` | a quote between two `>>>` lines, as on GitLab |
| `hard_breaks` | `false` | a line break, kept wherever a paragraph has one |
| `cjk_friendly_emphasis` | `false` | `**` and `*` beside Chinese, Japanese, and Korean punctuation |

To turn smart punctuation on, say, and HTML off, with the diagrams that can
carry it, set them in `little.yml`:

```yaml
# little.yml
markdown:
  smart_punctuation: true
  raw_html: false
  mermaid: false
```

- Set only those you change: the rest stay as they were. A theme can set
  them in its `little.yml` too, like `math: tex` in a theme that renders
  math itself, and the site's `little.yml` has the last word. They're the
  same for every page: a page can't choose its own in its frontmatter.
- `math` is `mathml`, `tex`, or `false`, `mermaid` is `svg` or `false`, and
  `highlight` is `pygments`, `github`, or `false`. Each other must be `true`
  or `false`, unquoted, as YAML reads `yes` and `"true"` as text.
  `inline_footnotes` needs `footnotes`, and `mermaid: svg` needs `raw_html`.
  A setting little doesn't have, like `tabels`, is an error too, which lists
  those it has. Otherwise, every command stops and says why.
