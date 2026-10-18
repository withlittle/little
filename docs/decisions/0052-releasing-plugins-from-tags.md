# 0052 — Releasing plugins from tags

- Status: accepted
- Date: 2026-10-17

## Context
`pydoc`, `rustdoc`, and `jsdoc` are each released on their own, from a tag
that names them, with notes that list what changed in their folders since
their last release, not everything in the repository
([0050](./0050-one-repository-for-little-its-themes-and-plugins.md)). Each
has its own version, in its `Cargo.toml`, raised as it's released, by
Cargo's convention until 1.0, and a version, once released, is never
changed, nor its tag moved
([0051](./0051-semantic-versioning-and-conventional-commits.md)). How the
tags are named, and what a release publishes, was left until releases were
set up.

None has been released, so a site that uses one builds its `.wasm` file
itself, in a clone of little's repository, with Rust's `wasm32-wasip2`
target, and names it by its path. A site can name a plugin by an HTTPS URL
instead, which little downloads in the site's first build, and never
downloads again ([0029](./0029-plugins-that-render-code-blocks.md)), so a
URL that always led to a plugin's latest version would never update: a
site needs a URL for each version.

The plugins' releases share the repository with little's own, once it has
some. GitHub labels one of a repository's releases "Latest", and writes a
release's notes, if asked, from every pull request merged since the release
before it, whatever it changed.

## Options considered
- **Tools:**
  - release-please, Google's, which keeps a pull request open for each
    component, raising its version and adding to its `CHANGELOG.md` from
    Conventional Commits, and tags and releases it once that's merged. It
    handles monorepos, but runs on Node, in an action, with a config and a
    manifest of versions of its own, which is more than three plugins,
    released by one maintainer, need.
  - release-plz, which does much the same for Rust's crates, with
    git-cliff, but is built to publish them to crates.io, which the plugins
    aren't, from one Cargo workspace, where each plugin is a workspace of
    its own.
  - cocogitto, a Rust binary, which raises the versions of a monorepo's
    packages, tags them, and writes their changelogs, from Conventional
    Commits, with hooks for each step, and checks commits, as committed
    already does: another tool to learn, for what a workflow does.
  - A workflow of little's own, started by a tag pushed by hand, which
    checks the tag, builds the plugin, and publishes it, with notes from
    [git-cliff](https://git-cliff.org), a Rust binary that writes
    Conventional Commits into notes, from a template, for the tags and
    folders it's given, and releases nothing. Versions are raised by hand,
    which git-cliff can suggest.
- **Tags:** `pydoc-v0.1.0`, as release-please and release-plz name a
  component's; `pydoc@0.1.0`, as npm's monorepos do, which puts an `@` in
  each URL to the file; or `pydoc/v0.1.0`, as Go's modules in a folder do,
  which reads as a folder in it.
- **Notes:** GitHub's own, which list every pull request in the repository
  since the release before, whichever component it changed; or git-cliff's,
  from the commits that changed the plugin's folders since its last tag.

## Decision
- A plugin is released from a tag that names it and its version,
  `<plugin>-v<version>`, like `pydoc-v0.1.0`, on a commit on `main`, pushed
  by hand. Pushing it runs `.github/workflows/release-plugin.yml`, which
  starts only for `pydoc-v*`, `rustdoc-v*`, and `jsdoc-v*`. little's own
  tags, once it's released, mustn't start with a plugin's name, and a
  pattern of its own, like `v*`, won't match a plugin's, as GitHub matches
  a pattern against the whole tag.
- Before building anything, the workflow checks that the tag is a plugin's
  name and a version, `X.Y.Z`, with no pre-release; that its commit is on
  `main`; that the version has no release, not even a draft; and that it's
  the version in the plugin's `Cargo.toml`. It builds the plugin as CI
  does, with `--locked`, so a `Cargo.lock` that wasn't changed with the
  version stops it too.
- A release, titled like `pydoc 0.1.0`, publishes the plugin's `.wasm`
  file, like `little_pydoc.wasm`, and nothing else. gh uploads the file to
  a draft, and publishes it once it's there, so a run that fails publishes
  nothing, and its tag can be deleted and pushed again. Once a release is
  published, its tag never is.
- A plugin's release isn't labelled the repository's latest, which is left
  for little's own.
- Its notes:
  - say which version of `little:plugin` it's built for, from
    `wit/plugin.wit` at the tag, and how to name it in `little.yml`, by its
    URL, with a link to its page in the manual;
  - list what's changed since its last tag, from git-cliff, with
    `cliff.toml`, which the plugins share: what breaks first, then
    features, fixes, and performance, from commits of type `feat`, `fix`,
    or `perf`, or any with `!`, that changed the plugin's folder or
    `plugins/api_reference/`, which every plugin is built with, so its
    commits are listed in each, under its scope;
  - don't list commits to `wit/plugin.wit`, which are little's: a plugin
    built for a new version names it in its own `src/lib.rs`, in its
    folder;
  - say a first release is the first, as 0051 says.
- A version is raised by hand, in a pull request,
  `chore(<plugin>): release <version>`, with its `Cargo.lock` and the
  manual's URLs for it, before it's tagged; each plugin's first, 0.1.0,
  which its `Cargo.toml` says already, needs none. git-cliff, with
  `cliff.toml`, suggests the next, numbered as 0051 sets.
- The manual gives each plugin's URL with the version last released, like
  `https://github.com/withlittle/little/releases/download/pydoc-v0.1.0/little_pydoc.wasm`,
  so a site updates it by changing the URL, as for any plugin.
- The repository's releases are immutable, a setting turned on once, so
  GitHub won't let a published release's file change, nor its tag move,
  and attests to each.
- [`RELEASING.md`](https://github.com/withlittle/little/blob/main/RELEASING.md)
  lists the steps.

## Consequences
- Releasing a plugin takes a pull request, then a tag, pushed by hand, and
  a run of a few minutes. Nothing's released but by a tag the maintainer
  pushes, and nothing's committed but by a pull request.
- A mistake in a tag, like a version that doesn't match `Cargo.toml`, fails
  the run before anything's built.
- A commit's type and scope decide whether its release's notes list it, and
  how: a change people see needs `feat`, `fix`, or `perf`, and one that
  breaks something, `!`.
- A site that names a plugin by its URL keeps that version until it changes
  the URL.
- Limits, for now:
  - the workflow doesn't run the tests, which CI ran on `main`, so a tag is
    pushed once CI has passed there;
  - pre-releases, like `pydoc-v0.2.0-rc.1`, aren't supported;
  - GitHub runs no workflow for a push of more than three tags, so they're
    pushed one at a time;
  - nothing checks that a release's pull request changes the manual's URLs;
  - until little has a release, GitHub may still show a plugin's as the
    latest;
  - the manual's URLs lead nowhere until each plugin's first release, after
    this is merged; links into the repository aren't checked
    ([0020](./0020-links-to-a-pages-file-in-its-repo.md)), so
    `little build --strict` doesn't fail on them.
