# 0054 — Third-party notices

- Status: accepted
- Date: 2026-10-18
- Amends: [0052](./0052-releasing-plugins-from-tags.md)

## Context
little is under the MIT licence, but its binary is built from some 570
crates others wrote, and each plugin's `.wasm` file from up to some 100.
Nearly all of them are under licences, like MIT, Apache-2.0, BSD, ISC, and
Zlib, that let anyone share a program built from them, as long as each copy
carries the crate's copyright notice and licence. A release that carries
only little's own licence doesn't.

A crate's `license`, in its `Cargo.toml`, doesn't always say it all. Some
bundle C code under a licence of its own, which `license` leaves out: onig_sys
bundles Oniguruma, and tikv-jemalloc-sys, which little's Linux builds use,
jemalloc. Some crates' licence files hold more than one licence, or the
notices of code they're built on, after their own, which cargo-about can't
match to a licence, so it would give a standard text in their place:
comrak's holds cmark-gfm's; curve25519-dalek's, Go's; libm's, musl's;
aws-lc-sys's, those of AWS-LC, BoringSSL, and OpenSSL, and the text of each
of its licences; and aws-lc-rs's, both of its licences, with Amazon's
copyright. Others ship no licence file at all, like the crates of Ruff's
parser, which `pydoc` is built on, and oxc's, which `jsdoc` is.

little also highlights code with the syntax definitions that bat collects,
which two-face compiles in, and 66 of them are under licences that ask to be
acknowledged.

Five of little's crates, like cssparser and selectors, are under MPL-2.0,
which asks that their source be available too. It is, on crates.io, at the
version the notices give.

Neither little nor its plugins have been released, and a plugin's release
publishes only its `.wasm` file
([0052](./0052-releasing-plugins-from-tags.md)).

## Options considered
- **Tools:**
  - [cargo-about](https://github.com/EmbarkStudios/cargo-about), which reads
    each crate's licence files, matches each to SPDX's licences, and writes
    them through a Handlebars template. It fails on a licence it isn't told
    to accept, and a crate it can't read rightly can be clarified in its
    `about.toml`, with the files it should use, each pinned by a checksum.
  - cargo-deny, which reads the same clarifications, but only checks
    licences, and writes no notices.
  - cargo-license, which lists each crate's licence, but not its text.
  - Notices written by hand, which go out of date with each dependency.
- **Where they go:** a `THIRD-PARTY-NOTICES.md` in each release, beside the
  binary or `.wasm` file; or a `little licenses` command, as some programs
  have, which is code in little's core for what a release can carry, and
  which a plugin can't have.
- **Committed or written in each release:** committed, so a pull request
  that changes a dependency changes them too, some 700 KB of them; or
  written as each release is made, and checked in CI.
- **A crate cargo-about can't read rightly:** clarified, by hand; or given
  SPDX's standard text, which has placeholders for its copyright.

## Decision
- cargo-about 0.9.2 writes the notices of little, and of each plugin, through
  `about.hbs`, at the repository's root, with the `about.toml` beside its
  `Cargo.toml`. `scripts/notices.sh` runs it, as CI and the releases do.
- Each `about.toml` accepts only the licences its crates use, in order of
  preference, so a crate with a choice, like `MIT OR Apache-2.0`, is listed
  under the first. A crate under any other licence fails the notices, so it's
  read before it's accepted. They cover each target the component is built
  for, little's six, or `wasm32-wasip2`, and leave out build and dev
  dependencies, and the component's own crates.
- little's `about.toml` clarifies seven crates, each with the files it
  should use, pinned by their checksums: onig_sys and tikv-jemalloc-sys, with
  the licence of the code they bundle, and comrak, curve25519-dalek, libm,
  aws-lc-sys, and aws-lc-rs, with their whole licence files. chrono's is
  read with cargo-about's own workaround.
- Any other crate is given its own licence file, where cargo-about can match
  it, and SPDX's standard text otherwise, with a link to the crate's
  repository. Some 50 of little's crates are given SPDX's, like Pagefind's
  and merman's, as are all the plugins' Ruff and oxc crates.
- cargo-about only warns of a file whose checksum has changed, or of a
  clarification no crate needs, so the script fails on any warning too.
- little's notices end with those of its syntax definitions: those whose
  licences ask to be acknowledged, under their licence, with its text, from
  two-face's own list, which `examples/syntax_notices.rs` prints, so it isn't
  compiled into little.
- The notices are written as each release is made, not committed. CI writes
  them all for each pull request, so one that changes a crate's licence
  fails there, before a release can.
- A plugin's release publishes its `THIRD-PARTY-NOTICES.md` beside its
  `.wasm` file, which amends 0052, where it published nothing else. little's
  releases will carry its own, once they're set up.

## Consequences
- A pull request that adds a crate under a licence an `about.toml` doesn't
  accept, or changes a clarified crate's licence file, fails CI until the
  licence is read, and accepted, or the checksum is updated, as
  [`RELEASING.md`](https://github.com/withlittle/little/blob/main/RELEASING.md)
  says.
- A clarification names a crate, not a version, so it holds for a new
  version whose files have the same checksums.
- A crate that bundles code under another licence, and doesn't say so,
  isn't caught: only reading it, as it's added, would catch it.
- Where a crate ships no licence file, its notice is SPDX's standard text,
  and its copyright is in its repository, which the notices link to.
  aws-lc-sys's licence file is given once, under ISC, so its Apache-2.0,
  BSD-3-Clause, and MIT texts are SPDX's too.
- little's notices are some 700 KB, and a plugin's up to some 75 KB.
