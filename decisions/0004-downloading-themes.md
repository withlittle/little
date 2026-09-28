# 0004 — Downloading themes

- Status: accepted
- Date: 2026-09-28
- Amends: [0003](./0003-choosing-and-layering-themes.md)

## Context
A company wants one theme for all its sites. It builds on a published theme
and changes only the fonts and colours, perhaps with a few templates of its
own. With `theme` limited to folders, every site had to keep a copy of the
published theme beside it. Published themes live on GitHub, which offers every
tag as a `.zip` and a `.tar.gz`. Each archive holds one folder, like
`fancy_theme-5.0.4/`.

## Options considered
- **Where to keep a download:** the OS's cache folder, shared by every site,
  or a `.little/` folder in the site. Kept in the site, it's easy to find and
  to delete, and little changes nothing outside the site.
- **When to download again:** on every build, when a flag asks, when the
  server says the archive changed, or never. A tag's archive never changes,
  and if little never downloads again, builds work offline and need no flag.
- **Telling a `.zip` from a `.tar.gz`:** by the end of the URL, by the
  `Content-Type` header, or by the file's first bytes. An archive's URL needn't
  end in `.zip` or `.tar.gz`, and servers often send a vague
  `application/octet-stream`. The first bytes are always there.
- **Crates:** `reqwest` or `ureq` for HTTPS; `zip`; `tar` and `flate2`.
  `reqwest` brings an async runtime (tokio) that little doesn't otherwise
  need. `ureq` blocks until the download is done, which is all little needs.
  Its `rustls` feature does TLS in Rust, so it needs no OpenSSL on the machine.
- **Unsafe archives:** unpack them as they are, or refuse paths that lead out
  of the archive (like `../../.bashrc`, known as "zip-slip") and links. A link
  could point anywhere, like `~/.ssh`, and a build would copy what it points
  at into the site.

## Decision
- `theme`, in a site's or a theme's `little.yml`, can be the `https://` URL of
  a `.zip` or `.tar.gz` archive that holds a theme in a single folder.
  `http://` is an error.
- The first build that needs the theme downloads the archive, unpacks it, and
  keeps the folder inside as `.little/themes/<hash of the URL>/` in the site
  being built. little writes a `.gitignore` into `.little/` so that git
  ignores it.
- little never downloads a URL it has kept. To download it again, delete
  `.little/`. To use another version, change the URL.
- An archive's type comes from its first bytes. Anything that isn't a `.zip`
  or a `.tar.gz` is an error. So is an archive that holds more than one thing
  at its top, a symlink, a hard link, or a path that leads out of it. A failed
  download or unpack leaves no theme behind, so the next build tries again.
- A downloaded theme is a theme like any other. Its own `theme` can be a
  path, relative to its folder, or a URL. Loops are still found by comparing
  folders, and reported with the URLs.
- `little serve` doesn't watch downloaded themes, as they never change.
- New crates:
  - `ureq`, with only `rustls`, for HTTPS;
  - `zip`, with only deflate;
  - `tar`, with no extras;
  - `flate2`, for gzip, which little already built in for syntect, the syntax
    highlighter comrak uses.

## Consequences
- A company theme can be just a `little.yml` that sets `theme` to the URL of
  the theme it builds on, plus any files it changes.
- Once a theme is downloaded, the site builds offline. A branch's archive
  changes, but little downloads it again only after `.little/` is deleted.
- little grows from about 5.8 MiB to about 8.1 MiB, mostly for TLS, and by
  about 22 crates.
- Limits, for now:
  - little can't sign in, so a private repository's archive answers 404;
  - there's no timeout and no size limit, so a stalled download waits until
    you press Ctrl-C;
  - little trusts only the public certificate authorities it's built with, so
    a proxy that inspects HTTPS with a company's own certificate is refused.
    A proxy set in `HTTPS_PROXY` works;
  - links are refused even where a theme wouldn't use them;
  - a `.zip` must be compressed with deflate, or not at all, as nearly all
    are, GitHub's included;
  - files lose their permissions and timestamps, which themes don't need;
  - with `source: .`, `.little/` is inside the source, so a downloaded theme's
    Markdown becomes pages;
  - while a download fails, `little serve` tries it twice each time you save,
    as it reads the config twice.
