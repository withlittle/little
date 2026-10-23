# 0053 — little's decisions are in its manual

- Status: accepted; amended by
  [0061](./0061-leaving-pages-out-of-search.md), with which a page can leave
  itself out of search
- Date: 2026-10-18
- Amends: [0032](./0032-docs-are-the-manual.md)

## Context
These records were in `decisions/`, at the root of the repository, beside
`VISION.md` and `AGENTS.md`, since `docs/` is the manual, for people who
use little, and meta docs stay out of it
([0032](./0032-docs-are-the-manual.md)). But a record says why little works
as it does, which people who use little ask too: why math is MathML, why a
plugin runs in a sandbox, or why `2.` sorts before `10.` in the nav. The
manual linked to four of them, on GitHub, where a reader leaves the site,
search doesn't find them, and nothing checks their links, to each other or
to other sites.

## Options considered
- **Where the records are:**
  - at the root, with a page in the manual that links to them on GitHub:
    nothing moves, but a reader still leaves the manual to read one, and
    their links still aren't checked;
  - at the root, added to the manual by a plugin's `files` hook, which
    reads their folder: a plugin to keep, for a folder that can move;
  - or in the manual, in `docs/decisions/`, where each is a page, built,
    searched, and checked as any other, and still read on GitHub, in its
    folder.
- **How a reader finds them:** each in `nav`, so each has breadcrumbs, and
  previous and next pages, with a line in `little.yml` for each; or a page
  that lists them, alone in `nav`, as little's API is reached from `/api/`.

## Decision
- The records are in `docs/decisions/`, so each is a page of the manual, at
  the URL of its file, like `/decisions/0001-language-rust`.
- `docs/decisions/README.md`, "Design decisions", says what they are, and
  lists each, by its number and title. It alone is in `nav`, under
  Reference, and the manual's home page links to it; a new record goes in
  its list, not in `nav`.
- A record links to another by its file, as before, and to little's files
  outside `docs/`, like `VISION.md`, on GitHub, as the manual's pages do.
  The manual's pages link to a record by its file too, so the link is
  checked.
- `VISION.md`, `AGENTS.md`, and `RELEASING.md` stay at the root: they're for
  people who work on little.

## Consequences
- A reader reads why little works as it does in the manual, and finds it
  with search, and `little build --strict` checks each record's links.
- A record isn't in the sidebar, and has no breadcrumbs, or previous and
  next pages: a reader goes from one to another by their links, or by the
  list.
- A page can't leave itself out of search yet, so search finds a record
  whose decision a later one has amended; its status, at its top, names
  the later one, and the list says the rest of the manual describes little
  as it is.
- Links to `decisions/` on GitHub from outside the repository, in issues
  and pull requests, say, no longer lead to a record.
