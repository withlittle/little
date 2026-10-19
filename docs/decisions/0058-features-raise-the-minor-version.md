# 0058 — Features raise the minor version

- Status: accepted
- Date: 2026-10-19
- Amends: [0051](./0051-semantic-versioning-and-conventional-commits.md)

## Context
Until 1.0, little, `pydoc`, `rustdoc`, and `jsdoc` followed Cargo's
convention: a breaking change raised the minor version, and anything
else, a feature or a fix, the patch
([0051](./0051-semantic-versioning-and-conventional-commits.md)). So
`little init`, little's first feature since 0.1.0
([0057](./0057-starting-a-site-with-little-init.md)), would have made it
0.1.1.

But [SemVer](https://semver.org/)'s summary, which is what most people
know of it, says the patch version is for bug fixes, and the minor for
features. Read that way, 0.1.1 looks like a fix, and hides a new command.
SemVer itself sets no rules until 1.0.0, so either is SemVer.

`little:plugin`'s version isn't only for people to read: little loads a
plugin only if wasmtime reads the version it was built for as compatible
with little's, and wasmtime reads 0.2.0 and 0.2.1 as compatible, and 0.2.0
and 0.3.0 as not, as Cargo reads crates' versions.

## Options considered
- **Cargo's convention**, as 0051 chose: a version says whether an update
  can break anything, but a feature looks like a fix.
- **SemVer's FAQ**, which suggests raising the minor version for each
  release until 1.0: simple, but a release of only fixes looks like one
  with features, and the patch version goes unused.
- **SemVer's summary, before 1.0 too:** a feature raises the minor
  version, and a fix the patch, as they will from 1.0.0. A breaking change
  has no major version to raise until then, so it raises the minor too.
- **1.0.0 now**, so SemVer's rules apply in full. But little still breaks
  often, and SemVer's own advice, for an API that changes every day, is to
  stay at 0.y.z, as 0051 found.

## Decision
- Until 1.0.0, little, `pydoc`, `rustdoc`, and `jsdoc` raise:
  - the minor version, from 0.1.4 to 0.2.0, for a feature, or for a
    breaking change, which has no major version to raise until then;
  - the patch version, from 0.1.4 to 0.1.5, for anything else: a fix, or
    something faster.
- `little:plugin` keeps 0051's rule, as wasmtime reads its version: a new
  hook, or a new function for plugins to call, makes it 0.2.1, so plugins
  built for 0.2.0 still load, and any other change makes it 0.3.0.
- From 1.0.0, SemVer's own rules apply, as 0051 says, and the rest of 0051
  stands: what's public, what breaks, and how commits are written.
- git-cliff's `--bumped-version` follows it, with
  `features_always_bump_minor` in
  [`cliff.toml`](https://github.com/withlittle/little/blob/main/cliff.toml).
- `little init` makes little 0.2.0.

## Consequences
- A version's numbers mean what SemVer's summary says they do, but for a
  breaking change, until 1.0: a feature never hides in a patch.
- A patch release, like 0.2.1, only fixes things, so it's always safe to
  update to. A minor one, like 0.3.0, adds features, and can break a site,
  a theme, or a plugin, which its notes list first: the version alone no
  longer says whether an update breaks anything.
- Each release with a feature raises the minor version, so it climbs
  faster before 1.0.
- Cargo reads 0.1.0 and 0.2.0 as incompatible, but nothing depends on
  little or its plugins through Cargo: little isn't a library, and a site
  names a plugin's `.wasm` file by its URL.
- `little:plugin` is numbered by a rule of its own, as wasmtime needs.
