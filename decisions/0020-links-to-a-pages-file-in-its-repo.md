# 0020 — Linking to a page's file in its repository

- Status: accepted
- Date: 2026-10-03

## Context
Many documentation sites link to the repository their source is in, and from
each page to its Markdown file there, to view it, or to edit it and suggest a
change, as MkDocs, Docusaurus, and VitePress can. A theme can show the links,
but it doesn't know where the repository is, where each page's file is in it,
or how its host makes links to a file: GitHub's are `/blob/main/…` and
`/edit/main/…`, and GitLab's `/-/blob/main/…` and `/-/edit/main/…`. A site
is often in a folder of a bigger repository, like `website/`, so a file's
path in the source isn't its path in the repository.

## Options considered
- **Give the theme each page's path,** like Jekyll's `page.path` and Hugo's
  `.File.Path`, and let it make the links. Every theme would need to know
  how each host makes its links, and where the site is in the repository.
- **A pattern for each link,** like MkDocs' `edit_uri_template` and
  VitePress's `editLink.pattern`, with a placeholder for the page's path. It
  works with any host, but every site has to write the patterns, and get
  them right.
- **Read the repository's address from git,** from its `origin` remote:
  nothing to set, but a remote can be an SSH address, a fork, or missing, as
  in some CI, and the links would change with it.
- **A `repo` setting,** the repository's address, as a browser shows it,
  from which little works out each page's links on the hosts it knows, and
  gives them to the theme, as it does a page's other URLs.

## Decision
- A site sets `repo` to the address of its repository, like
  `https://github.com/withlittle/little`, and `branch` to the branch to link
  to, which little sets to `main`, as a file can be edited only on a branch.
  Both cascade like any config.
- `repo` must start with `https://` or `http://`, with a path after its host,
  and nothing after that (no `?` or `#`). It isn't the address it's cloned
  from, so it doesn't end in `.git`. `branch` must be text, so a branch like
  `2.0` is quoted. Otherwise, every command stops and says why.
- The theme gets `repo` as `site.repo`, as written. On `github.com` and
  `gitlab.com`, each page gets `page.source_url` and `page.edit_url`, where
  to view and edit its file on the branch, with each part of the path
  percent-encoded.
- A file's path starts at the root of the repository: the nearest folder,
  from the source up, with a `.git` in it, which is a file in a worktree, or,
  without one, the site's folder, as in a copy of the repository. A source
  folder outside the repository has no links to its files.
- The 404 page has neither link, as it has no file. A page's frontmatter can
  change or turn off either, like any other key
  ([0006](./0006-frontmatter-and-config-win.md)), as `edit_url: false` does
  on a page that's generated from something else.
- The built-in theme links to `site.repo` in its header, where there's room,
  and to `page.edit_url` and `page.source_url` below each page's content.
- Links into the repository aren't checked
  ([0016](./0016-checking-links.md)): a new page's file isn't there until it's
  pushed, which would fail `--strict` in CI, a link checker can't see the
  files in a private repository, and each page would have two more links to
  check over the network.
- No new crates.

## Consequences
- A site sets one line, and its theme can link to its repository and to each
  page's file there, with nothing to work out for each host.
- Limits, for now:
  - only `github.com` and `gitlab.com` get links to each page's file. GitHub
    Enterprise, a GitLab of your own, Gitea, and Bitbucket get only
    `site.repo`. A pattern for each link, as MkDocs has, could add them later;
  - a theme doesn't get a page's path, like `docs/guide/install.md`, for a
    link of its own, like one to the page's history;
  - a site with no `.git` of its own, in a folder with one, like a home folder
    kept in git, takes that folder as the root of its repository;
  - as links into the repository aren't checked, a `repo` or `branch` with a
    typo in it gives broken links without a warning.
