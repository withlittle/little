# 0081 — Mermaid 12's look, with merman 0.10

- Status: accepted
- Date: 2026-11-19
- Amends: [0027](./0027-drawing-mermaid-diagrams.md),
  [0028](./0028-how-diagrams-look.md)

## Context
little draws Mermaid diagrams with merman
([0027](./0027-drawing-mermaid-diagrams.md)), 0.7.0, which uses chrono for
a gantt chart's today, so little depends on chrono too, to give it one.
chrono was marked as unmaintained on 17 November 2026, as
[RUSTSEC-2026-0543](https://rustsec.org/advisories/RUSTSEC-2026-0543.html).

merman 0.8 and later don't use chrono, and have a new API: a `Renderer`
that's given a request for each diagram. merman 0.10.0, the latest, draws
diagrams as Mermaid 12 does, whose default look differs from Mermaid 11's:
white boxes with a shadow, black lines, another font, and a colour for
each of a sequence diagram's participants. By default it lays flowcharts
out with ELK, which is under EPL-2.0, a licence little's `about.toml`
doesn't accept.

## Options considered
- **Which merman:** stay on 0.7.0, with chrono; or move to 0.10.0, and
  port `src/mermaid.rs` to its API.
- **Which look is little's default:**
  - Mermaid 12's, as merman 0.10 draws it, so little still sets no look of
    its own, and "Mermaid's default look" stays true as Mermaid's changes;
  - Mermaid 11's, by setting `theme: default`, `look: classic`, and
    `layout: dagre` beneath the site's config
    ([0028](./0028-how-diagrams-look.md)): nearer to how diagrams looked,
    though boxes are wider, but a look little would keep as Mermaid's
    moves on.
- **ELK:** take merman's defaults, and accept EPL-2.0; or leave its
  `layout-elk` feature out, so merman lays out with what it has.

## Decision
- little uses merman 0.10.0, without its default features: with
  `all-diagrams`, `svg`, and `layout-cytoscape`, so without ELK.
- little sets no look, so diagrams have Mermaid 12's default look. A site
  or a theme that wants Mermaid 11's sets `theme: default`,
  `look: classic`, and `layout: dagre` in `site.theme.mermaid`, which the
  manual says.
- little no longer depends on chrono itself. It gives merman its today,
  its time, and UTC, as before, in merman's own types.
- A diagram whose type merman can't tell is still warned of as
  `No diagram type detected`.

## Consequences
- Every site's diagrams look different once it builds with this version,
  and can differ in size.
- chrono stays in little's `Cargo.lock`, and so its notices, while
  lychee-lib's octocrab uses it.
- A flowchart that asks for `layout: elk` is laid out without it.
