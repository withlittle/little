# Releasing

How little, and its plugins, `pydoc`, `rustdoc`, and `jsdoc`, are released.
Each is released on its own, from a tag of its version: `v0.2.0` for
little, and one that names the plugin too, like `pydoc-v0.2.0`, for a
plugin. Pushing the tag to GitHub runs a workflow, which checks the tag,
builds what it releases, and publishes it in a release of little's
repository, with its [third-party notices](#third-party-notices), and notes
that list what changed since its last release:

- [`release.yml`](./.github/workflows/release.yml), for little: its binary
  for Linux, macOS, and Windows, on x64 and arm64, each in an archive, then
  its crate, `little-docs`, on crates.io, as
  [ADR 0055](./docs/decisions/0055-releasing-little.md) says.
- [`release-plugin.yml`](./.github/workflows/release-plugin.yml), for a
  plugin: its `.wasm` file, as
  [ADR 0052](./docs/decisions/0052-releasing-plugins-from-tags.md) says.

How versions are numbered is in
[ADR 0051](./docs/decisions/0051-semantic-versioning-and-conventional-commits.md).

A release can't be undone, so only the maintainer pushes a tag.

## Once, before the first release

Turn on immutable releases: in the repository's **Settings**, under
**Releases**, select **Enable release immutability**. From then on, a
published release's file can't be changed or deleted, nor its tag moved or
deleted, as ADR 0051 says a release never is, and GitHub attests to each
release, so anyone can check that a file came from it. Its title and notes
can still be edited. It applies only to releases published after it's
turned on.

## Releasing a plugin

### 1. See what's changed

[git-cliff](https://git-cliff.org) lists a plugin's changes since its last
release, as the release's notes will: the commits that changed its folder,
`plugins/<plugin>/`, or the library the plugins share,
`plugins/api_reference/`, of a type that's released, `feat`, `fix`, or
`perf`, with those that break something first. Install it, with
`cargo install --locked git-cliff` or `brew install git-cliff`, then, in the
repository's root:

```sh
plugin=pydoc
PLUGIN=$plugin git cliff --unreleased --tag-pattern "^$plugin-v" \
  --include-path "plugins/$plugin/**" --include-path "plugins/api_reference/**"
```

It warns of the commits it skips, like merges, which aren't Conventional
Commits; that's expected. With `--bumped-version` added, the same command
prints the tag to release next: after a breaking change, it raises the
minor version, from `pydoc-v0.1.4` to `pydoc-v0.2.0`, and after anything
else, the patch, to `pydoc-v0.1.5`. With nothing to list, there's nothing
to release, so it says so, and prints the last tag. Keep `--unreleased`:
without it, it raises the patch even then.

### 2. Raise its version, in a pull request

On a branch from `main`:

1. Raise `version` in `plugins/<plugin>/Cargo.toml`.
2. Run its tests, `cargo test --manifest-path plugins/<plugin>/Cargo.toml`,
   which updates its `Cargo.lock` to the new version too. CI builds with
   `--locked`, as the release does, so if `Cargo.lock` isn't committed with
   `Cargo.toml`, the pull request fails, before the release can.
3. Change the URLs in its page in the manual,
   `docs/plugins/python-api-reference.md`, `rust-api-reference.md`, or
   `javascript-api-reference.md`, to the new version, so the manual gives
   it. `git grep -n "download/$plugin-v" docs/` finds them.
4. Commit them as `chore(<plugin>): release <version>`, like
   `chore(pydoc): release 0.2.0`, open a pull request, and merge it once CI
   passes.

### 3. Tag it, and push the tag

Once the pull request is merged, check CI has passed on `main`, as the
release doesn't run the tests again, then tag `main`:

```sh
git switch main
git pull --ff-only
gh run list --workflow ci.yml --branch main --limit 1
git tag -a pydoc-v0.2.0 -m "pydoc 0.2.0"
git push origin pydoc-v0.2.0
```

`-a` makes an annotated tag, which records who tagged it, and when. Push
the tag by its name, not with `--tags`, which pushes every tag you have:
GitHub runs no workflow at all for a push of more than three tags. To
release more than one plugin, push each one's tag on its own.

### 4. Watch it run

The push starts **Release a plugin**, in the repository's **Actions** tab,
or `gh run watch`, which asks which run to watch. Before it builds
anything, it checks:

- that the tag names a plugin and a version, `X.Y.Z`; pre-releases, like
  `pydoc-v0.2.0-rc.1`, aren't supported;
- that the commit it's on is on `main`;
- that the plugin has no release of that version, not even a draft;
- that the plugin's `Cargo.toml` has that version.

Then it writes the notes, which its log shows, and the plugin's third-party
notices, builds the `.wasm` file, as CI does, and publishes them, in a few
minutes. Check the release on GitHub, or with:

```sh
gh release view pydoc-v0.2.0
gh release verify pydoc-v0.2.0   # its attestation, once releases are immutable
```

Its notes say which version of `little:plugin` the plugin is built for, how
to name it in `little.yml`, and what's changed; they can still be edited,
on GitHub, though its files can't. A plugin's release isn't labelled
"Latest", which is left for little's own. If GitHub shows one so anyway,
before little has a release, nothing depends on it: the manual gives each
plugin's URL with its version.

## Releasing little

little is released as a plugin is, from a tag of its version alone, like
`v0.2.0`, which runs [`release.yml`](./.github/workflows/release.yml).

### 1. See what's changed

git-cliff lists little's changes since its last release, as for a plugin,
from the commits that changed what's built into it: `src/`, `theme/`,
`wit/`, `Cargo.toml`, or `Cargo.lock`. Changes to the manual aren't
listed, nor to the plugins, which are released on their own. In the
repository's root:

```sh
git cliff --unreleased --tag-pattern '^v[0-9]' \
  --include-path 'src/**' --include-path 'theme/**' --include-path 'wit/**' \
  --include-path Cargo.toml --include-path Cargo.lock
```

With `--bumped-version` added, it prints the tag to release next, like
`v0.2.0`, as it does for a plugin.

### 2. Raise its version, in a pull request

On a branch from `main`:

1. Raise `version` in `Cargo.toml`.
2. Run `cargo test`, which updates `Cargo.lock` to the new version too.
3. Change the URL of little's release in the manual to the new version, in
   [Publishing](./docs/getting-started/publishing.md#publishing-on-github-pages),
   so its workflow downloads it. `git grep -n 'releases/download/v' docs/`
   finds it.
4. Commit them as `chore: release <version>`, like `chore: release 0.2.0`,
   open a pull request, and merge it once CI passes.

### 3. Tag it, and push the tag

Once CI has passed on `main`, as for a plugin, tag it:

```sh
git switch main
git pull --ff-only
gh run list --workflow ci.yml --branch main --limit 1
git tag -a v0.2.0 -m "little 0.2.0"
git push origin v0.2.0
```

### 4. Watch it run

The push starts **Release little**, which checks the tag as a plugin's is:
that it's a version, `vX.Y.Z`; that its commit is on `main`; that it has no
release; and that `Cargo.toml` has that version. Then it:

- builds little for each of its six targets, each on a GitHub runner of
  its own system, and checks each binary runs, by building a little site;
- writes little's third-party notices, and packs each binary in an archive
  with them, and `LICENSE`;
- publishes the archives, with a `SHA256SUMS` of them, as the repository's
  latest release, and attests where they were built;
- publishes `little-docs` to crates.io, from the tag, once the release is
  out, as cargo-binstall downloads its archives.

It takes some ten minutes. Check the release and the crate with:

```sh
gh release view v0.2.0
gh release verify v0.2.0
cargo info little-docs
```

A pull request that changes `release.yml`, `Cargo.toml`, or `Cargo.lock`
runs it too, but for the release and crates.io, so a change that would
break a release breaks there first.

## When a release fails

If the run fails before the release is published, nothing's published: a
check that fails stops it before the build, and if the upload fails, gh
deletes the release it was making. Its log says what went wrong.

- If it failed for a reason outside the repository, like the network,
  re-run it, with **Re-run failed jobs** on its page.
- If the version has a release already, it's a draft, as a published
  release's tag can't be pushed again: one made by hand, or left by a run
  that was cancelled. Delete it, on the releases page, and re-run the run.
- Otherwise, fix what's wrong in a pull request, as in step 2. Then delete
  the tag, on GitHub and here, and tag `main` again, as in step 3:

  ```sh
  git push origin --delete pydoc-v0.2.0   # or v0.2.0, for little
  git tag -d pydoc-v0.2.0
  ```

A tag whose release wasn't published can be deleted, as nothing was
released from it; one whose release was published never is.

little's crate is published after its release, so if only **publish to
crates.io** fails, the release is out, but the crate isn't. Re-run the
job: it checks whether crates.io has the version first, and publishes it
only if it hasn't. If it fails for a reason in the repository, fix it, and
release the next patch version, as the release can't be changed. Nor can a
version on crates.io: one that's wrong there can only be yanked, with
`cargo yank`, so new projects don't pick it.

## Third-party notices

A plugin's release carries a `THIRD-PARTY-NOTICES.md`, with the licences of
the crates it's built from, which
[cargo-about](https://github.com/EmbarkStudios/cargo-about) writes, as
[ADR 0054](./docs/decisions/0054-third-party-notices.md) says, and each of
little's archives carries its own. CI writes them all for each pull
request, as a release would, so one that changes a crate's licence fails
there. To write them yourself, install cargo-about 0.9.2, with
`cargo install --locked cargo-about@0.9.2`, then, in the repository's root:

```sh
scripts/notices.sh pydoc target/notices/pydoc.md   # or little, rustdoc, or jsdoc
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

## The first releases

little's `Cargo.toml`, and each plugin's, says 0.1.0, which hasn't been
released, so their first releases skip steps 1 and 2. Each one's notes say
it's its first release, rather than list every change before it. Once the
pull request that added little's release is merged:

1. Tag `main`, as in a plugin's step 3, with `pydoc-v0.1.0`, and once its
   release is published, `rustdoc-v0.1.0`, then `jsdoc-v0.1.0`. The
   manual's URL for each leads nowhere until it's released.
2. Tag `main` with `v0.1.0`, as in little's step 3. Its release is
   published, but not its crate: crates.io's trusted publishing can't
   publish a crate's first version, so **publish to crates.io** says so,
   and skips it.
3. Publish `little-docs` 0.1.0 yourself, from the tag, with an
   [API token](https://crates.io/settings/tokens) with the `publish-new`
   scope, which `cargo login` asks for:

   ```sh
   cargo login
   git switch --detach v0.1.0
   cargo publish --locked
   cargo logout
   ```

4. On crates.io, in `little-docs`'s **Settings**, under **Trusted
   Publishing**, add a publisher on GitHub: the repository
   `withlittle/little`, the workflow `release.yml`, and the environment
   `crates-io`. From then on, **publish to crates.io** publishes each
   release, with a token crates.io gives that run alone, so delete the API
   token.
5. Optionally, in the repository's **Settings**, under **Environments**,
   limit `crates-io`, which the first run made, to tags like `v*`, so only
   a release can ask crates.io for a token.
