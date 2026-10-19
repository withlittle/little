---
title: Install
description: Download little for your system, or install it with Cargo, and check it works.
---

# Install

little is a single binary, with no runtime, no `node_modules`, and no version
managers. Download it for your system, or install it with Cargo.

## Download it

Each of little's [releases](https://github.com/withlittle/little/releases)
has it for Linux, macOS, and Windows, on x64 and arm64, each in an archive
with its licence and third-party notices. These are the latest:

| System | Archive |
| --- | --- |
| Linux on x64 | [`little-x86_64-unknown-linux-musl.tar.gz`](https://github.com/withlittle/little/releases/latest/download/little-x86_64-unknown-linux-musl.tar.gz) |
| Linux on arm64 | [`little-aarch64-unknown-linux-musl.tar.gz`](https://github.com/withlittle/little/releases/latest/download/little-aarch64-unknown-linux-musl.tar.gz) |
| macOS on Apple silicon | [`little-aarch64-apple-darwin.tar.gz`](https://github.com/withlittle/little/releases/latest/download/little-aarch64-apple-darwin.tar.gz) |
| macOS on Intel | [`little-x86_64-apple-darwin.tar.gz`](https://github.com/withlittle/little/releases/latest/download/little-x86_64-apple-darwin.tar.gz) |
| Windows on x64 | [`little-x86_64-pc-windows-msvc.zip`](https://github.com/withlittle/little/releases/latest/download/little-x86_64-pc-windows-msvc.zip) |
| Windows on arm64 | [`little-aarch64-pc-windows-msvc.zip`](https://github.com/withlittle/little/releases/latest/download/little-aarch64-pc-windows-msvc.zip) |

Linux's are static, so they run on any distribution, with glibc or musl.
Unpack the archive, and put `little`, or `little.exe` on Windows, in a
folder on your `PATH`. On a Mac with Apple silicon, this downloads the
latest, and puts `little` in `/usr/local/bin`:

```sh
curl -fsSL https://github.com/withlittle/little/releases/latest/download/little-aarch64-apple-darwin.tar.gz \
  | sudo tar -xz -C /usr/local/bin little
```

On Linux, or a Mac with Intel, use the archive for your system instead; to
put `little` in another folder, like `~/.local/bin`, use its path, without
`sudo`.

### If your system stops it

little isn't signed yet, so macOS and Windows may stop it, if it's
downloaded with a browser:

- macOS says Apple can't check it for malware. Choose **Done**, then, in
  **System Settings**, under **Privacy & Security**, **Open Anyway**, and
  run it again. Or, before you run it, take off the quarantine the browser
  put on it, with `xattr -d com.apple.quarantine little`. `curl`, as above,
  doesn't quarantine it.
- Windows says it protected your PC. Choose **More info**, then **Run
  anyway**. Or, before you unpack the archive, unblock it, in PowerShell,
  with `Unblock-File little-x86_64-pc-windows-msvc.zip`.

### Checking a download

Each release has a `SHA256SUMS` file, with each archive's SHA-256 checksum.
Download it beside the archive, and check the archive with
`sha256sum --check --ignore-missing SHA256SUMS` on Linux, or
`shasum -a 256 --check --ignore-missing SHA256SUMS` on macOS. On Windows,
`Get-FileHash`, in PowerShell, gives an archive's checksum, to compare with
its line in `SHA256SUMS`.

With the [GitHub CLI](https://cli.github.com), you can check where an
archive came from too: that little's release workflow built it, in little's
repository, and that it's the file the latest release published, unchanged:

```sh
gh attestation verify little-aarch64-apple-darwin.tar.gz --repo withlittle/little
gh release verify-asset little-aarch64-apple-darwin.tar.gz --repo withlittle/little
```

For an older release's archive, give `gh release verify-asset` its tag,
like `v0.1.0`, before the archive's name.

## Install it with Cargo

little is on crates.io as [`little-docs`](https://crates.io/crates/little-docs),
as `little` is another crate's name there, but the command is still
`little`. [cargo-binstall](https://github.com/cargo-bins/cargo-binstall)
installs little's release for your system, as above, into `~/.cargo/bin`:

```sh
cargo binstall little-docs
```

Or Cargo builds it, with [Rust](https://rustup.rs) 1.96 or later, which
takes a few minutes:

```sh
cargo install --locked little-docs
```

## Build it from source

Cargo builds little's `main` branch, as it is now, with:

```sh
cargo install --locked --git https://github.com/withlittle/little little-docs
```

Or clone little's repository and install it from there, which gives you this
manual to read offline, with `little serve` in the clone:

```sh
git clone https://github.com/withlittle/little.git
cd little
cargo install --locked --path .
```

In a clone, `cargo build --release` builds the same binary without
installing it, at `target/release/little`, which you can copy anywhere.

## Check it works

```sh
little --version
```

prints `little`, and its version, like `little 0.1.0`. `little --help` lists
every [command](../reference/commands.md).

## Update it

Download the latest release's archive again, or run the same
`cargo binstall` or `cargo install` again. Each release's notes, on
[GitHub](https://github.com/withlittle/little/releases), list what's changed
since the one before.

little's releases follow [Semantic Versioning](https://semver.org/). Until
1.0, a release that raises the minor version, like 0.2.0 after 0.1.4, adds
features, and can break a site, a theme, or a plugin, which its notes list
first. One that raises the patch version, like 0.1.5, only fixes bugs, or
makes something faster, so it's safe to update to. little's plugins are
numbered the same way, each on its own.

Next, start [your first site](first-site.md).
