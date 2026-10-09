---
title: Install
description: Build little from source with Rust, and check it works.
---

# Install

little is a single binary, with no runtime, no `node_modules`, and no version
managers. There are no prebuilt binaries yet, so build it from source, with
a recent stable [Rust toolchain](https://rustup.rs).

## Build it from source

Cargo builds little and installs it into `~/.cargo/bin`, which rustup put on
your `PATH`:

```sh
cargo install --locked --git https://github.com/withlittle/little little
```

Or clone little's repository and install it from there, which gives you this
manual to read offline, with `little serve` in the clone:

```sh
git clone https://github.com/withlittle/little.git
cd little
cargo install --locked --path .
```

Either takes a few minutes the first time. In a clone, `cargo build --release`
builds the same binary without installing it, at `target/release/little`,
which you can copy anywhere.

## Check it works

```sh
little --version
```

prints `little`, and its version, like `little 0.1.0`. `little --help` lists
every [command](../reference/commands.md).

## Update it

little has no releases yet, so each install builds its `main` branch as it is
then. To update it, run the same `cargo install` again, after a `git pull` in
a clone.

Next, start [your first site](first-site.md).
