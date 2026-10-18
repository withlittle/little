# 0045 — Limits on downloads

- Status: accepted
- Date: 2026-10-15
- Amends: [0004](./0004-downloading-themes.md),
  [0029](./0029-plugins-that-render-code-blocks.md)

## Context
The first build that needs a theme or a plugin from a URL downloads it
([0004](./0004-downloading-themes.md),
[0029](./0029-plugins-that-render-code-blocks.md)), holding it in memory,
and unpacks a theme's archive into the site's `.little/`. Nothing limited
either. A server that stopped answering held the build up until you pressed
Ctrl-C, or, in CI, until the job timed out, with no word of why; one that
sent without end filled the memory; and an archive of a few kilobytes, of
zeros, say, could unpack to fill the disk.

## Options considered
- **Settings, or fixed limits:** a setting for each limit lets a site on a
  slow connection wait longer, but it's a setting for a case few sites have,
  and a theme or a plugin is small. Fixed limits, generous enough for any
  theme or plugin, ask nothing of an author, and a setting can come later.
- **How long:** one limit on the whole download, or one on how long the
  server takes to connect, and then to answer, too. A whole download's limit
  must leave time for a big file on a slow connection, so it's minutes; a
  server that doesn't answer at all is found in seconds.
- **How much:** ureq's limit on a response's body, or little's own count as
  it reads. ureq's stops reading at the limit, and says so in its own error,
  which little words as it does its others.
- **What an archive unpacks to:** the size its files say they are, or the
  bytes little writes. An archive's sizes are what its maker wrote, and
  needn't be true; the bytes written are.

## Decision
- A download stops if the server takes over 30 seconds to connect, its TLS
  handshake included, or 30 more to answer, or if it takes over 5 minutes
  in all, its redirects included, or is over 200 MB.
- A theme's archive stops unpacking once its files, together, come to over
  1 GB, as little writes them.
- Either way, nothing is kept, so the next build tries again, and the error
  finishes little's sentence, as a server's answer does, like
  "`little.yml` sets `theme` to `https://…`, but little couldn't download
  it: it took over 5 minutes". The others end "the server didn't answer in
  30 seconds", "it's over 200 MB, the most little downloads", and, for an
  archive, "little couldn't unpack it: it unpacks to over 1 GB, the most
  little unpacks".
- The limits are in `src/download.rs`, not the config.
- No new crates: ureq has the timeouts and the limit on a body.

## Consequences
- A server that doesn't answer stops the build within a minute, and a
  download that crawls, within 5, saying why, where before they held it up
  for good.
- 200 MB is over ten times the size of a plugin in Python, which carries
  Python with it, in 18 MB.
- On a connection too slow for 5 minutes, download the archive or the
  `.wasm` file yourself, and set `theme` to the theme's folder, or `wasm`
  to the file.
- Limits, for now:
  - the limits can't be changed without changing little;
  - a download still isn't checked against a hash;
  - only the size of an archive's files is limited, not how many there are.
