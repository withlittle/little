# 0006 — Frontmatter and config have the last word

- Status: accepted
- Date: 2026-09-29

## Context
Of a page's frontmatter, only `title` and `description` reached the theme.
little dropped every other key, so a theme couldn't offer settings of its own,
like `tags` or a `hero` image. And little built `site.nav` from the folders
over whatever the config set, so a `nav` in `little.yml` was silently thrown
away, and a site couldn't order its sidebar. Config cascades so that the last
writer wins, and the author writes last, but in both cases little had the last
word.

## Options considered
- **Where frontmatter goes:** flat on `page`, as in Jekyll (`page.tags`), or
  in a namespace of its own: `page.meta` in MkDocs, `.Params` in Hugo,
  `page.extra` in Zola, `frontMatter` in Docusaurus. A namespace can't clash
  with what little adds to `page` later, but an author has to learn which keys
  go where. Flat keys read as they're written.
- **Which keys:** a list of the ones little knows, which themes can't add to,
  or all of them.
- **Who wins a clash:** what little works out, as in Jekyll, where a page's
  `url` and `content` override any in its front matter, or the author, as in
  the cascade.
- **How it wins:** each key replaced whole, or merged like the cascade: maps
  key by key, anything else replaced. Merging lets a page change part of what
  little works out, like the title of `page.prev`, without repeating the rest.
- **The nav:** little's over the config's, as before, or the config's over
  little's.

## Decision
- Every key in a page's frontmatter reaches the theme as `page.<key>`, as
  written, and merges over what little works out for the page, the way a
  `little.yml` merges over the level beneath. So it wins even for `type`,
  `url`, `title`, `content`, `nav`, `children`, `prev`, and `next`.
- A key left blank is ignored, so a page still never holds null
  ([0002](./0002-theme-structure.md)), and `title:` keeps little's title.
- Frontmatter that isn't `key: value` settings, like a list or plain text, is
  a build error naming the page, as it is in a `little.yml`.
- Likewise, what little works out is the bottom of `site`: a `nav` in the
  theme's or site's `little.yml` replaces little's. It must be a list of
  `{ title, url, children }`, or the build fails. `page.prev` and `page.next`
  follow the `site.nav` the theme gets.

## Consequences
- A theme can offer settings of its own, like `tags` or a `hero` image,
  without little knowing about them.
- A key little adds to `page` later may already be in a page's frontmatter,
  and there the author's value wins. Flat keys take that risk, as in Jekyll.
- Frontmatter changes what the theme sees, not what little does: `url`
  doesn't move the page, and `type` doesn't choose its template. little reads
  just `title`, for the nav. Moving a page, or choosing its template, would be
  features of their own.
- A page can now turn off its previous or next link (`next: false`), rename
  it (`prev: { title: Start here }`), or point it elsewhere, which
  [0005](./0005-previous-and-next-pages.md) said a page couldn't do.
- As 0005 foresaw, the previous and next links follow a hand-written nav, and
  a page that isn't in it gets neither. The nav must use each page's URL as
  little builds it (`/guide/`, not `/guide`), or the theme can't mark the page
  as current and the page gets no links. little doesn't check that the nav's
  URLs lead to pages. `page.children` still lists what's in the page's folder.
- A config's `nav` used to be thrown away, so no theme relies on one.
- Frontmatter that isn't settings used to be ignored, and plain text between
  the `---` lines disappeared. Now the build stops and says so.
