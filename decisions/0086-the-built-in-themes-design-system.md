# 0086 — The built-in theme's design system

- Status: accepted
- Date: 2026-11-23
- Amends: [0033](./0033-the-built-in-themes-design.md),
  [0038](./0038-the-built-in-themes-page-and-tables.md),
  [0080](./0080-settings-in-groups-under-site-and-page.md)

## Context
The built-in theme's layout, its light and dark, and its accessibility are
[0033](./0033-the-built-in-themes-design.md)'s, and they hold up. Its look
grew a rule at a time, though, and it shows:
- its stylesheet had eleven font sizes, six weights, two of which only a
  variable font draws, and eighteen spaces, each chosen for its own rule;
- its colours came from three places: an accent from Tailwind's violet,
  code and alerts in GitHub's colours, and greys of its own;
- a site could set one thing, `site.theme.fonts.body`, whose default was
  written in the stylesheet, the template, and the theme's `little.yml`,
  two ways.

[0080](./0080-settings-in-groups-under-site-and-page.md) named the look
`site.theme` is to hold: a logo, fonts, and colours, each colour as a light
and a dark. What makes Tailwind's and Bootstrap's sites look finished is
that they're built from a system: a few scales, which everything is
measured in.

## Options considered
- **Tailwind**, which 0033 turned down, as it is at 4.3:
  - its engine is TypeScript, so little can't run it, and its CLI without
    Node is 80 to 106 MB, with a JavaScript runtime in it. Hugo dropped
    that CLI in 0.161, and asks for npm;
  - CSS the theme builds and commits has only the classes its templates
    use, so a theme that builds on it can't use another;
  - utility classes reach only the theme's own markup, and over a third of
    the stylesheet styles HTML that little, plugins, Pagefind, and merman
    write;
  - encre-css, in Rust, writes Tailwind 4's classes, and little could
    compile with it as it does Sass with grass
    ([0018](./0018-sass-in-themes.md)). It's for a theme's author, though,
    and one more thing that follows another project.
- **Bootstrap**, compiled from its Sass by grass: a system, with components
  a page of docs doesn't need, and variables that are gone once it's
  compiled, so a site couldn't set one.
- **A system of the theme's own**, in plain CSS: custom properties in
  tiers, which a site sets, and a theme that builds on it reads.
- **A colour's light and dark:** a pair for each, as 0080 had it, which a
  site must choose so that both read; or one colour, which the theme takes
  the hue and strength of, and gives a lightness of its own, for each use,
  with CSS's relative colours.
- **Where a setting's default is:** in the stylesheet, which then works
  without the template, but hides it from `little config`; or in the
  theme's `little.yml`, as a site's look is the theme's to default there,
  which the template then writes into each page.

## Decision
- The built-in theme stays plain CSS, in 0033's layers, and is built from
  **tokens in three tiers**, each made only from the one above:
  1. **settings**, which a site sets;
  2. **scales**, the ramps and steps worked out from them;
  3. **roles**, what each colour is for, like `--color-link`.

  A rule takes its colours from the roles, and its type, space, and
  corners from the scales. The names are Tailwind 4's, where both have the
  thing: `--font-sans`, `--text-sm`, `--font-weight-semibold`,
  `--leading-tight`, `--radius-lg`, and `--shadow-sm`.
- **Settings**, under `site.theme`, which amends 0080's:

  | Setting | Custom property | Default |
  | --- | --- | --- |
  | `colors.primary` | `--primary` | `"#7c3aed"` |
  | `colors.info`, `success`, `warning`, `danger` | `--info`, and so on | a blue, a green, an amber, and a red |
  | `fonts.body`, `fonts.code` | `--font-sans`, `--font-mono` | the system's |
  | `fonts.heading` | `--font-heading` | none: the body's |
  | `radius` | `--radius` | `0.5rem` |

  Each colour is one colour, not a pair. A scale takes only its hue and its
  strength, with `oklch(from var(--primary) …)`, and sets the lightness, so
  a pale primary still reads as a link, in light and dark.
