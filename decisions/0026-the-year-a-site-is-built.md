# 0026 — The year a site is built

- Status: proposed
- Date: 2026-10-05

## Context
Many documentation sites end each page with a copyright notice, like
`© 2026 Little. All rights reserved.`, and want its year to be the current
one, without editing the theme each January. A theme can't work it out: its
templates get `site` and `page`, neither has a date, and MiniJinja has no
clock of its own.

## Options considered
- **Where the year comes from:**
  - the reader's browser, from a script in the theme, like
    `new Date().getFullYear()`: nothing for little to do, but a reader
    without scripts, or a search engine, sees no year, and it's the
    reader's year, not the site's;
  - a clock in templates, like `now()` in Hugo, in Zola, and in
    minijinja-contrib, which the theme formats, like
    `{{ now() | dateformat(format="[year]") }}`: any date, in any format,
    but just the year takes a format, in the `time` crate's syntax, each
    page reads the clock again, and a build can't be told to use another
    time;
  - the time of the build, like Jekyll's `site.time` and MkDocs'
    `build_date_utc`, which the theme formats with a filter: read once,
    but still a format to write, and a filter for little to add;
  - or the year of the build, as a number in `site`: just what a copyright
    notice needs, written as it reads, `{{ site.year }}`.
- **The time zone:** the builder's, as in Jekyll and Hugo, or UTC, as in
  MkDocs, and as CI builds in anyway, so a site built on a laptop and in CI
  at the same moment gets the same year.
- **Reproducible builds:**
  [`SOURCE_DATE_EPOCH`](https://reproducible-builds.org/specs/source-date-epoch/)
  is the time a build should act as if it ran at, so building the same
  source again gives the same files. Sphinx and MkDocs honour it.

## Decision
- The theme gets `site.year`, the year the build starts in, in UTC, as a
  number, like `2026`. little works it out once, so every page of a build
  has the same year, even one that runs over midnight on New Year's Eve.
- With `SOURCE_DATE_EPOCH` set, it's that time's year instead. It must be a
  whole number of seconds since 1970, as `date +%s` prints it, or
  `little build` and `little serve` stop and say so, as the specification
  asks.
- Like `site.nav`, it's something little works out, so it's beneath the
  config ([0006](./0006-frontmatter-and-config-win.md)): a `year` in a
  `little.yml` replaces it.
- The built-in theme doesn't show a copyright notice: whether a site has
  one, and what it says, is up to the site, so its theme writes its own,
  like `&copy; {{ site.year }} {{ site.name }}. All rights reserved.`
- No new crates: little works out the year with `time`, which lychee-lib
  already builds.

## Consequences
- A theme's notice has the right year as long as the site is built each
  year, with nothing to edit.
- A site that isn't rebuilt keeps the year it was last built in.
- The year turns over at midnight UTC, not the builder's midnight: in
  Sydney, at 11am on 1 January.
- `little serve` works the year out again with each build, so one left
  running over New Year's Eve shows the new year after the next change.
- A theme still can't show a date, like "Built on 5 October 2026", or when
  a page last changed. A `site.time`, and a filter to format it, could come
  later.
