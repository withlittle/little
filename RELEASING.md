# Releasing

How little's plugins, `pydoc`, `rustdoc`, and `jsdoc`, are released. Each
is released on its own, from a tag that names it and its version, like
`pydoc-v0.2.0`. Pushing the tag to GitHub runs
[`release-plugin.yml`](./.github/workflows/release-plugin.yml), which checks
the tag, builds the plugin's `.wasm` file, and publishes it in a release of
little's repository, with notes that list what changed in the plugin since
its last release. Why it works so is in
[ADR 0052](./decisions/0052-releasing-plugins-from-tags.md), and how
versions are numbered, in
[ADR 0051](./decisions/0051-semantic-versioning-and-conventional-commits.md).
little's own releases aren't set up yet.

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
   which updates its `Cargo.lock` to the new version too. The release
   builds with `--locked`, so it fails if `Cargo.lock` isn't committed with
   `Cargo.toml`.
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

Then it writes the notes, which its log shows, builds the `.wasm` file, as
CI does, and publishes it, in a few minutes. Check the release on GitHub,
or with:

```sh
gh release view pydoc-v0.2.0
gh release verify pydoc-v0.2.0   # its attestation, once releases are immutable
```

Its notes say which version of `little:plugin` the plugin is built for, how
to name it in `little.yml`, and what's changed; they can still be edited,
on GitHub, though its file can't. A plugin's release isn't labelled
"Latest", which is left for little's own. If GitHub shows one so anyway,
before little has a release, nothing depends on it: the manual gives each
plugin's URL with its version.

## When a release fails

If the run fails, nothing's published: a check that fails stops it before
the build, and if the upload fails, gh deletes the release it was making.
Its log says what went wrong.

- If it failed for a reason outside the repository, like the network,
  re-run it, with **Re-run failed jobs** on its page.
- If the version has a release already, it's a draft, as a published
  release's tag can't be pushed again: one made by hand, or left by a run
  that was cancelled. Delete it, on the releases page, and re-run the run.
- Otherwise, fix what's wrong in a pull request, as in step 2. Then delete
  the tag, on GitHub and here, and tag `main` again, as in step 3:

  ```sh
  git push origin --delete pydoc-v0.2.0
  git tag -d pydoc-v0.2.0
  ```

A tag whose release wasn't published can be deleted, as nothing was
released from it; one whose release was published never is.

## The first releases

Each plugin's `Cargo.toml` says 0.1.0, which hasn't been released, so its
first release skips steps 1 and 2. Once the pull request that added this
file is merged, tag `main`, as in step 3, with `pydoc-v0.1.0`, and once its
release is published, `rustdoc-v0.1.0`, then `jsdoc-v0.1.0`. The manual's
URL for each leads nowhere until it's released. Each one's notes say it's
its first release, rather than list every change before it.
