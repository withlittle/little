# 0065 — little's decisions are at the root of its repository

- Status: accepted
- Date: 2026-11-04
- Supersedes: [0053](./0053-decisions-in-the-manual.md)
- Amends: [0032](./0032-docs-are-the-manual.md),
  [0061](./0061-leaving-pages-out-of-search.md)

## Context
Since [0053](./0053-decisions-in-the-manual.md), these records have been in
the manual, in `docs/decisions/`, each a page, so a reader could read why
little works as it does without leaving the site, search found them, and
`little build --strict` checked their links. Since
[0061](./0061-leaving-pages-out-of-search.md), search leaves them out, as
each says what was decided when it was made, not how little works now. With
their list, they're 65 of the manual's 117 pages, for people who work on
little more than for people who use it, and each of the manual's pages on a
plugin starts by sending its reader to them. little's docs are to be
simpler, and this is where that starts.

## Options considered
- **Where the records are:** in the manual, as 0053 has them; or in
  `decisions/`, at the root of the repository, beside `VISION.md`,
  `AGENTS.md`, and `RELEASING.md`, as before 0053, where they're read on
  GitHub, and nothing checks their links.
- **What the manual says of them:** it links to them on GitHub, as it does
  to little's other files outside `docs/`, though a link into the
  repository isn't checked
  ([0020](./0020-links-to-a-pages-file-in-its-repo.md)); or nothing, so it
  says how little works, not why.
- **Their list:** `decisions/README.md`, which GitHub shows below the
  folder's files; or AGENTS.md's alone, as before 0053.

## Decision
- The records are in `decisions/`, at the root of the repository, for
  people who work on little: `docs/` is the manual, for people who use it,
  and meta docs stay out of it, as [0032](./0032-docs-are-the-manual.md)
  had it.
- `decisions/README.md` lists each, by its number and title, and AGENTS.md
  sums each up; a new record goes in both.
- A record links to another by its file, and to little's other files by
  their paths, like `../VISION.md` or `../docs/writing/math.md`, so it
  reads on GitHub as the files are at its commit.
- The manual doesn't link to the records, or list them: its pages on
  plugins no longer say which records explain them, and its home page no
  longer offers why little works as it does. The plugins' READMEs, for
  people who work on them, still link to theirs.
- The manual's `little.yml` no longer lists the records in `nav`, or leaves
  them out of search.

## Consequences
- The manual has less than half the pages it had, each for people who use
  little.
- Nothing checks a record's links, as before 0053: one to a file that
  moves breaks quietly.
- Once the manual is published again, its URLs under `/decisions/` lead
  nowhere, as little has no redirects. Links to `decisions/` on GitHub,
  from before 0053, lead to their records again, and those to
  `docs/decisions/` don't.
- The manual no longer uses `search.exclude`, which its Search page still
  documents.
