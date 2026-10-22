# 0057 — Starting a site with `little init`

- Status: accepted; amended by [0060](./0060-other-formats-in-plugins.md),
  under which a `README.ipynb` isn't a new site's home page, as no plugin
  reads it yet
- Date: 2026-10-19

## Context
A site needs only a `docs/` folder with a page in it, so the manual started
one with two commands for Unix's shells, `mkdir -p` and `echo`. That leaves
a site without a name, a `little.yml` to give it one in, or `dist/` kept
out of git, which each reader adds next, from the manual.
[VISION.md](https://github.com/withlittle/little/blob/main/VISION.md)'s
roadmap has had `little init`, to start a site in one command, as its third
slice, from the start.

Other tools start a project in different ways. Zola's `zola init` and
mdBook's `mdbook init` ask questions, and Zola's refuses a folder that
isn't empty. MkDocs' `mkdocs new` asks nothing. `cargo init` starts a
package in a folder that has files, but not one with a package already,
and adds `/target` to its `.gitignore`.

## Options considered
- **Questions:** prompts, for a name and settings, as Zola and mdBook ask;
  options, for each setting; or neither, with the settings a site sets next
  in `little.yml`, ready to uncomment.
- **Where:** a folder it's given, made if it isn't there, or the current
  one, as `cargo new` and `cargo init` do between them; or only the one
  `-C` names.
- **A folder with files in it:** refused, unless it's empty, as Zola's is;
  refused only where there's a `little.yml`, as `cargo init` refuses a
  package; or written over.
- **`.gitignore`:** left alone; written only where there isn't one; or
  added to, as `cargo init` adds to it.
- **`little.yml`:** every setting, commented out; or the site's name, and
  the few settings a site sets next.

## Decision
- `little init [PATH]` starts a site in `PATH`, in the folder `-C` names,
  as `git -C`'s paths are, making it if it isn't there, or in that folder
  itself, without one. It asks nothing, and has no options of its own.
- It writes:
  - `little.yml`, with `name`, the folder's name, written as YAML, so a
    name like `yes` or `123` is read back as it is, and `description`,
    `url`, and `repo` commented out, under a link to the manual's
    [Configuring](../getting-started/configuring.md) page;
  - `docs/README.md`, a home page that says how to add pages, and links to
    the manual, but to no page of the site, so a new site builds with
    `--strict`; unless `docs/` has a home page, a Markdown file or notebook
    called `README`, in any case, as a build finds one;
  - and `/dist/` in `.gitignore`, which it writes if it isn't there, or
    adds to the end of, unless a line in it is `dist`, `dist/`, `/dist`, or
    `/dist/`. `.little/`, where downloads are kept, ignores itself.
- It stops, writing nothing, where there's a `little.yml`, and writes its
  own last, so a run that fails part of the way through can be run again.
  Other files don't matter, so it starts a site in a project's folder, as
  `cargo init` does.
- It prints what it did with each file, `created`, `kept`, or
  `added /dist/ to`, then how to serve the site, like
  `next: cd my-docs && little serve`.
- It doesn't start a git repository, choose a theme, or write a workflow to
  publish the site.

## Consequences
- A site is one command, then `little serve`, and the README and the
  manual start one with `little init`.
- little 0.1.0 doesn't have `little init`, but the README, and the manual,
  published from `main` ([0056](./0056-publishing-the-manual.md)), tell
  readers to run it, until the next release has it.
- A site's called after its folder, like `my-docs`, until its `little.yml`
  says otherwise. The home page's heading is "Welcome", not the site's
  name, so its tab doesn't read "my-docs · my-docs".
- `/dist/` in `.gitignore` doesn't follow a `destination` set later.
- To publish a site on GitHub Pages, its `url` and `repo` still need
  uncommenting and filling in, and it needs the workflow on the
  [Publishing](../getting-started/publishing.md) page.
