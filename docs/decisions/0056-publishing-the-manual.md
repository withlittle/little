# 0056 — Publishing the manual

- Status: accepted
- Date: 2026-10-19

## Context
little's manual, in `docs/`, is a little site
([0032](./0032-docs-are-the-manual.md)), whose `url` is
`https://withlittle.github.io/little`, where the README, and `little-docs`
on crates.io, send readers. Nothing publishes it there: 0032 left that to
a change of its own, so it's read in `docs/` on GitHub, or with
`little serve`. Nor does CI build it, so a broken link in it is found only
by building it by hand, as
[AGENTS.md](https://github.com/withlittle/little/blob/main/AGENTS.md) asks
of each change.

The manual's [Publishing](../getting-started/publishing.md) page gives a
site a workflow for GitHub Pages that downloads a release of little, by its
version, and builds the site with `--strict`. The manual isn't quite such a
site: it documents `main`, which can have what no release does yet; its
theme is `./theme`, the built-in theme's files as they are on `main`; and
the `rustdoc` plugin, which adds its `/api/`, is built from the repository
too, for the `little:plugin` that `main`'s little calls.

## Options considered
- **Which little builds it:**
  - a release, downloaded, as the Publishing page's workflow does, in
    seconds, but a page, template, or plugin that needs what's on `main`,
    and not yet released, breaks the build;
  - or little, built from the same commit, as `cargo run -- build` does,
    which takes a few minutes, but less once Cargo's cache has its
    dependencies, so the manual is always built by the little it
    documents.
- **When it's published:**
  - on each push to `main`, so a fix to a page is published at once, but a
    feature is documented from when it's merged, before it's released;
  - on each release's tag, so the manual matches the latest release, but a
    fix to a page waits for the next one;
  - or as [versions](./0040-versions-from-git-refs.md), the latest
    release's at `/`, from its tag, and `main`'s at a path of its own, with
    each release's pull request moving the first to its tag.
- **Pull requests:** not built, as now; or built, but not published, so a
  broken link, or a page that doesn't build, fails there, not on `main`.

## Decision
- `.github/workflows/docs.yml` builds the manual with little from the
  same commit: it builds the `rustdoc` plugin, into the plugin's own
  `target/`, where `little.yml` looks for it, then runs
  `cargo run --release --locked -- build --strict`, so a broken link,
  within the manual or to another site, or anything else little warns of,
  stops it.
- Each push to `main` publishes the manual, from `dist/`, on GitHub Pages,
  at `https://withlittle.github.io/little`, with GitHub's
  `upload-pages-artifact` and `deploy-pages` actions, as the Publishing
  page's workflow does. It can also be run by hand, from the Actions tab,
  which publishes only `main`'s.
- A pull request's run builds the manual, but doesn't publish it, and a
  newer run cancels an older one. Runs on `main` wait for each other, so a
  deploy isn't cut short, and one waiting gives way to a newer one, so the
  latest commit's manual is the one published.
- The repository's Pages are published by GitHub Actions, and only `main`
  can deploy to its `github-pages` environment. Its website, in its About,
  is the manual.

## Consequences
- The manual at `https://withlittle.github.io/little` is `main`'s,
  published a few minutes after each merge, so it can document a feature
  before a release has it, as 0032 says it may, until the manual has
  versions.
- A link to another site that's down, or turns link checkers away, fails
  the build, as the Publishing page warns: the manual that's published
  stays, and the run can be run again once the site's back, or the link's
  gone. On a pull request, it fails a check that has nothing to do with the
  change.
- Each pull request, and each push to `main`, builds little twice, in CI
  and for the manual, which is free on GitHub's runners for a public
  repository.
- The Publishing page's workflow, which downloads a release, is still the
  one for sites: only a site in little's own repository can build little
  from the commit.
