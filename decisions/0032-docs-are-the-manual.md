# 0032 — little's docs are its manual

- Status: accepted
- Date: 2026-10-09

## Context
`docs/` began as an example site, to try little and its built-in theme on:
a page or two for each feature, written to show it off, like "What this
skeleton already does". Meanwhile, the README grew into little's manual,
1,400 lines long, with every setting in it, and AGENTS.md said `docs/`
isn't project documentation. So the example drifted: it listed plugins as
coming soon after they'd shipped. little's docs will be published at
<https://withlittle.github.io/little>, for people who use little, and the
manual should be what's there.

## Options considered
- **Where the manual lives:**
  - in the README, as now, with `docs/` an example: one long page, which a
    reader scrolls, and searches with their browser, while the site that
    shows little off doesn't say how to use it;
  - in both, in full: every feature documented twice, in two places that
    drift;
  - or in `docs/`, with the README pointing to it: the manual is a little
    site, so it shows what little does as it says how, and the README says
    what little is, how to install it, and where to read more.
- **How its pages are grouped:** by what little does, as the README is, or
  by what a reader is trying to do: get started, write pages, choose or make
  a theme, use or write a plugin, or look something up.
- **How its pages are ordered:** little sorts the sidebar by title, so
  "Configuring" would come before "Install". A site can write its `nav` in
  `little.yml` by hand; or little could let a page give its own place, as
  roadmap slice 1 plans, which is a change to little of its own.

## Decision
- `docs/` is little's manual, for people who use little, published at
  <https://withlittle.github.io/little>. Its pages are grouped by what a
  reader is trying to do, in folders, each without a page of its own:
  Getting started, Writing, Themes, Plugins, and Reference.
- The README says what little is, how to install it, and how to start, and
  links to the manual. The `docstrings` plugin's README keeps what's for
  developing it, and links to its page in the manual.
- Each page uses what it documents, where it can, so the manual still tries
  little out: the page on math has math in it, the page on diagrams draws
  them, and the page on notebooks is a notebook.
- Pages link to each other by their files, like `../writing/math.md`, so the
  manual reads on GitHub as it does on the site. Links to little's own
  files, like `src/little.yml`, are to them on GitHub, which little doesn't
  check.
- `nav`, in `little.yml`, orders the pages, by hand, for now.
- A change people using little can see updates its page in the manual, and
  the reference pages, in the same pull request, and a new page goes in
  `nav`. Meta docs, like this one, stay at the root of the repository.

## Consequences
- The manual is a site, so it can be searched and navigated, and the README
  is short enough to read.
- A page that isn't in `nav` is still built, but isn't in the sidebar, and
  has no breadcrumbs, or previous and next pages, so a new page needs adding
  to `nav` by hand, until a page can give its own place.
- The manual describes `main`, not a release: until little has releases,
  and versions of its docs (roadmap slice 6), it can describe what a
  release doesn't have yet.
- Until the repository is public, and GitHub Pages publishes the site, the
  manual is read in `docs/` on GitHub, or with `little serve`.
- Publishing it, with a workflow that builds it and deploys it to GitHub
  Pages, is a change of its own.
