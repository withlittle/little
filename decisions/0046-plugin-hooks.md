# 0046 — Plugin hooks

- Status: accepted
- Date: 2026-10-15
- Amends: [0019](./0019-site-search.md),
  [0029](./0029-plugins-that-render-code-blocks.md),
  [0030](./0030-plugins-that-add-pages.md),
  [0031](./0031-a-plugin-for-pythons-docstrings.md),
  [0044](./0044-a-plugin-for-rusts-doc-comments.md)

## Context
A plugin rendered fenced code blocks ([0029](./0029-plugins-that-render-code-blocks.md)),
and, built for a second world, `plugin-with-pages`, added pages
([0030](./0030-plugins-that-add-pages.md)). Each new thing a plugin could
do would have been another world, and another kind of plugin, but authors
think of a build as steps, and want to change what one makes: open each
link to another site in a new tab, add Open Graph tags to the theme's
`<head>`, or write a `sitemap.xml`. None of these is a code block or a
page.

Static site generators let plugins in at the build's steps: MkDocs' event
hooks, like `on_page_content`, are each given what the build has made, and
give back what it goes on with, as Eleventy's transforms and Jekyll's
hooks change it. Nobody uses little's plugins yet but little's own.

## Options considered
- **Kinds of plugin:** a world for each, as before; or one kind, with a
  hook for each step of the build, each its own interface, so a plugin
  exports only those it needs, and there's no kind to choose.
- **What a hook is:** an event, which tells a plugin what happened, with
  more functions to change it; or a filter, given what the build has made
  so far, giving back what it goes on with, so changing a thing is one
  call.
- **Which steps:** `before_build` and `after_build`, which leave a plugin
  to find and parse the files it changes; or one for each thing the build
  makes, from the config to the destination's files, so each is given
  what it changes, as little has it.
- **Changing markup:** only the theme writes markup (principle 1); or a
  plugin can change the theme's files, and each page as it's written, as
  the site's author chooses, while little itself still writes none.
- **Search and link checks:** before plugins, or after, so a reader's
  search and the links checked are those of the site as it's published.
- **A hook that fails:** stop the build; or, for one called for each page,
  warn, as of a block a plugin can't render, and go on without it.
- **Plugins built already:** keep 0.1.0's worlds beside the hooks; or a
  new version of the package, 0.2.0, which they're built again for.

## Decision
- `wit/plugin.wit` is `little:plugin@0.2.0`: an interface for each hook,
  and one, `warnings`, that a plugin imports, with `warn`. A plugin's world
  exports the hooks it needs, like `world links { export
  little:plugin/content@0.2.0; }`; `world hooks` exports every one, for
  little's own bindings.
- little calls each hook as the build reaches it, in this order, each given
  what the build has made, giving back what it goes on with:

  | Hook | Called | Given |
  | ---- | ------ | ----- |
  | `setup` | as the plugin starts | its settings |
  | `config` | once | the site's config |
  | `theme` | once | the theme's files, layered |
  | `files` | once | the source's files' paths, for files to add and leave out |
  | `source` | for each page | its text |
  | `code-blocks` | for each block in its `languages` | the block |
  | `content` | for each page | its HTML, before the theme |
  | `site` | once | `site` |
  | `page` | for each page | `page` |
  | `html` | for each page | its HTML, from the theme |
  | `destination` | once | the destination's files' paths, for files to add |
- Each hook of each plugin that has it is called in the order `plugins`
  names them, a theme's first, then the site's, each given what the one
  before gave back.
- One called once that fails, as it gives back an error, or crashes,
  stops the build. One called for each page or block is warned of, naming
  the plugin, the hook, and the page, and the build goes on with what it
  was given; `--strict` fails on it. After a crash, little starts the
  plugin again, calling `setup`, before the next hook.
- `config` can't change `theme`, `plugins`, `source`, `destination`,
  `host`, or `port`, which little reads before plugins start. `theme`'s
  files stay in `_templates/`, `_static/`, and `_sass/`, with
  `_templates/default.html`. `files` adds each file as if it were in the
  source, so a Markdown file or notebook is a page, as `pages`'s were, and
  can add one where there's one only if it leaves that one out.
  `destination` can't add a file where there's one. Each path a hook gives
  back is relative, without `..`, `\`, or `:`, so it's in its folder on
  any system, Windows too, and, as on a disk, no file it adds is where a
  folder is, or in a file. Any of these stops the build, before the
  destination is touched, so the last build stays.
- `site`'s `nav` stays a list of `{ title, url, children }`, which
  `page.prev`, `page.next`, and `page.breadcrumbs` follow.
- Search indexes the `title` and `content` that `page` gives back, so it
  sees what `source`, `code-blocks`, `content`, and `page` change, but not
  `html`. Links are checked in what `html` gives back.
- A version from a git ref starts its plugins again, with its own
  settings, and calls each hook but `config` and `theme`, as it's built
  with the site's config and theme
  ([0042](./0042-versions-share-the-sites-config-and-theme-files.md)).
- A plugin built for 0.1.0 doesn't load:
  ``loading plugin `csv` from ./csv.wasm: it exports `little:plugin/code-blocks@0.1.0`, but none of little's hooks, from `little:plugin@0.2.0` ``.

## Consequences
- A plugin is one kind of thing, which does as much as it needs, from a
  single hook: "Writing a plugin" in the manual is a plugin that opens
  links to other sites in a new tab, with `content`, and "Plugin hooks"
  documents each hook.
- `docstrings` and `doc_comments` add their pages with `files`, and warn
  with `warn`. A plugin's page can be a notebook, and a plugin can add a
  file that isn't a page.
- A site's markup can depend on its plugins, as its author chooses; a
  theme still owns it, and little still writes none.
- Plugins built for 0.1.0 must be built again, for 0.2.0.
- Limits, for now:
  - `little config` prints the config before plugins change it;
  - warnings' line numbers count the lines `source` gives back;
  - headings `content` adds aren't in `page.toc`;
  - an `.html` file in the source is copied, not a page, so only `files`
    sees it;
  - a hook has no time limit.
