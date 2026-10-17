# 0050 — One repository for little, its themes, and its plugins

- Status: accepted
- Date: 2026-10-17

## Context
little's repository holds more than little. Beside its code, in `src/`, are
its built-in theme, in `theme/`, which is compiled into its binary; the
interface plugins export, `wit/plugin.wit`, and the plugin little's tests
build, in `tests/plugin/`; the `pydoc`, `rustdoc`, and `jsdoc` plugins, and
the library they share, in `plugins/`, each a Cargo workspace of its own;
and the manual, in `docs/`, which documents them all, and documents little's
own code with `rustdoc`. As little gains themes and plugins, each could live
in a repository of its own, or each kind in one.

little's first three weeks, 101 commits, merges aside, show how they change:
- The built-in theme changes with little: 27 of the 32 commits that changed
  `theme/` changed `src/` too, mostly `site.rs` and `config.rs`, as a
  feature gives templates new data, and the theme, which owns the markup,
  shows it.
- The plugin interface is young: `little:plugin` went from 0.1.0 to 0.2.0
  eight days after it was written ([0046](./0046-plugin-hooks.md)), in one
  commit to `src/`, `wit/`, `tests/plugin/`, both plugins, and the manual,
  and a plugin built for 0.1.0 doesn't load. Each plugin reads
  `wit/plugin.wit` by its path in the repository.
- Otherwise, the plugins change on their own: 5 of the 9 commits that
  changed `plugins/` didn't change `src/`. But each of the 9 changed the
  manual, an ADR, or `AGENTS.md`.
- little has one maintainer, and no releases. A theme in a repository of
  its own, theme-bootstrap, was made, and has been deleted.

A site names a theme or a plugin by its folder, its file, or its URL
([0004](./0004-downloading-themes.md),
[0029](./0029-plugins-that-render-code-blocks.md)), so which repository it's
built in is hidden from the site, but for its URL.

## Options considered
- **One repository,** as now: a change to what little gives themes or
  plugins is one pull request, with the theme and plugins it changes, tested
  together, and documented in the same manual. But CI checks every
  workspace on every change, and releasing one component, not all, needs
  tags that say which.
- **A repository for each kind:** one for little, one for themes, and one
  for plugins. A change to the plugin interface, or to templates' data,
  spans three, released in order, and the plugins are tested against a
  release of little, not the change. A theme there still needs an archive
  made for it, as GitHub's archive of a tag holds the whole repository, and
  a theme's archive must hold one folder. It pays the costs of many
  repositories without most of what they give.
- **A repository for each component:** little, and each theme and plugin.
  Each is released on its own, a theme's URL can be GitHub's archive of its
  tag, as it is, and each is an example for authors to copy. But a change
  to the plugin interface spans every one; the library the API reference
  plugins share needs a repository of its own, and to be published on
  crates.io; the built-in theme can't leave little, as its binary holds it;
  and each repository's CI, dependencies, and issues add to one
  maintainer's work.
- **Others:** git submodules, which join separate repositories, but with
  the costs of both; or one repository, split into a read-only one for each
  component, as Symfony publishes its components, which takes tooling
  little doesn't need yet.

## Decision
- little, its built-in theme, `wit/plugin.wit`, `tests/plugin/`, its
  plugins and the library they share, the manual, and the ADRs stay in one
  repository, `withlittle/little`, as they are. little stays at its root,
  not in a `core/` folder, as the repository is little's.
- Each plugin stays a Cargo workspace of its own, with its own dependencies
  and lock file, as if it were in a repository of its own.
- Each component is released on its own, not the repository as a whole: it
  keeps its own version, in its `Cargo.toml`, and is released from a tag
  that names it, so a fix to `pydoc` needn't wait for little. How the tags
  are named, and what each release publishes, is decided as releases are
  set up.
- A new theme or plugin of little's own goes in this repository too. One
  moves to a repository of its own once it has maintainers of its own, or
  its CI holds the rest up, and not while what it's built on, the plugin
  interface or templates' data, still breaks. A site names it by URL either
  way, and changes the URL to change its version, so a move breaks no site:
  its releases so far stay where they are.
- Other people's themes and plugins are in their own repositories, as now.

## Consequences
- A change to what little gives themes or plugins is one pull request, with
  the built-in theme and the plugins it changes, tested together, and
  documented in the manual.
- Releases take more care than in a repository of one component: each
  component has its own tags, release steps, and notes, which list what
  changed in its folders since its last tag, not everything since the
  repository's last tag, as GitHub's own notes would.
- A theme of little's own, besides the built-in one, can't be named by
  GitHub's archive of a tag, so its release makes an archive of its folder.
- Limits, for now:
  - CI checks every workspace on every change, which takes longer with each
    plugin, so it'll check only those a change touches once that's too
    slow;
  - authors of other plugins copy `wit/plugin.wit`, until it's published
    with little's releases.
