#!/usr/bin/env bash
# Times little building a large site, to measure a change to how fast it
# builds: a site of PAGES pages, 2,000 unless given, each of 4 KB of
# Markdown, in folders of 50, built with the built-in theme, with the whole
# nav on every page and then with `nav_prune`.
#
#   scripts/bench.sh [PAGES]
#
# It builds little's release binary first, and needs perl, for a clock.
# Timings vary from run to run, and from machine to machine, so compare
# those of one machine, and run it more than once. To see where the time
# goes, profile a build of the same site, kept with KEEP=1, by a binary
# with its symbols: cargo build --profile profiling.
set -euo pipefail

if [[ $# -gt 1 || ! ${1:-2000} =~ ^[1-9][0-9]*$ ]]; then
  echo "usage: $0 [PAGES]" >&2
  exit 2
fi
pages=${1:-2000}
root=$(cd "$(dirname "$0")/.." && pwd)

cargo build --release --locked --quiet --manifest-path "$root/Cargo.toml"
little=$root/target/release/little

site=$(mktemp -d)
if [[ -n ${KEEP:-} ]]; then
  echo "the site is kept in $site"
else
  trap 'rm -rf "$site"' EXIT
fi

# A page links to the one before it, in its folder or the one before, so
# the link check has a link to follow from each, and none leaves the site.
paragraph="Some *text* with \`code\` in it, and enough words after them to make a paragraph of the length one in a manual often is, more or less."
mkdir -p "$site/docs"
echo "# A site of $pages pages" > "$site/docs/README.md"
for ((page = 1; page <= pages; page++)); do
  folder=$(printf 'folder-%03d' $(((page - 1) / 50 + 1)))
  if ((page % 50 == 1 || pages == 1)); then
    mkdir -p "$site/docs/$folder"
    echo "# Folder ${folder#folder-}" > "$site/docs/$folder/README.md"
  fi
  if ((page == 1)); then
    before=../README.md
  elif ((page % 50 == 1)); then
    before=$(printf '../folder-%03d/page-%05d.md' $(((page - 2) / 50 + 1)) $((page - 1)))
  else
    before=$(printf 'page-%05d.md' $((page - 1)))
  fi
  {
    echo "# Page $page"
    echo
    echo "After [the page before]($before)."
    for section in 1 2 3 4 5 6; do
      echo
      echo "## Section $section of page $page"
      for _ in 1 2 3 4; do
        echo
        echo "$paragraph"
      done
      echo
      echo '```rust'
      echo "fn main() { println!(\"page {}, section {}\", $page, $section); }"
      echo '```'
    done
  } > "$site/docs/$folder/$(printf 'page-%05d.md' "$page")"
done

# `little build` prints `built ...` once the site's written, then checks
# its links, so the time before that line is the build's, and the time
# after it the check's.
clock='BEGIN { $| = 1; $start = time }
  if (/^built /) { $built = time } else { print STDERR $_ }
  END { printf "%.1f %.1f\n", $built - $start, time - $built if $built }'

echo "$pages pages, with $("$little" --version)"
printf '%-18s %10s %16s %8s\n' "" "Built in" "Links checked in" "Size"
for prune in false true; do
  echo "nav_prune: $prune" > "$site/little.yml"
  times=$("$little" build -C "$site" | perl -MTime::HiRes=time -ne "$clock")
  read -r built checked <<< "$times"
  size=$(du -sk "$site/dist" | cut -f1)
  printf '%-18s %8s s %14s s %5s MB\n' "nav_prune: $prune" "$built" "$checked" $((size / 1024))
done