- **Scales:**
  - grey, in eleven steps, 50 to 950, in the primary's hue, with almost
    none of its strength, so the greys belong with it;
  - the primary, at the same eleven steps;
  - eight type sizes, 12px to 36px, four weights, and three line heights;
  - twelve spaces, in steps of 4px, with two half steps;
  - four corners, each a share of `radius`, and three shadows.
- **Roles:** the page's four backgrounds, two borders, the heading's, the
  text's, and muted text's colours, the link's and its hover's, an accent,
  for markers and the focus ring, and code's. The primary is never used as
  it's set.
- **One recipe for alerts:** each kind sets one colour, `--alert`, which
  its text, background, and border are worked out from. Note, Tip, Warning,
  and Caution take `info`, `success`, `warning`, and `danger`; Important
  takes the primary; and the note on an old version
  ([0040](./0040-versions-from-git-refs.md)) takes `warning`. Code's six
  colours are six hues at one lightness. This amends 0033, whose alerts
  and code were in GitHub's colours.
- **The defaults are in the theme's `little.yml`**, and nowhere else. The
  template writes each setting into the page, in a `<style>`, in the
  `little.tokens` layer, so a stylesheet outside a layer still wins. It
  writes a `radius` of 0 as `0px`, as the scales multiply it.
- little checks one thing: a setting under `site.theme.colors` that's
  empty stops the build, as one under `site.theme.mermaid` does
  ([0028](./0028-how-diagrams-look.md)), since YAML reads `#7c3aed`
  without quotes as a comment.
- **The look:**
  - the nav's pages hang from a rail, with a marker on the page this is,
    as the table of contents has;
  - a table is rows between hairlines, with no frame, which amends
    [0038](./0038-the-built-in-themes-page-and-tables.md): its script
    still puts each in a box that scrolls;
  - an alert has a border in its colour, and no bar down its side;
  - the page starts beside the nav, not in the middle of its column, and
    the search button lines up with it. A site of one page is still
    centred;
  - the footer is under the page, and the page's text is at most 45rem
    wide.
- Pagefind's styles reset every property of its own element, and aren't in
  a layer, so the theme sizes a box around the search button,
  `.header-search`, and sets Pagefind's variables on it.
- A few values are still a rule's own: an icon's and a chevron's size, a
  hairline's width, the size of code and keys within text, in `em`, and
  the site's name's line height
  ([0039](./0039-a-long-site-name-in-the-built-in-themes-header.md)).
- **Contrast:** with eight primaries, from yellow and lime to navy and a
  mid grey, measured in Chrome, the least was 4.9:1 for a link, where
  WCAG's AA asks 4.5:1, and 3.4:1 for a marker, where it asks 3:1.
- **Browsers:** relative colours have been in every major browser since
  September 2024, so what the theme needs is still Baseline 2024, as 0033
  has it.
- No new crates.

## Consequences
- A site gives the theme its colour, its fonts, and its corners in a few
  lines of `little.yml`, and `little config` shows what they are.
- A theme that builds on the built-in one sets `--primary`, not `--accent`,
  and `--color-diagram-bg`, not `--diagram-bg`, and one with a
  `default.html` of its own writes the settings into `:root`, as the
  built-in one's does, or its pages have no colours. So little's minor
  version rises.
- The stylesheet grows from about 1,240 lines to about 1,400.
- Limits, for now:
  - a logo, a favicon, links in the header, a footer of a site's own, and
    a stylesheet of a site's own aren't settings yet;
  - a font's name loads nothing, so a font that isn't the reader's own
    needs a theme that builds on this one;
  - a value that isn't a colour isn't checked: CSS drops it, and the page
    loses its colours;
  - diagrams still have Mermaid's look, whatever the primary
    ([0028](./0028-how-diagrams-look.md));
  - `fonts.math`, which 0080 named, isn't a setting: math is in the
    reader's math font.
