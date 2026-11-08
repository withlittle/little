---
title: Install
description: Install little with one command, and check it works.
---

# Install

```sh
cargo install --locked little-docs
```

This needs [Rust](https://rustup.rs) 1.96 or later, and takes a few
minutes. The package is [`little-docs`](https://crates.io/crates/little-docs),
but the command is `little`. To check it works, run `little --version`.

## Install without Rust

Download the archive for your system, unpack it, and put `little`
(`little.exe` on Windows) on your `PATH`:

| System | Archive |
| --- | --- |
| macOS, Apple silicon | [`little-aarch64-apple-darwin.tar.gz`](https://github.com/withlittle/little/releases/latest/download/little-aarch64-apple-darwin.tar.gz) |
| macOS, Intel | [`little-x86_64-apple-darwin.tar.gz`](https://github.com/withlittle/little/releases/latest/download/little-x86_64-apple-darwin.tar.gz) |
| Linux, x64 | [`little-x86_64-unknown-linux-musl.tar.gz`](https://github.com/withlittle/little/releases/latest/download/little-x86_64-unknown-linux-musl.tar.gz) |
| Linux, arm64 | [`little-aarch64-unknown-linux-musl.tar.gz`](https://github.com/withlittle/little/releases/latest/download/little-aarch64-unknown-linux-musl.tar.gz) |
| Windows, x64 | [`little-x86_64-pc-windows-msvc.zip`](https://github.com/withlittle/little/releases/latest/download/little-x86_64-pc-windows-msvc.zip) |
| Windows, arm64 | [`little-aarch64-pc-windows-msvc.zip`](https://github.com/withlittle/little/releases/latest/download/little-aarch64-pc-windows-msvc.zip) |

The Linux builds are static, so they run on any distribution. On a Mac
with Apple silicon, this puts little in `/usr/local/bin`:

```sh
curl -fsSL https://github.com/withlittle/little/releases/latest/download/little-aarch64-apple-darwin.tar.gz \
  | sudo tar -xz -C /usr/local/bin little
```

For another system, change the archive's name in the URL. If you use
[cargo-binstall](https://github.com/cargo-bins/cargo-binstall),
`cargo binstall little-docs` downloads the right archive for you.

### If macOS or Windows blocks it

little isn't signed yet, so your system may block a copy you downloaded in
a browser. On macOS, run `xattr -d com.apple.quarantine little`, or use
the `curl` command above, which avoids this. On Windows, run
`Unblock-File little-x86_64-pc-windows-msvc.zip` in PowerShell before you
unpack the archive.

### Check a download

Each release has a `SHA256SUMS` file. Download it next to the archive, and
run:

```sh
sha256sum --check --ignore-missing SHA256SUMS      # Linux
shasum -a 256 --check --ignore-missing SHA256SUMS  # macOS
```

On Windows, compare `Get-FileHash`'s output with the archive's line in
`SHA256SUMS`.

The [GitHub CLI](https://cli.github.com) can also check that little's
release workflow built the archive, and that it hasn't changed since:

```sh
gh attestation verify little-aarch64-apple-darwin.tar.gz --repo withlittle/little
gh release verify-asset little-aarch64-apple-darwin.tar.gz --repo withlittle/little
```

## Update it

Run the command you installed little with again, or download the latest
archive. Each release's [notes](https://github.com/withlittle/little/releases)
list what's changed, starting with what breaks.

little follows [Semantic Versioning](https://semver.org/). Until 1.0, a
minor release, like 0.3.0, adds features and can break your site, theme,
or plugins. A patch release, like 0.2.1, only fixes bugs or makes little
faster, so it's safe to update to. Each of little's plugins has its own
version, numbered the same way.

## Build from source

To install the latest code on `main`, before it's released:

```sh
cargo install --locked --git https://github.com/withlittle/little little-docs
```
