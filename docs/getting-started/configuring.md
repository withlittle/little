---
title: Configuring
description: Name your site, and choose its folders and repository, in little.yml.
---

# Configuring

A site's settings go in `little.yml`, next to its `docs/` folder:

```yaml
# little.yml
name: My Project
description: Docs that just work
```

Set only what you want to change. Everything else has a default, so the
file can be empty, or missing. To see every setting your site ends up
with, run `little config`. [Settings](../reference/settings.md) lists them
all.

## How settings combine

Settings come from three places, each over the one before:

1. little's defaults
2. the theme's `little.yml`
3. your site's `little.yml`

So your site has the last word. little's defaults cover only its own
settings, like the folders a site is built from. A site's name, language,
and look are up to the theme and the site.

You can add settings of your own, too. Each reaches the theme as
`site.<key>`, so a theme could show `announcement: "2.1 is out!"` as
`site.announcement`.

## The built-in theme's settings

| Key | Default | Used for |
| --- | ------- | -------- |
| `name` | `Documentation` | The site's title, in the header and the browser's tab |
| `description` | none | Text beside the site's title |
| `font` | a system UI font stack | The text's `font-family` |
| `lang` | `en` | Each page's `lang`, which [search](../writing/search.md) matches words in |
| `search` | `true` | The [search box](../writing/search.md) in the header, and the index it searches |
| `repo` | none | A link to the [repository](#choosing-the-sites-repository) in the header, and links to view and edit each page's file there, below the page |

## Choosing folders

little builds a site from `docs/` into `dist/`. To use other folders, set
`source` and `destination`:

```yaml
# little.yml
source: ./content
destination: ./public
```

Paths are relative to the folder with your `little.yml`, or absolute.

Each build wipes the destination first, so use a folder that holds only
builds. A build won't wipe a folder that has other files in it, unless
little built it.

## Choosing the site's repository

Set `repo` to your repository's address, as your browser shows it:

```yaml
# little.yml
repo: https://github.com/withlittle/little
```

The built-in theme then links to the repository in its header. If it's on
github.com or gitlab.com, the theme also links to view and edit each page's
file, below the page.

Those links are to the `main` branch. For another, set `branch`, like
`branch: trunk`. Quote a branch that looks like a number, like
`branch: "2.0"`.

A page's [frontmatter](../writing/frontmatter.md#frontmatter-wins) can
change its links, or remove one, like `edit_url: false` on a page that's
generated from something else.
