#!/usr/bin/env bash
# Writes the third-party notices of little, or of one of its plugins, as its
# releases include them: see docs/decisions/0054-third-party-notices.md.
#
#   scripts/notices.sh little|pydoc|rustdoc|jsdoc|jupyter FILE
#
# It needs cargo-about 0.9.2: cargo install --locked cargo-about@0.9.2.
set -euo pipefail

usage="usage: $0 little|pydoc|rustdoc|jsdoc|jupyter FILE"
if [[ $# -ne 2 ]]; then
  echo "$usage" >&2
  exit 2
fi
component=$1
out=$2
root=$(cd "$(dirname "$0")/.." && pwd)
case $component in
  little) manifest=$root/Cargo.toml ;;
  pydoc | rustdoc | jsdoc | jupyter) manifest=$root/plugins/$component/Cargo.toml ;;
  *)
    echo "$usage" >&2
    exit 2
    ;;
esac

mkdir -p "$(dirname "$out")"
# rustup can install what rust-toolchain.toml asks for on the first cargo
# command, saying so on stderr, which would fail the check below: so run one
# first.
cargo metadata --locked --format-version 1 --manifest-path "$manifest" > /dev/null
log=$(mktemp)
trap 'rm -f "$log"' EXIT
# cargo-about reads the about.toml beside the manifest. It only warns of a
# licence file whose checksum has changed, or of a clarification no crate
# needs, so any warning fails this too.
if ! cargo about generate --locked --fail --manifest-path "$manifest" \
  --output-file "$out" "$root/about.hbs" 2> "$log" || [[ -s $log ]]; then
  cat "$log" >&2
  echo "error: cargo-about failed, or warned of something, as it wrote $component's notices" >&2
  exit 1
fi

if [[ $component == little ]]; then
  cargo run --locked --quiet --manifest-path "$manifest" --example syntax_notices >> "$out"
fi
