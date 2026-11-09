# 0073 — `AGENTS.md` points to what's written elsewhere

- Status: accepted
- Date: 2026-11-09
- Amends: [0065](./0065-decisions-at-the-root.md)

## Context
`AGENTS.md` says how to work on little, and coding agents read all of it
at the start of every session. It had grown to 1,042 lines, and most of
them said again what other files say:

- 598 lines summed up each of these records, as
  [0065](./0065-decisions-at-the-root.md) asked, and a record that changed
  another meant rewriting the other's summary too.
- 243 lines described each module in `src/`, each plugin, and each
  workflow, which their `//!` comments, READMEs, and comments, with
  `RELEASING.md`, already describe.
- Most of its 67 lines of commands gave the same three for each plugin,
  which each plugin's README gives.

Claude Code, which reads `AGENTS.md`, recommends a file of instructions
under 200 lines: a longer one costs context in every session, and is
followed less closely. A fact written in two places has to be kept the
same in both, too, and the manual gives each fact one home, as
[0072](./0072-the-manuals-concise-style.md) sets.

## Options considered
- **Keep the summaries**, so an agent knows each decision without opening
  its record, at the cost of their length in every session, and of keeping
  each in step with its record.
- **Shorten each summary to a line**, which `decisions/README.md`'s titles
  already are.
- **Split it up**, into files an agent reads only when it works on what
  they're about: `AGENTS.md` files in the folders, though one in `docs/`
  would be built as a page of the manual, or Claude Code's
  `.claude/rules/`, which other agents don't read. Each would still repeat
  what's written elsewhere.
- **Point to the records**, and to the other files that say what little
  is, keeping in `AGENTS.md` only how to work on it.

## Decision
- `AGENTS.md` holds how to work on little: its commands, core principles,
  working rules, conventions, and how to write the manual. It points to
  the rest, rather than repeating it.
- It no longer sums up each record. `decisions/README.md` lists them, and
  `AGENTS.md` asks that, before you change or add to what little does, you
  read the records about it, and any later record each one's `Status:`
  names.
- A short map of the repository takes the place of its description of
  each module and plugin. It points to each module's `//!`, each plugin's
  README, the workflows' comments, and `RELEASING.md`, and keeps the few
  rules an agent needs before it opens them, like listing each of the
  built-in theme's files in `builtin`. What only `AGENTS.md` said, of
  `src/testing.rs`, moved to its `//!`.
- Its commands give a plugin's once, as a pattern, and leave the plugins
  the manual uses to its `little.yml`, and the scopes of commits to
  `committed.toml`.

## Consequences
- `AGENTS.md` is under 200 lines, and a new record, module, or plugin
  doesn't add to it.
- An agent has to open a record to know what it decided. `AGENTS.md` says
  when, but nothing checks that it does.
- A module's `//!`, and a plugin's README, are its only description, so
  they change with it.
