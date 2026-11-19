# 0082 — The dates of a site and its pages

- Status: accepted
- Date: 2026-11-19
- Supersedes: [0026](./0026-the-year-a-site-is-built.md)
- Amends: [0027](./0027-drawing-mermaid-diagrams.md),
  [0029](./0029-plugins-that-render-code-blocks.md),
  [0043](./0043-a-versions-own-nav-and-plugins.md),
  [0080](./0080-settings-in-groups-under-site-and-page.md)

## Context
A theme gets one date from little: `site.year`, the year of the build, for
a copyright notice ([0026](./0026-the-year-a-site-is-built.md)). It can't
show the day a site was built, when a page was written, or when it last
changed, which a reader looks for to judge whether a page is still right.
0026 foresaw a `site.time`, and a filter to format it.

`site.year` is one moment, cut down to what a notice needs, so a theme
that wants its month has nothing to ask for. And it's the build's moment,
where a notice's year is meant to be the work's: a site built again in
January, with nothing changed, claims the new year.

Two moments matter to a site: when it was built, which the clock knows,
and when its source last changed, which only its history knows. A page
has two of its own: when it was created, and when it last changed. Most
sites keep their history in git, which little already reads a site's
versions from ([0040](./0040-versions-from-git-refs.md)).

## Options considered
- **What the theme gets:** a year, as now; or each moment whole, and a
  filter to format it, so a theme writes the year, the month, or the day.
  Beside them, `site.year` would be a second way to write the year of
  `site.built`.
- **Where a page's dates come from:**
  - its frontmatter alone, as Zola reads `date` and `updated`: exact, but
    every author has to keep them;
  - its file's times on disk, as Hugo can: nothing to keep, but git
    doesn't store them, so each file has the time it was cloned or checked
    out. In CI every page would be created and changed today, and two
    builds of one commit would differ;
  - git's history, as Docusaurus and VitePress read a page's last change,
    and Hugo can: nothing to keep, and the same for everyone who builds a
    commit.
- **When git can't say,** as for a page that isn't committed, or in a
  shallow clone, where the oldest commit seems to add every file:
  - fall back to the file's time, or the build's, so every page has
    dates, but some are wrong, and nothing marks which;
  - or leave the date out, so a theme shows only dates that are true.
- **Whether to warn of a shallow clone,** which is how CI often checks a
  repository out, and the manual's own does: a warning would say why a
  site's pages have no dates, and `--strict` would catch it. But little
  can't tell whether a theme shows dates, so it would warn every site
  built that way, and fail `--strict` for those that show none.
- **A setting to turn pages' dates on,** for a theme that shows them to
  set: little would read the history only when asked, and could warn when
  it can't. But it's a setting to learn for a value that could just be
  there, and git gives a history quickly.
- **Which of a commit's dates:** its author's, when the change was
  written, which a rebase keeps; or its committer's, when the commit
  joined this history, which git orders commits by, and
  `SOURCE_DATE_EPOCH` is usually set from.
- **A file that's renamed:** a new file, as git records no renames, so a
  page that's moved was created that day; or the same file where git
  finds it is, as `git log --follow` does, so pages keep their history
  when a site is reorganised.
- **When a site last changed:**
  - the newest of its pages' dates, which misses a change to its config
    or its theme;
  - the newest commit to change a file the build reads, which little
    would have to list, and a shallow clone can't give;
  - or the commit it's built from, `HEAD`: one question for git, right
    even in a shallow clone, though in a repository the site shares, a
    commit that doesn't touch it counts.
- **`site.modified` when git can't say:** the build's time in its place,
  so it's never missing, and a notice needs no test. But a site can be
  built years after it last changed, and one name would mean two things.
  With the build's time under its own name, always there, a theme that
  wants one for the other says so, with `default`.
