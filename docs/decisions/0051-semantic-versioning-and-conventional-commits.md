# 0051 — Semantic Versioning and Conventional Commits

- Status: accepted; amended by
  [0058](./0058-features-raise-the-minor-version.md), which raises the
  minor version, not the patch, for a feature until 1.0, but for
  `little:plugin`'s
- Date: 2026-10-17

## Context
little, with its built-in theme, and the `pydoc`, `rustdoc`, and `jsdoc`
plugins are each to be released on their own, from a tag that names them
([0050](./0050-one-repository-for-little-its-themes-and-plugins.md)).
None has been released yet. Each `Cargo.toml` says 0.1.0, which
`little --version` prints, but nothing says what the next number will
mean. Nor does a commit say which component it changes, or whether it
breaks a site, a theme, or a plugin, so a release's number, and its notes,
would be worked out by reading every commit since the last.

The plugin interface, `little:plugin`, has a version of its own, 0.2.0, in
`wit/plugin.wit`, and it isn't only for people to read: little finds a
plugin's hooks, and gives it its imports, with wasmtime, only if the plugin
was built for a compatible version. wasmtime reads 0.2.0 and 0.2.1 as
compatible, and 0.2.0 and 0.3.0 as not, as Cargo reads crates' versions.

## Options considered
- **Versions:**
  - [Semantic Versioning](https://semver.org/), or SemVer, from 1.0.0,
    with its rules for each number: a breaking change raises the major
    version, a feature the minor, and a fix the patch. But little still
    breaks often, and SemVer's own advice, for an API that changes every
    day, is to stay at 0.y.z. `little:plugin` would go to 1.0.0 too,
    which would break every plugin once; kept below 1.0, each hook added
    would raise it to 0.3.0, then 0.4.0, and break them all again.
  - SemVer, at 0.y.z for now. Its rules for each number apply only from
    1.0.0: until then, "anything MAY change at any time". Its FAQ suggests
    raising the minor version for each release, but then no number says
    whether an update is safe. Cargo's
    [convention](https://doc.rust-lang.org/cargo/reference/semver.html)
    does: in 0.y.z, a change to y can break, and a change to z can't.
  - Calendar versions, like 2026.10.0, which say when, but not whether an
    update breaks anything.
- **Commits:**
  - [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/),
    checked in CI. [committed](https://github.com/crate-ci/committed), from
    crate-ci, is a Rust binary, with a GitHub Action; commitlint needs
    Node; and cocogitto checks commits as part of a tool that releases
    too, which is for releases to choose, as they're set up.
  - Squash merging, with each pull request's title checked, which leaves
    one commit for each. But 23 of little's 62 pull requests had more than
    one, which a squash would lose.
  - Conventional Commits, unchecked, which are easy to forget.

## Decision
- **Versions:** little, `pydoc`, `rustdoc`, and `jsdoc` follow SemVer
  2.0.0, each with its own version, in its `Cargo.toml`, raised as it's
  released. `api_reference` is released only inside the plugins, and
  `tests/plugin` isn't released.
  - Until 1.0.0, where SemVer sets no rules, each follows Cargo's
    convention: a breaking change raises the minor version, from 0.1.4 to
    0.2.0, and anything else, a feature or a fix, the patch version, from
    0.1.4 to 0.1.5.
  - A component goes to 1.0.0 once what it gives sites, themes, and
    plugins stops breaking: for little, the plugin interface and templates'
    data. From then, SemVer's own rules apply.
  - `little:plugin` has its own version, by the same rules, raised by the
    change to `wit/plugin.wit` that needs it: a new hook, or a new
    function for plugins to call, makes it 0.2.1, as plugins built for
    0.2.0 still load; any other change, even a function added to a hook,
    which plugins that export it would lack, makes it 0.3.0. little moving
    to a version that breaks plugins is a breaking change of little's.
  - A version, once released, is never changed, nor its tag moved; a
    mistake is fixed in the next one.
- **What's public:** SemVer needs a public API, to tell a breaking change
  from others. It's what the manual documents for people to rely on:
  - little's: its commands and their options; `little.yml`'s settings and
    their defaults; frontmatter's keys; the data and filters templates
    get, and which template each page uses; the HTML little writes for a
    theme to style, like highlighted code's classes; the files and URLs a
    build writes; the built-in theme's files, by their paths, for a theme
    that builds on it; and the versions of `little:plugin` it loads.
  - A plugin's: its settings, the pages it adds and their URLs, and the
    version of `little:plugin` it's built for.
  - Not public: how the built-in theme looks, the wording of messages, and
    little's Rust items, at `/api/`, as little isn't a library.
- **Commits** follow Conventional Commits 1.0.0:
  `<type>(<scope>)!: <description>`, with a lowercase, imperative
  description, as the spec's examples have, and no full stop.
  - The type says what a commit does for people who use little or a
    plugin: `feat` adds a feature, `fix` fixes a bug, and `perf` makes
    something faster, each of which is released; `docs`, `refactor`,
    `test`, `style`, `build`, `ci`, and `chore` change nothing they use;
    and `revert` undoes commits, named in a `Refs:` footer.
  - The scope is the plugin a commit is about: `pydoc`, `rustdoc`,
    `jsdoc`, or `api_reference`. A commit about little, its built-in
    theme, or `wit/plugin.wit`, or about more than one component, has
    none.
  - A breaking change, of any type, has `!` before its `:`, and a
    `BREAKING-CHANGE:` footer that says what to change. The spec's
    `BREAKING CHANGE:`, with a space, means the same, but isn't a git
    trailer, so git, and GitHub, would miss the co-author trailer beside
    it.
  - CI checks each commit a pull request adds with committed, as
    `committed.toml` sets: its type, scope, mood, and full stop, a first
    line of up to 100 characters, and no merge, `WIP`, or `fixup!`
    commits, so a branch behind `main` is rebased on it, not merged with
    it. Pull requests are still merged with a merge commit, so each of
    their commits stays in `main`'s history.
- Commits from before this aren't rewritten; each component's first
  release starts its notes afresh.

## Consequences
- A component's next version, and its notes, can be worked out from the
  commits that changed its folders since its last release.
- A version says whether an update can break a site, a theme, or a
  plugin: until 1.0, one from 0.1.4 to 0.2.0 can, and one to 0.1.5 can't.
- Each commit takes a little more thought: its type, its scope, and
  whether it breaks anything.
- GitHub's merge commits, like "Merge pull request #64 from …", aren't
  Conventional Commits, and aren't checked.
- Limits, for now:
  - nothing checks that a description is lowercase, which committed
    can't, nor how long a body's lines are;
  - nothing checks that a commit without `!` doesn't break anything:
    that's the author's call;
  - `main` doesn't require the check to pass, so a pull request that
    fails it can still be merged.
