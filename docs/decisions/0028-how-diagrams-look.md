# 0028 — How diagrams look

- Status: accepted
- Date: 2026-10-06
- Amends: [0027](./0027-drawing-mermaid-diagrams.md)

## Context
little draws each Mermaid diagram as SVG as it builds the page
([0027](./0027-drawing-mermaid-diagrams.md)), in Mermaid's default look,
whatever the theme's colours and font. A theme can't make diagrams its own:
- its CSS can colour them, but each diagram's styles are scoped to its id,
  so a rule must outweigh an id, like
  `svg[id^="mermaid-"]:not(#_) .node rect`, and name the classes of each
  type of diagram, which differ; and a font set in CSS doesn't fit the
  boxes, which were sized for the font the diagram was drawn with;
- each diagram can choose its own look, in a `%%{init: …}%%` directive or
  its frontmatter's `config:`, but then its author writes the same in every
  diagram, and a theme can't write it for them;
- or the theme sets `markdown.mermaid: false` and draws diagrams itself,
  with mermaid.js, giving up what 0027 gained.

Mermaid's own answer is a site's config, which a site passes to
`mermaid.initialize()`, and which every diagram starts from: its `theme`,
its `themeVariables`, for colours and fonts, `themeCSS`, `look`, and
settings for each type of diagram, like `gantt.useWidth`. A diagram's own
config goes over it. merman takes the same config, as its site config.

## Options considered
- **Where the config goes in `little.yml`:**
  - `mermaid`, holding Mermaid's own config, in Mermaid's own names, as
    Docusaurus' `themeConfig.mermaid.options` and VitePress' Mermaid
    plugin hold it: just what Mermaid's docs describe, and what an author
    writes in a diagram's own config, so either can be copied to the other;
  - under `markdown`, like `markdown.mermaid_config`, beside
    `markdown.mermaid`: but `markdown` holds the extensions an author
    writes with, each little's own, and checked;
  - `markdown.mermaid` itself, as settings rather than `svg`: one key, but
    a site setting a look would undo its theme's `false`, as `true` would;
  - settings of little's own, like `diagrams: { accent: "#6f42c1" }`: named
    as little names things, but fewer than Mermaid's, mapped onto them, and
    documented again.
- **Which settings little checks:** Mermaid has hundreds, for more than 20
  types of diagram, and adds more with each version. merman ignores one it
  doesn't know, as Mermaid does. little could check:
  - each setting's name and type, against Mermaid's schema, which little
    would keep up with Mermaid's;
  - none, so a mistake does nothing, as in a diagram's own config;
  - or just the likeliest mistakes, which change every diagram without a
    word: a `theme` Mermaid doesn't have, like `Dark`, which it swaps for
    its default, and a colour written without quotes, like
    `primaryColor: #6f42c1`, which YAML reads as a comment, so the setting
    is empty, and Mermaid works out odd colours from nothing.
- **little's own settings for Mermaid,** its seeds and a gantt chart's
  width ([0027](./0027-drawing-mermaid-diagrams.md)): in `src/little.yml`,
  where `little config` shows them, but where they'd look like a look, and
  every theme would get a `site.mermaid`; or in code, as now, beneath the
  site's config.
- **Dark mode:** each diagram drawn twice, in a light and a dark look, for
  the theme's CSS to show one, as Docusaurus chooses a theme for each; or
  one look, for now.

## Decision
- `mermaid` holds Mermaid's config, in Mermaid's own names, like `theme`,
  `themeVariables`, `themeCSS`, `fontFamily`, `look`, and `gantt`, which
  little draws every diagram with, as a site passes it to
  `mermaid.initialize()`.
- It cascades like any config: little sets none, as a site's look is the
  theme's to default and the site's to set; a theme sets its look in its
  `little.yml`, and the site's has the last word, setting by setting, so a
  site can change one of its theme's colours and keep the rest.
- A diagram's own config, in a `%%{init: …}%%` directive or its
  frontmatter, goes over it, as in Mermaid.
- little's own settings stay in code, beneath it: its seeds, and a gantt
  chart's width, 800 pixels, which a theme with a wider or narrower page
  can change, with `gantt: { useWidth: 1000 }`.
- little checks only that:
  - `mermaid` is settings;
  - `theme` is one of Mermaid's: `default`, `base`, `dark`, `forest`,
    `neutral`, `neo`, `neo-dark`, `redux`, `redux-dark`, `redux-color`, or
    `redux-dark-color`;
  - no setting is empty, naming the one that is, and asking for a colour
    in quotes.

  Each stops the build and says why. Any other setting is passed on as
  it's written: one Mermaid doesn't have does nothing, as in a diagram's
  own config.
- little reads `mermaid` only to draw diagrams: with `markdown.mermaid:
  false`, it doesn't check it either. A theme gets it as `site.mermaid`, as
  it does every setting.
- No new crates.

## Consequences
- A theme gives diagrams its colours and font once, in its `little.yml`,
  and a site changes them as it changes any of its theme's settings.
- Settings can be copied from Mermaid's docs, and from a diagram's own
  config, as they're written.
- A typo in a setting's name, like `themeVariable` or `theme_variables`,
  does nothing, without a word.
- Mermaid's own rules apply: `themeVariables` change only the `base`
  theme, and Mermaid works out other colours from them, so a colour must be
  hex, like `"#6f42c1"`; a named colour, like `red`, gives odd colours. A
  diagram's own `themeCSS` replaces the site's, rather than adding to it.
- merman measures each label's text without its font, from metrics for
  Mermaid's default font, Courier, and a generic sans-serif, so in another
  font a label can fit a little loosely, or tightly.
- Limits, for now:
  - a build draws each diagram in one look, so diagrams still can't turn
    dark in dark mode; a dark theme can choose `theme: dark`;
  - a page can't choose its diagrams' look in its frontmatter; each diagram
    still can;
  - a site can change its theme's settings, but not remove one, as with
    any setting.
