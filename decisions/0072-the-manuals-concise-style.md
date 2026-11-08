# 0072 — The manual's concise style

- Status: accepted
- Date: 2026-11-08
- Supersedes: [0066](./0066-the-manuals-plain-style.md)

## Context
[0066](./0066-the-manuals-plain-style.md) set a plain style for the
manual. Its pages now lead with what most readers type, but they're still
long and repetitive, and they don't flow. 0066's rewrite left the manual as
long as it was, 33,569 words before and 33,612 after, and it has grown to
over 35,000 words on 34 pages since. Two of 0066's rules are part of the
cause:

- **"Keep every fact."** A rewrite could reorder and split, but not cut,
  so every edge case and detail stayed where it was.
- **"Split a chain of clauses … with 'So', 'Then', or 'But'."** About 200
  sentences open with one of them. Short sentences repeat their subjects,
  and a page reads as a list of facts rather than an explanation.

The rest is what no rule stopped:

- **Facts are stated more than once.** The built-in theme's settings have
  a table in Configuring and another in Settings. The levels a table of
  contents lists are in both Navigation and Settings, and so are `host`
  and `port` in Commands. Each of the nine plugins' pages has the same
  "Getting it" section, which says how little downloads a plugin, as
  Using plugins does.
- **The manual teaches the tools around little.** Versions had a section
  on releasing a version, and another on cloning a repository's whole
  history in CI, with a step for `.github/workflows/docs.yml`, as if every
  reader used GitHub. Readers already know git, their CI service, and how
  they release.
- **It says what little doesn't do.** Nine pages have a "Limits" section,
  of what little doesn't do yet. Guides also explain how little works
  inside, which browsers the built-in theme's menus need, and rules that
  little's errors already explain, with their fixes.

## Options considered
- **Trim under 0066:** cut the longest pages, but keep its rules. They're
  what made the pages long, so the length would come back.
- **Move what's cut to Reference:** guides get shorter, but the manual
  doesn't, and a reader meets the same rules there.
- **Restructure by Diátaxis:** tutorials, how-to guides, reference, and
  explanation, each a section of its own. That's more structure than a
  manual this size needs, and its guides and Reference already split tasks
  from lookups.
- **New rules, tried on one page:** replace 0066's rules with ones that
  let a rewrite cut, try them on one page, then rewrite the rest a section
  at a time.

## Decision
New rules replace 0066's, so that readers find what they need, do it,
and stop reading. They're for the manual, in `docs/`, the README where
it's for people who use little, and doc comments in `src/`. The pages in
AsciiDoc, Org mode, reStructuredText, and the notebook follow them in
their own syntax.

- **One home for each fact.** State it once, on the page a reader would
  look for it, and link to it from anywhere else. Guides teach a task;
  Reference holds the rules a reader looks up. A pointer elsewhere is a
  line and a link, not a second copy.
- **Lead with the task.** Open each page with what most readers type or
  write: a command, a setting, or some Markdown. Put the common case
  first, and end a guide when its task is done.
- **Document little, not the tools around it.** Assume readers know git,
  their CI service, and how they release. Don't assume they use GitHub:
  what's only for one host, like a GitHub Pages workflow, goes in a
  section of its own. Saying little reads Markdown as GitHub does is fine.
- **Say what little does.** No "Limits" sections, or lists of what it
  doesn't do yet. Leave out how it works inside, browser support, and
  rules whose error message says how to fix them.
- **Write natural sentences.** Address the reader as "you". Join ideas
  that belong together, and vary the length. Don't open sentences with
  "So", "Then", or "But" by habit, or compare everything with "as X does".
- **Lists and tables are for lists and lookups**, not for paragraphs of
  rules. Keep table cells short.
- **A rewrite may cut.** Keep each heading a link leads to, or update the
  links, and keep pages' file names, as little has no redirects. Code,
  and what little prints, stay as they are.

The manual is rewritten a section per pull request, starting with
Versions. Review checks the rules.

## Consequences
- Pages get shorter. Versions went from 2,064 words to 504, with four
  sections: listing versions, the menu of versions, what's in a version,
  and a version's own nav and plugins.
- A rule only an error explains leaves the manual, so little's errors must
  say what's wrong and how to fix it.
- A reader who wants to know how little works inside reads these records,
  or its code. One who's new to git or CI learns them elsewhere.
- Review looks for a second copy of a fact, and a change to the manual
  links to a fact's page rather than restating it.
- Until the rewrite is done, some pages follow 0066's rules. The other
  eight "Limits" sections go as their sections are rewritten.
- These records and AGENTS.md are for people who work on little. They
  keep their own style.