- **`SOURCE_DATE_EPOCH`:** its
  [specification](https://reproducible-builds.org/specs/source-date-epoch/)
  has a build use it in place of the current time, and says to set it to
  when the source last changed. It could stand in for `site.modified`
  too, where git can't say; but that's a second source for it, so it
  stands in only for the clock.
- **The time zone:** UTC, as 0026 chose for the year, so a commit has the
  same date wherever it's built; each commit's own, so a site's dates
  would be in the zones of whoever committed; or the site's, as a
  setting, which needs the time zone database.
- **The filter:**
  - `dateformat`, from minijinja-contrib: a new crate, whose formats are
    in the `time` crate's syntax, like `[day] [month repr:long] [year]`,
    or by name, like `full`, in English;
  - or one of little's own, whose formats are `strftime`'s, like
    `%-d %B %Y`, as in Jekyll's `date` filter, Zola's, and Sphinx's
    `today_fmt`, which `time` reads too.
- **The time in `little serve`:** worked out for each rebuild, as the
  year is; or once, as it starts. `site` is part of what each page's
  render is kept by ([0076](./0076-rebuilding-only-what-differs.md)), so
  a time that changed with each rebuild would render every page on every
  save.

## Decision
- The theme gets four dates, and no `site.year`:

  | Date | When |
  | ---- | ---- |
  | `site.built` | the build started |
  | `site.modified` | the commit the site is built from was made |
  | `page.created` | the page's file was added |
  | `page.modified` | the page's file last changed |

  Each is text: a date and time in UTC, as RFC 3339 writes it, like
  `2026-11-19T02:37:42Z`, which a `<time>`'s `datetime` takes as it is.
- `site.built` is the time the build starts, or `SOURCE_DATE_EPOCH`, if
  it's set, which must be a whole number of seconds since 1970, or
  `little build` and `little serve` stop and say so. It's always there.
  It's the time a gantt chart has as its today
  ([0027](./0027-drawing-mermaid-diagrams.md)), and a plugin as its clock
  ([0029](./0029-plugins-that-render-code-blocks.md)).
- `little serve` works that time out once, as it starts, for every build
  it makes, so a rebuild still renders only the pages that differ.
- The other three are from git alone: from the repository the site's
  folder is in, by each commit's committer date.
  - `site.modified` is `HEAD`'s, in every version of the site.
  - `page.modified` is that of the newest commit to change the page's
    file, and `page.created` that of the commit that added it, or the
    last to, if it was removed and added again. A file git finds was
    renamed keeps the history it had by its old name. A commit that merges
    others isn't counted: those it merges are.
  - A version from a `ref` has its pages' dates from the history of its
    ref.
  - Changes that aren't committed move none of them.
- A date git can't give is left out, not guessed, as a page leaves out
  anything it doesn't have:
  - all three, when the site isn't in a repository, or its repository has
    no commits, or git isn't installed, or can't read it;
  - a page's two, when its file isn't committed, or is outside the
    repository, or a plugin added it, or it has none, like the 404 page;
  - every page's two, in a shallow clone. `site.modified` is still
    there, as a shallow clone has its newest commit whole.

  little warns of none of these.
- little reads each version's history with one `git log`, of its source
  folder, not one for each page. `little serve` reads it again only when
  the version's commit has changed.
- Like the nav, the dates are what little works out, so they're beneath
  the config ([0006](./0006-frontmatter-and-config-win.md)): a `created`
  or `modified` in a page's frontmatter, or under `page` in a
  `little.yml`, replaces the page's, and a `built` or `modified` under
  `site` the site's. Each reaches the theme as it's written.
  `little config` prints none of them. An entry in `site.versions` can
  set neither `built` nor `modified`, as it couldn't set `year`
  ([0043](./0043-a-versions-own-nav-and-plugins.md)).
- Plugins get them in their `site` and `page` hooks, as the theme does,
  so a plugin can give the pages it adds dates.
- little adds a filter, `dateformat`, which writes a date as its `format`
  says, in `strftime`'s codes:

  ```html
  {% if page.modified %}
  <p>Last updated on <time datetime="{{ page.modified }}">
    {{ page.modified | dateformat(format="%-d %B %Y") }}</time>.</p>
  {% endif %}
  <footer>&copy; {{ site.built | dateformat(format="%Y") }} {{ site.name }}</footer>
  ```

  - It reads a date and time as RFC 3339 writes it, or a date alone, like
    `2026-11-19`, as an author writes one in frontmatter, which it takes
    as midnight, UTC. It writes a time in the zone it's written in, so
    little's in UTC.
  - `format` must be given: how a date reads is the theme's to choose, so
    little has no format of its own, and none by name.
  - A value that isn't there is left as it is, as `url` leaves it, so
    nothing is written. One that isn't a date, or a `format` with a code
    `time` doesn't know, stops the build, which says so.
- The built-in theme shows none of them, as it shows no copyright notice.
- No new crates: little turns on `time`'s `formatting` and `parsing`
  features, which lychee-lib's dependencies already build it with.

## Consequences
- A theme can show when each page was created and last changed, and date
  its notice by the build, or by the site's last change, with the build's
  where git can't say:
  `{{ site.modified | default(site.built) | dateformat(format="%Y") }}`.
- Every theme that writes `{{ site.year }}` changes, to
  `{{ site.built | dateformat(format="%Y") }}`. Until it does, it shows
  no year, and nothing says so. A `year` under `site` in a `little.yml`
  is a key of the site's own, for its theme. So little's minor version
  rises.
- Two builds of one commit have the same `site.modified`, and the same
  dates for each page. Only `site.built` differs, unless
  `SOURCE_DATE_EPOCH` sets it.
- A site's CI must fetch its repository's whole history for its pages to
  have dates, as it must fetch each version's ref
  ([0040](./0040-versions-from-git-refs.md)). In a shallow clone they
  have none, and nothing says why.
- A theme tests for a date before it writes around it, as any but
  `site.built` can be missing.
- A site in a repository runs git at each build: for the commit it's
  built from, for when that was made, and for each version's history. For
  the manual, in a repository of 379 commits, the three take about a tenth
  of a second in all, on an Apple silicon Mac, and a longer history takes
  longer. A rebuild in `little serve` runs it once, unless there's another
  commit.
- A `little serve` left running keeps the time it started at, as
  `site.built`, a gantt chart's today, and a plugin's clock, until it's
  started again. 0026 had it work the year out with each build.
- Limits, for now:
  - every date is in UTC, so a commit made in Sydney at 9am on 1 January
    is dated 31 December;
  - `%B`, `%b`, `%A`, `%a`, and `%p` are in English, whatever the site's
    `language`. A format of numbers, like `%Y-%m-%d`, reads in any, and a
    theme's script can write a `<time>` in its reader's language and
    zone;
  - a page's dates are its file's, so a change to an image on it, or to
    the code its API reference is made from, doesn't move them;
  - a file moved into the source folder from elsewhere in the repository
    is created the day it arrives;
  - in a repository a site shares with other things, `site.modified`
    moves with every commit, whether or not it changes the site.
