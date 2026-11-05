---
title: Install
description: Install little with one command, and check it works.
---

# Install

```sh
cargo install --locked little-docs
```

This needs [Rust](https://rustup.rs) 1.96 or later. If yours is older, run
`rustup update` first. The build takes a few minutes.

The package is called [`little-docs`](https://crates.io/crates/little-docs),
but the command is `little`. `--locked` uses the dependency versions little
was tested with.

Check it works:

```sh
little --version
```

It prints little's version, like `little 0.3.0`. Run `little --help` to list
every [command](../reference/commands.md).

Next, start [your first site](first-site.md).

## Install without Rust

Download the archive for your system, unpack it, and put `little`
(`little.exe` on Windows) on your `PATH`.

| System | Archive |
| --- | --- |
| macOS, Apple silicon | [`little-aarch64-apple-darwin.tar.gz`](https://github.com/withlittle/little/releases/latest/download/little-aarch64-apple-darwin.tar.gz) |
| macOS, Intel | [`little-x86_64-apple-darwin.tar.gz`](https://github.com/withlittle/little/releases/latest/download/little-x86_64-apple-darwin.tar.gz) |
| Linux, x64 | [`little-x86_64-unknown-linux-musl.tar.gz`](https://github.com/withlittle/little/releases/latest/download/little-x86_64-unknown-linux-musl.tar.gz) |
| Linux, arm64 | [`little-aarch64-unknown-linux-musl.tar.gz`](https://github.com/withlittle/little/releases/latest/download/little-aarch64-unknown-linux-musl.tar.gz) |
| Windows, x64 | [`little-x86_64-pc-windows-msvc.zip`](https://github.com/withlittle/little/releases/latest/download/little-x86_64-pc-windows-msvc.zip) |
| Windows, arm64 | [`little-aarch64-pc-windows-msvc.zip`](https://github.com/withlittle/little/releases/latest/download/little-aarch64-pc-windows-msvc.zip) |

On a Mac with Apple silicon, this downloads little and puts it in
`/usr/local/bin`:

```sh
curl -fsSL https://github.com/withlittle/little/releases/latest/download/little-aarch64-apple-darwin.tar.gz \
  | sudo tar -xz -C /usr/local/bin little
```

For another system, change the archive's name in the URL. To put little
somewhere else, like `~/.local/bin`, change the folder after `-C`, and
drop `sudo`. The Linux builds are static, so they run on any distribution.

If you use [cargo-binstall](https://github.com/cargo-bins/cargo-binstall),
`cargo binstall little-docs` downloads the right archive for you.

### If macOS or Windows blocks it

little isn't signed yet, so your system may block a copy you downloaded in
a browser. The `curl` command above avoids this on macOS.

- **macOS** says Apple can't check it for malware. Choose **Done**, then
  either run `xattr -d com.apple.quarantine little`, or choose
  **Open Anyway** in **System Settings** > **Privacy & Security**. Then run
  little again.
- **Windows** says it protected your PC. Choose **More info**, then
  **Run anyway**. Or, before you unpack the archive, unblock it in
  PowerShell: `Unblock-File little-x86_64-pc-windows-msvc.zip`.

### Check a download

Each release has a `SHA256SUMS` file. Download it next to the archive, and
run:

```sh
sha256sum --check --ignore-missing SHA256SUMS      # Linux
shasum -a 256 --check --ignore-missing SHA256SUMS  # macOS
```

On Windows, compare the output of `Get-FileHash`, in PowerShell, with the
archive's line in `SHA256SUMS`.

With the [GitHub CLI](https://cli.github.com), you can also check that
little's release workflow built the archive, and that it hasn't changed
since:

```sh
gh attestation verify little-aarch64-apple-darwin.tar.gz --repo withlittle/little
gh release verify-asset little-aarch64-apple-darwin.tar.gz --repo withlittle/little
```

For an older release, add its tag, like `v0.1.0`, before the archive's
name in `gh release verify-asset`.

## Update it

Run the command you installed little with again, or download the latest
archive. Each release's [notes](https://github.com/withlittle/little/releases)
list what's changed.

little follows [Semantic Versioning](https://semver.org/). Until 1.0, a
minor release, like 0.3.0, adds features and can break your site, theme,
or plugins. Its notes list what breaks first. A patch release, like 0.2.1,
only fixes bugs or makes little faster, so it's safe to update to. Each of
little's plugins has its own version, numbered the same way.

## Build from source

To install the latest code from the `main` branch:

```sh
cargo install --locked --git https://github.com/withlittle/little little-docs
```

Or clone the repository, and install it from there:

```sh
git clone https://github.com/withlittle/little.git
cd little
cargo install --locked --path .
```
