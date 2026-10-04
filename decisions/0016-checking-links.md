# 0016 — Checking links with lychee

- Status: accepted; amended by [0017](./0017-links-to-pages-files.md),
  which shows a broken link in Markdown as it's written, and by
  [0025](./0025-rendering-math-as-mathml.md), which warns of math that can't
  be rendered as of a broken link, and fails on it with `--strict`
- Date: 2026-10-01

## Context
A link to a page that moved, an image that was renamed, or a `#fragment` whose
heading changed breaks without a word: the build succeeds, and a reader finds
it. MkDocs warns of each one as it builds, which makes them easy to find and
fix. A site's links come from its theme as well as its Markdown, and those to
other sites can only be checked over the network.

## Options considered
- **Our own checker:** read the built HTML, and look for each link's file in
  `dist/` and its `#fragment` among the `id`s of that page. It's small to
  start with, but it's code little would own, and it would grow to check
  other sites, with retries, rate limits, and GitHub's own fragments, as
  well-maintained checkers already do.
- **An external checker,** like lychee or html-proofer, run on `dist/`:
  nothing to build, but every author has another thing to download and run,
  and to set up for a site under a path.
- **[lychee-lib](https://crates.io/crates/lychee-lib),** the library behind
  lychee, built into little: it reads HTML, and checks files, `#fragments`,
  and other sites, with retries and limits on how often it asks each one.
- **When to check:** on every build, as MkDocs does, or only when asked. A
  broken link is worth knowing about whether or not you thought to ask.

## Decision
- `little build` checks every link in the site once it's built, and warns of
  each one that's broken, but still succeeds. `little build --strict` fails
  on one, once `dist/` is written.
- `little serve` checks only the links within the site after each build, so
  a save needs no network and doesn't wait for other sites.
- The built HTML is checked, not the Markdown, so the theme's links are too.
  A warning names each file a link is in by what it was built from, like
  `docs/guide.md`. It shows the link as a reader follows it: one within the
  site from the root of the domain, like `/little/guide/install#from-source`,
  and one to a page at its URL.
- A link is checked once, however many files it's in, and warned of once,
  naming three of them and counting the rest.
- The site is checked as it's published: a link from the root of the domain
  is found in `dist/` only under the path of the site's `url`, and a link to
  the site's own `url` is found in `dist/`, not on the web.
- Links that only work on the author's machine, like `localhost` and private
  IP addresses, aren't checked, nor, as in lychee, are example domains,
  `mailto:` and `tel:` links, or URLs in code.
- New crates: `lychee-lib`, without its default features; `tokio` and
  `futures`, to run it; and `regex` and `url`, which it uses too, to lead the
  site's URLs into `dist/`.

## Consequences
- Broken links are found as a site is built, with nothing else to install,
  and CI can fail on them with `--strict`.
- little grows from about 8.6 MB to about 22.6 MB, and from about 127 crates
  to about 452. Compiling it from scratch takes about 2 minutes, not 33
  seconds, but only once, as cargo keeps what it's compiled.
- little has two HTTP clients: `ureq`, which downloads themes
  ([0004](./0004-downloading-themes.md)), and lychee's `reqwest`, with the
  async runtime 0004 avoided.
- Limits, for now:
  - links to other sites are checked on every build, with no cache, so
    without a network each one warns, and one to a site that doesn't answer
    holds a build up until it times out, which can take about a minute;
  - some sites turn link checkers away, so a link to one can warn even
    though it works, and there's no way to skip one;
  - only HTML is checked, not CSS, and a warning names the file, not the
    line;
  - lychee drops the `/` from the end of a link within the site, so one to a
    page or folder gets it back, but `/guide/` that isn't there is shown as
    `/guide`.
