# 0022 — Choosing Markdown's extensions, and callouts

- Status: accepted
- Date: 2026-10-04

## Context
little renders Markdown with comrak, with five of GitHub's extensions to
CommonMark on, tables, strikethrough, autolinks, task lists, and footnotes,
and no way for a site to change them. Slice 2 of little's
[roadmap](../VISION.md), authoring essentials, asks for callouts: a note or
a warning that stands out from the text around it. comrak has GitHub's, which
GitHub calls alerts, and about 30 more extensions, each off unless it's
asked for.

An author who writes on GitHub expects `:tada:` to be 🎉, and `<details>` to
fold, but little leaves `:tada:` as it is, and drops raw HTML, leaving
`<!-- raw HTML omitted -->` in its place. Others want what GitHub doesn't
have, like `==marked==` text or smart quotes. A theme that renders math, with
KaTeX or MathJax, needs comrak to find it, or `$a_1 * b_1$` is read as
emphasis. Which Markdown a site is written in is the site's to choose, like
its theme.

## Options considered
- **Where they're set:** in the cascade, like any config, so a theme can
  turn on what it renders, like math, and a site has the last word; or in a
  page's frontmatter too, which can wait.
- **What they're named:** comrak's names, like `tasklist`, `shortcodes`,
  `highlight` for `==mark==`, and `unsafe` for raw HTML; or little's own, for
  what an author writes or a reader sees, like `task_lists`, `emoji`, `mark`,
  and `raw_html`, which wouldn't change if little rendered Markdown with
  something else.
- **Which are on:** GitHub's, which authors know: tables, strikethrough,
  autolinks, task lists, footnotes, alerts, emoji, and raw HTML; or more.
  Each one more is syntax an author can trip on: with subscript, `~2~`, only
  `~~` strikes through.
- **Which are offered:** each of comrak's that changes what an author can
  write, but not:
  - heading ids, which each page's table of contents links to;
  - attributes, like `{#id .class}`, as an id an author gives a heading would
    break its link in the table of contents;
  - wikilinks, `[[page]]`, which wouldn't lead from the folder of the file
    they're in, or to a page from its `.md` file, as other links do
    ([0015](./0015-files-and-relative-links.md),
    [0017](./0017-links-to-pages-files.md));
  - chat's syntax: spoilers, underlines, subtext, and greentext, as Discord
    has, and Phoenix's HEEx templates;
  - `\(math\)` and `\[math\]`, which GitHub doesn't have;
  - frontmatter, which little reads first, and options that only change the
    HTML comrak writes, like figures with captions, which comrak writes in a
    `<p>`, where a `<figure>` can't be.
- **Callouts:** GitHub's alerts,

  ```markdown
  > [!NOTE]
  > Useful to know, even when skimming.
  ```

  which read as a quote wherever Markdown is rendered without them, so a
  README reads well on GitHub and in little; or the `:::note` fences of
  Docusaurus and VitePress, which GitHub shows as text. comrak writes an
  alert as GitHub does, a `<div class="markdown-alert markdown-alert-note">`
  with its title in a `<p class="markdown-alert-title">`, or as Sphinx does,
  an `<aside class="admonition note">`.

## Decision
- `markdown`, in the cascade, holds a setting for each extension, `true` or
  `false`. little's `little.yml` sets each one. On, as on GitHub:
  - `tables`, `strikethrough`, `autolinks` (`www.example.com`), `task_lists`,
    and `footnotes`;
  - `alerts`: GitHub's callouts;
  - `emoji`: shortcodes, like `:tada:`;
  - `raw_html`: HTML in Markdown, like `<details>`, rendered as it is.

  Off:
  - `math`: `$…$`, `$$…$$`, `` $`…`$ ``, and ```` ```math ```` blocks, as on
    GitHub, each marked with `data-math-style` for a theme's script to
    render, so a theme that renders math turns it on;
  - `smart_punctuation`: “curly quotes”, dashes for `--` and `---`, and `…`;
  - `superscript` (`^2^`), `subscript` (`~2~`), `mark` (`==text==`), and
    `insert` (`++text++`);
  - `description_lists`, `inline_footnotes` (`^[a note]`), `fenced_divs`
    (`:::name`, for a `<div class="name">`), and `multiline_blockquotes`
    (`>>>`, as on GitLab);
  - `hard_breaks`: each line break in a paragraph is a `<br>`;
  - `cjk_friendly_emphasis`: `**` and `*` work beside Chinese, Japanese, and
    Korean punctuation.
- A theme reads them as `site.markdown`, like `site.markdown.math`.
- A setting that isn't `true` or `false` stops the build, as YAML reads
  `yes` and `"true"` as text; so does a setting little doesn't have, like
  `tabels`, naming it, and listing those it has, and `inline_footnotes`
  without `footnotes`, as comrak would leave `^[a note]` as text.
- Callouts are GitHub's alerts, in GitHub's markup: `[!NOTE]`, `[!TIP]`,
  `[!IMPORTANT]`, `[!WARNING]`, and `[!CAUTION]`, in any case. An alert can
  have a title of its own, like `> [!WARNING] Before you upgrade`. The
  built-in theme gives each kind a colour, for its border and its title.
- Links in raw HTML lead where a browser takes them: little doesn't lead them
  from the folder of the file they're in, to a page from its `.md` file, or
  under the path of the site's `url`.

## Consequences
- A README from GitHub, with its alerts, emoji, and HTML, renders in little
  as it does there.
- A site's Markdown is the site's to choose, a setting at a time, and a
  theme's choices carry over to each site that uses it.
- Raw HTML that was dropped is now rendered, `<script>` too, so a site built
  from Markdown it doesn't trust should set `raw_html: false`.
- comrak's alerts differ from GitHub's: an alert can have a title, as above,
  and an empty alert, or one in a list, is still an alert, where GitHub shows
  a quote; and `>[!NOTE]`, with no space, is a quote, where GitHub shows an
  alert. Titles are in English, but an alert's own title can be in any
  language.
- Nothing new to download or build: every extension is comrak's, already
  in little, whose binary grows by about 85 KB, under 0.25%.
- Limits, for now:
  - a page can't choose its own extensions in frontmatter;
  - callouts are GitHub's five kinds, without GitHub's icons;
  - links and images in raw HTML aren't checked as they're written, but as a
    reader follows them.
