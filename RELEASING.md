# Releasing

little and each of its plugins are released on their own, by pushing a tag
of the version: `v0.4.0` for little, or `pydoc-v0.2.0` for a plugin. The
tag's workflow builds the release, and publishes it on GitHub, with notes
that list what's changed. A release can't be undone, so only the
maintainer pushes a tag.

A release is four steps: see what's changed, raise the version in a pull
request, tag `main`, and watch the run. They use
[`gh`](https://cli.github.com) and [git-cliff](https://git-cliff.org), and
settings that are made once, as
[Before your first release](#before-your-first-release) says.

The records say why little is released this way:
[0051](./decisions/0051-semantic-versioning-and-conventional-commits.md)
and [0058](./decisions/0058-features-raise-the-minor-version.md), for how
versions are numbered, [0055](./decisions/0055-releasing-little.md), for
little's releases, and
[0052](./decisions/0052-releasing-plugins-from-tags.md), for plugins'.

## Releasing little

### 1. See what's changed

```sh
git cliff --unreleased --tag-pattern '^v[0-9]' \
  --include-path 'src/**' --include-path 'theme/**' --include-path 'wit/**' \
  --include-path Cargo.toml --include-path Cargo.lock
```

It lists what the release's notes will: each `feat`, `fix`, and `perf`
commit since the last release that changed what's built into little, with
those that break something first. Changes to the manual and the plugins
aren't listed, and it warns of the commits it skips, like merges, which is
expected.

Add `--bumped-version` to print the tag to release next, like `v0.4.0`: a
feature or a breaking change raises the minor version, and anything else
the patch. With nothing to release, it prints the last tag. Keep
`--unreleased`, or it raises the patch even then.

### 2. Raise the version, in a pull request

On a branch from `main`:

1. Raise `version` in `Cargo.toml`.
2. Run `cargo test`, which writes the new version into `Cargo.lock` too.
3. Change the URL of little's release in the manual's
   [Publishing](./docs/getting-started/publishing.md#publishing-on-github-pages)
   page to the new version. `git grep -n 'releases/download/v' docs/`
   finds it.
4. Commit them as `chore: release 0.4.0`, open a pull request, and merge it
   once its checks pass.

The pull request also builds little as a release does, for each system,
without releasing it, so what would break the release fails here first.

### 3. Tag `main`, and push the tag

The release doesn't run the tests again, so wait for CI to pass on `main`
after the merge, then tag it:

```sh
git switch main
git pull --ff-only
gh run list --workflow ci.yml --branch main --limit 1   # completed, success
git tag -a v0.4.0 -m "little 0.4.0"
git push origin v0.4.0
```

`-a` makes an annotated tag, which records who tagged it, and when. The
push starts the release.

### 4. Watch it run

```sh
gh run watch   # choose "Release little"
```

In some 20 minutes, [`release.yml`](./.github/workflows/release.yml)
checks the tag against `main` and `Cargo.toml`, builds little for Linux,
macOS, and Windows, on x64 and arm64, and publishes each in an archive, as
the repository's latest release. Then it publishes little's crate,
`little-docs`, on crates.io. Check both with:

```sh
gh release view v0.4.0
gh release verify v0.4.0   # its attestation
cargo info little-docs
```

## Releasing a plugin

A plugin is released in the same four steps, from a tag that names it too,
like `pydoc-v0.2.0`.

### 1. See what's changed

```sh
plugin=pydoc
PLUGIN=$plugin git cliff --unreleased --tag-pattern "^$plugin-v" \
  --include-path "plugins/$plugin/**" --include-path "plugins/api_reference/**"
```

The second `--include-path` is the library the plugin shares with others
like it:

| Plugin | Its library |
| ------ | ----------- |
| `pydoc`, `rustdoc`, `jsdoc`, `godoc`, `openapi` | `plugins/api_reference/**` |
| `asciidoc`, `orgmode`, `restructuredtext` | `plugins/markup/**` |
| `jupyter` | none: leave the second `--include-path` out |

It lists what the release's notes will: each `feat`, `fix`, and `perf`
commit since the plugin's last release that changed either folder, with
those that break something first. It warns of the commits it skips, like
merges, which is expected.

Add `--bumped-version` to print the tag to release next, like
`pydoc-v0.2.0`: a feature or a breaking change raises the minor version,
and anything else the patch. With nothing to release, it prints the last
tag. Keep `--unreleased`, or it raises the patch even then.

### 2. Raise the version, in a pull request

On a branch from `main`:

1. Raise `version` in `plugins/pydoc/Cargo.toml`.
2. Run `cargo test --manifest-path plugins/pydoc/Cargo.toml`, which writes
   the new version into the plugin's `Cargo.lock` too.
3. Change the URLs in the plugin's page in the manual to the new version.
   `git grep -n 'download/pydoc-v' docs/` finds them.
4. Commit them as `chore(pydoc): release 0.2.0`, open a pull request, and
   merge it once its checks pass.

### 3. Tag `main`, and push the tag

The release doesn't run the tests again, so wait for CI to pass on `main`
after the merge, then tag it:

```sh
git switch main
git pull --ff-only
gh run list --workflow ci.yml --branch main --limit 1   # completed, success
git tag -a pydoc-v0.2.0 -m "pydoc 0.2.0"
git push origin pydoc-v0.2.0
```

Push each tag by its name, on its own, not with `--tags`, which pushes
every tag you have: GitHub runs no workflow at all for a push of more than
three tags.

### 4. Watch it run

```sh
gh run watch   # choose "Release a plugin"
```

In a few minutes,
[`release-plugin.yml`](./.github/workflows/release-plugin.yml) checks the
tag against `main` and the plugin's `Cargo.toml`, builds its `.wasm` file,
and publishes it, with its third-party notices. Check the release with:

```sh
gh release view pydoc-v0.2.0
gh release verify pydoc-v0.2.0   # its attestation
```

Its notes say which version of `little:plugin` the plugin is built for,
and how to name it in `little.yml`. A plugin's release isn't labelled
"Latest", which is left for little's own.

### A new plugin's first release

Release a new plugin as soon as the pull request that adds it is merged:
tag `main`, as in step 3, with the version its `Cargo.toml` has, `0.1.0`.
There's nothing to do in steps 1 and 2. Its page in the manual, which is
published from `main`, names that release already, so its URL leads
nowhere until the release is out.

## When a release fails

A run publishes nothing until its last steps: a check that fails stops it
before it builds, and if an upload fails, gh deletes the release it was
making. The run's log says what went wrong.

- **Something outside the repository, like the network:** re-run it, with
  **Re-run failed jobs** on the run's page.
- **"… is released already":** the version has a draft release, made by
  hand, or left by a run that was cancelled. Delete the draft, on the
  releases page, and re-run the run.
- **Anything else:** fix it in a pull request, as in step 2. Then delete
  the tag, on GitHub and here, and tag `main` again, as in step 3:

  ```sh
  git push origin --delete v0.4.0   # or pydoc-v0.2.0, for a plugin
  git tag -d v0.4.0
  ```

  Only a tag whose release wasn't published can be deleted, as nothing
  was released from it.
- **Only "publish to crates.io":** little's release is out, but its crate
  isn't. Re-run the job, which publishes the crate only if crates.io
  doesn't have the version yet. If it fails for a reason in the
  repository, fix it, and release the next patch version: a release can't
  be changed, and a version on crates.io can only be yanked, with
  `cargo yank`, so new projects don't pick it.

## Before your first release

On the computer you release from, install the tools the steps use, and
sign in to GitHub:

```sh
brew install gh git-cliff
gh auth login
```

Choose HTTPS, and **Yes** to "Authenticate Git with your GitHub
credentials?", so `git push` can push a tag: GitHub Desktop's sign-in
doesn't reach `git` in a terminal. `cargo install --locked git-cliff`
installs git-cliff too.

Releases also rely on three settings outside the repository's files, made
once, by hand:

- **Release immutability**, in the repository's **Settings**, under
  **Releases**: a published release's files can't be changed or deleted,
  nor its tag moved or deleted, and GitHub attests to each release, so
  anyone can check that a file came from it. A release's title and notes
  can still be edited.
- **A trusted publisher**, in `little-docs`'s **Settings** on crates.io,
  under **Trusted Publishing**: the repository `withlittle/little`, the
  workflow `release.yml`, and the environment `crates-io`. It gives
  **publish to crates.io** a token for its run alone, so there's no API
  token to keep.
- **The `crates-io` environment**, in the repository's **Settings**, under
  **Environments**, which only tags like `v*` can use, so only a release
  can ask crates.io for a token.

## Third-party notices

A plugin's release carries a `THIRD-PARTY-NOTICES.md`, with the licences of
the crates it's built from, which
[cargo-about](https://github.com/EmbarkStudios/cargo-about) writes, as
[ADR 0054](./decisions/0054-third-party-notices.md) says, and each of
little's archives carries its own. CI writes them all for each pull
request, as a release would, so one that changes a crate's licence fails
there. To write them yourself, install cargo-about 0.9.2, with
`cargo install --locked cargo-about@0.9.2`, then, in the repository's root:

```sh
scripts/notices.sh pydoc target/notices/pydoc.md   # or little, rustdoc, jsdoc, godoc, openapi, jupyter, asciidoc, orgmode, or restructuredtext
```

Each one's `about.toml`, beside its `Cargo.toml`, accepts the licences its
crates use, and clarifies the crates cargo-about can't read rightly, with
the licence files it should use. When it fails, cargo-about says why:

- `failed to satisfy license requirements`, for a crate under a licence the
  `about.toml` doesn't accept. Read the licence: if it lets a release carry
  the crate with only its notice, as MIT does, add it to `accepted`;
  otherwise, don't use the crate.
- `failed to validate all files specified in clarification`, for a new
  version of a clarified crate, whose licence file has changed. Read the new
  file, in `~/.cargo/registry/src/*/<crate>-<version>/`, check that the
  clarification still gives the crate's licences, and put the file's
  checksum, from `shasum -a 256 <file>`, in its place.
- `config found for a crate not present in the graph`, for a clarified crate
  that's no longer used. Delete its clarification.

Nothing catches a crate that bundles code under a licence its `license`
leaves out, like a `-sys` crate's C library, or whose licence file has
others' notices after its own, so read a new crate's licence files as it's
added, and clarify it as the others are.
