# 0066 — The manual's plain style

- Status: superseded by [0072](./0072-the-manuals-concise-style.md),
  which lets a rewrite cut, and states each fact once
- Date: 2026-11-04

## Context
The manual was hard to read. Its pages were long, and readers had to dig
for what they came for:

- The common case came after the edge cases. The Install page, say, began
  with a table of six archives, and how to unpack one. `cargo install`
  came 83 lines down.
- Sentences chained clause after clause with commas. So one sentence could
  hold several facts.
- Guides held rules that few readers need, like how `url` is checked,
  beside what everyone does.

[0065](./0065-decisions-at-the-root.md) began making little's docs simpler,
by moving these records out of the manual. This record is about how the
manual's pages are written. AGENTS.md already asked for doc comments "in
the manual's plain style", but nothing said what that was.

## Options considered
- **A quick start, or rewriting the pages:** a quick start page, before
  the rest, which leaves the long pages as they are; or rewriting each
  page, so each starts with what most readers need.
- **How a page is ordered:** in the order little does things, with each
  rule where it applies; or the common case first, and edge cases last.
- **How sentences are written:** long, with clauses chained by commas, as
  these records are; or short, one idea each.
- **Checking the style:** with a prose linter, like Vale, in CI; or in
  review, as a linter can't tell one idea from two.

## Decision
The manual, in `docs/`, is written plainly. So is the README, where it's
for people who use little:

- **Lead with the example.** A page starts with what most readers type or
  write: a command, a setting, or some Markdown. The Install page starts
  with `cargo install --locked little-docs`.
- **Write short sentences, one idea each.** Split a chain of clauses into
  sentences. Join them with words like "So", "Then", or "But".
- **Put the common case first.** Edge cases come last, in short bullets.
  Rules a reader looks up, like how `url` is checked, go in Reference, and
  the guide links to them.
- **List what's a list.** What stops a build, what a hook is given, or
  what a command warns of is a list, after a line like "These stop the
  build:".
- **Keep table cells short.** A cell that needs more than a sentence or
  two gets a note below its table.
- **Keep every fact.** A rewrite can reorder and split. But it keeps every
  fact, every heading, and every anchor a link leads to. Code, and what
  little prints, stay exactly as they are.

The pages written in AsciiDoc, Org mode, and reStructuredText follow it, in
their own syntax. Doc comments in `src/`, which the manual publishes at
`/api/`, follow it as they're written or changed. Review checks the style.

## Consequences
- A reader finds the common case at the top of each page, and can stop
  there.
- The manual is about as long as it was. Getting started is a third
  shorter, and some of its rules moved to Reference. Other sections grew a
  little, as short sentences repeat their subjects.
- A reader who needs an edge case may follow a link to Reference.
- AGENTS.md sums the style up, so people and agents who change the manual
  write in it.
- These records and AGENTS.md are for people who work on little. They
  aren't rewritten.
