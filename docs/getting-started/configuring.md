---
title: Configuring
description: How a site's config cascades over its theme's and little's, and the settings most sites change.
---

# Configuring

A site's config is YAML, in a `little.yml` beside its `docs/` folder. It
cascades through three levels: **little's built-in defaults → the theme's
`little.yml` → the site's `little.yml`**. Each level is deep-merged over the
one before, so the last writer wins. Both files are optional, and an empty
one is fine, so a site sets only what it wants to change:

```yaml
# little.yml
name: "My Project Docs"
description: "Docs that just work"
```

- little's own defaults, in its
  [`src/little.yml`](https://github.com/withlittle/little/blob/main/src/little.yml),
  cover only little's own settings, like the
  [theme](../themes/choosing-a-theme.md), the [folders](#choosing-folders) a
  site is built from and into, and the
  [Markdown](../writing/markdown.md#choosing-markdowns-extensions) its pages
  are written in. A site's name, language, and look are up to the theme and
  the site.
- Every key reaches the theme as `site.<key>`, so you can add your own, like
  `announcement: "2.1 is out!"`, for a theme that shows it.
- `little config` prints the config the theme gets, merged, so you can check
  a setting without building.
- [Settings](../reference/settings.md) lists every setting little has, and
  where it's documented.

## The built-in theme's settings

The built-in theme uses these keys, with defaults from its
[`little.yml`](https://github.com/withlittle/little/blob/main/theme/little.yml):

| Key | Default | Used for |
| --- | ------- | -------- |
| `name` | `Documentation` | The site's title, in the header and the browser's tab |
| `description` | none | Text beside the site's title |
| `font` | a system UI font stack | The text's `font-family` |
| `lang` | `en` | Each page's `lang`, which [search](../writing/search.md) matches words in |
| `search` | `true` | The [search box](../writing/search.md) in the header, and the index it searches |
| `repo` | none | A link to the [repository](#choosing-the-sites-repository) in the header, and links to edit and view each page's file there, below its content |

## Choosing folders

A site is built from its `source` folder into its `destination` folder:

| Key | Default | The folder… |
| --- | ------- | ----------- |
| `source` | `./docs` | whose `.md` files, and those in a format a [plugin](../plugins/using-plugins.md#pages-in-other-formats) reads, become the site's pages, and whose other files are copied into it |
| `destination` | `./dist` | the site is built into |

To use others, set them in `little.yml`:

```yaml
# little.yml
source: ./content
destination: ./public
```

- Paths are relative to the site's folder, the one holding its `little.yml`,
  or absolute. A theme's `little.yml` can set them too, but they're still
  relative to the site.
- The source can be a hidden folder itself, like `./.github`: only what's
  [hidden](../writing/pages-and-files.md#hidden-files) inside it is left out.
- With `source: .`, everything in the site's folder that isn't hidden is
  published, its `little.yml` included, but the destination: a build never
  copies itself.
- Every build wipes the destination, so point it at a folder that holds
  nothing but builds. little leaves a `.little-build` file in each folder it
  builds, and only wipes a folder that has one or is empty: pointed at any
  other folder, a build stops instead.

## Choosing the site's repository

Set `repo` to the address of the repository your site's source is in, as a
browser shows it, and a theme can link to it, and to each page's file there,
so readers can see the page's Markdown, or suggest a change to it:

```yaml
# little.yml
repo: https://github.com/withlittle/little
```

- The theme gets `repo` as `site.repo`. On GitHub and GitLab, each page gets
  where to view and edit its file too, as `page.source_url` and
  `page.edit_url`, like
  `https://github.com/withlittle/little/blob/main/docs/getting-started/configuring.md`
  and
  `https://github.com/withlittle/little/edit/main/docs/getting-started/configuring.md`
  for this page. The built-in theme links to the repository in its header,
  and to edit and view the page's file below its content.
- The links are to files on the `main` branch. Set `branch` for another, like
  `branch: trunk`, in quotes if it looks like a number, like `branch: "2.0"`.
- Each file's path starts at the root of the repository: the nearest folder,
  from `docs/` up, with a `.git` in it, or, without one, the site's folder. So
  a site in a `website/` folder links to `website/docs/guide/install.md`. A
  source folder outside the repository has no links to its files.
- A page's [frontmatter](../writing/frontmatter.md) can change either link,
  or turn it off, like `edit_url: false` on a page that's generated from
  something else.
- A repository anywhere else, like a GitLab or Gitea of your own, gets
  `site.repo`, but no `page.source_url` or `page.edit_url`: little doesn't
  know how its links are made.
- Links into the repository aren't
  [checked](../writing/links-and-images.md#checking-links).
- `repo` must start with `https://` or `http://`, with a path after its host,
  and nothing after that path (no `?` or `#`). It isn't the address you clone
  from, so it doesn't end in `.git`. Otherwise, every command stops and says
  why.
