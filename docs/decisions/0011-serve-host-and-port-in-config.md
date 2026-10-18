# 0011 — The config chooses where `little serve` serves

- Status: accepted
- Date: 2026-09-30

## Context
`little serve` served at `127.0.0.1:8080`, or at the address its `--addr`
flag gave, like `--addr 127.0.0.1:3000`. A site that always needs another
port, because something else has 8080 or it's served beside another site,
had to say so on every run, and everyone who served it had to know to. Where
little serves is a setting little owns, and those belong in the cascade, where
a site sets them once
([AGENTS](https://github.com/withlittle/little/blob/main/AGENTS.md)).

## Options considered
- **Where it's set:** in flags only, as before, and as in Zola, mdBook, and
  Docusaurus; or in the config too, as in Jekyll (`host` and `port` in
  `_config.yml`), MkDocs (`dev_addr` in `mkdocs.yml`), and Astro
  (`server.host` and `server.port`), with flags for a single run.
- **The names:** one setting for both, like MkDocs' `dev_addr: 127.0.0.1:8000`,
  which has to be written whole to change just the port; flat `host` and
  `port`, as in Jekyll; or both in a `serve` setting, as Astro puts them in
  `server`. In a template, `site.host` can read as the host the site is
  published at, where `site.serve.host` can't, but `port: 3000` is the
  simplest to write, and every setting little reads so far is flat.
- **The flags:** `--addr`, as before, which can't change just the port; or
  `--host` and `--port`, named as the settings are, as in Jekyll, Docusaurus,
  and Astro.
- **Port 0:** has the system pick a free port, so the URL little prints would
  have to come from the server, not the config.
- **When they're checked:** when the config loads, like every setting little
  reads, as MkDocs checks `dev_addr`; or only when `little serve` starts, so a
  bad `port` wouldn't stop `little build`.

## Decision
- `host` and `port` are where `little serve` serves the site. little's
  `little.yml` sets them to `127.0.0.1` and `8080`, `--addr`'s old default,
  and a theme's and a site's can override them, like any setting.
- `little serve --host` and `--port` override the config for a single run.
  They replace `--addr`.
- `host` must be a host name or an IP address, as text, and `port` a number
  from 1 to 65535, or the config doesn't load, and the error names the
  setting. An IPv6 address is written bare, like `::1`, and goes in brackets
  in the URL little prints.

## Consequences
- A site sets its host and port once, for everyone who serves it, and
  `little config` shows them. The flags still change them for a single run.
- `--addr` is gone: `little serve --addr 127.0.0.1:3000` is now
  `little serve --port 3000`.
- `little serve` reads them when it starts. Changing them while it runs
  rebuilds the site, as any change to `little.yml` does, but it keeps serving
  where it started, so tabs open at that address keep reloading. To move it,
  stop it and start it again.
- A bad `host` or `port` stops `little build` and `little config` too, though
  neither uses them.
- Like every setting, they reach the theme, as `site.host` and `site.port`.
  They're where the site is served while it's written, not where it's
  published, and they don't follow the flags, so a theme shouldn't build
  links from them.
- little now reads `host` and `port`, so a theme or site can't use them for
  settings of its own.
- Port 0 is an error.
