# 0075 — Finding anchors in the site's pages

- Status: accepted
- Date: 2026-11-12
- Amends: [0016](./0016-checking-links.md)

## Context
lychee checks a link into a page of the site, like `guide/#usage`, by
reading the page's file and looking for the anchor among its `id`s
([0016](./0016-checking-links.md)). lychee 0.24 reads the whole file for
each such link, before it looks in its cache of the file's anchors, and
finds the anchors of one file at a time.

The built-in theme gives each page about eight of these links, to itself:
its table of contents, its headings' own links, and the link that skips to
its content. A site of 2,000 pages, as `scripts/bench.sh` writes it, has
16,000, each a read of a page, which is 335 KB with the whole nav on it.
Checking its links took longer than building it, on an Apple silicon Mac:

| | Built in | Links checked in |
| --- | --- | --- |
| The whole nav on every page | 8 s | 12 s |
| With `nav_prune` | 3.6 s | 1.1 s |

`little serve` checks the links within the site after every save.

## Options considered
- **Leave it to lychee,** and report it there. A fix would reach little
  with a later release, and wouldn't find the anchors of more than one
  file at once.
- **little checks every link within the site,** and lychee only those
  that leave it. little would then decide which file a link leads to: a
  folder's `index.html`, a link to the site's own `url`, a link from the
  root of the domain. lychee does that already, and isn't slow at it.
- **little finds only the anchors.** lychee still checks that the file is
  there, once for all the links to it, and little reads each file once
  for its `id`s, several files at a time.

## Decision
- lychee still reads the built site for its links, and checks each one
  that isn't to an anchor in a file as before.
- For a link to an anchor in a file, lychee checks the link without its
  fragment, once for each file linked to, so what it says of a file that
  isn't there, or a folder with no `index.html`, is unchanged.
- little then looks for the anchor in the file, or in its folder's
  `index.html`, among the `id` and `name` of each element, as lychee
  finds them, read with html5gum, the tokenizer lychee reads HTML with.
  It reads each file once, on as many threads as the machine has cores.
- What counts as found is as lychee has it: the fragment as it's written,
  or decoded, like `café` for `caf%C3%A9`; `#top`, in any case, without
  looking; any fragment of a file that isn't HTML, like an image; and any
  fragment of a file that can't be read. A link that isn't found is
  warned of in lychee's words, "Cannot find fragment".
- A link to text in a page, like `#:~:text=words`, or into a Markdown
  file in the destination, is left to lychee whole.

## Consequences
- The same site's links are checked in 7.3 seconds with the whole nav on
  every page, and in 0.6 with `nav_prune`. What's left is lychee reading
  each page for its links.
- The warnings are the same, so the manual says nothing new.
- html5gum is a dependency of little's own, at the version lychee already
  brings, so little's binary and its notices are the same.
- A page is read twice, once by lychee, for its links, and once by
  little, for its anchors, where lychee read it once for each link to it.
- If lychee changes what it counts as an anchor, little's rule has to
  follow it: a test holds each case above.
