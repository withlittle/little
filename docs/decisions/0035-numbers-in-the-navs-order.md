# 0035 — Numbers in the nav's order

- Status: accepted
- Date: 2026-10-10

## Context
Unless a site writes its own `nav`
([0006](./0006-frontmatter-and-config-win.md)), little builds it from the
source's folders: the home page first, then each folder's pages and folders
by title, whatever their case. It compared titles a character at a time, so
a number in a title sorted as text: `1.`, `10.`, `11.`, …, `19.`, `2.`,
`20.`. A project's decisions, numbered in their titles as adr-tools writes
them, like `# 10. Use Redis` in `0010-use-redis.md`, read 1, then 10 to 19,
then 2: in the sidebar, and from page to page, as previous and next pages
([0005](./0005-previous-and-next-pages.md)) and breadcrumbs
([0010](./0010-breadcrumbs.md)) follow the nav. Padding a file's name with
zeros doesn't help, as the nav sorts by title.

Finder, and Windows' File Explorer, sort the numbers in names by their
value, so authors expect `2` before `10`.

## Options considered
- **Leaving it**, for authors to pad the numbers in their titles, like
  `02.`, or write the nav themselves: readers see the zeros, and a site of
  200 pages writes a nav of 200 lines, which little builds one to save.
- **Sorting by file name**, which an author can pad: the nav's order would
  no longer follow what readers see, and `2-…` would still come after
  `10-…`.
- **Comparing numbers in little's own code**, splitting titles into runs of
  digits and of other text: about 30 lines, but they must handle leading
  zeros, and numbers too long for a `u64`, and be a total order, as Rust's
  sort may panic on one that isn't, since 1.81.
- **natord**, a port of `strnatcmp.c`: unchanged since 2015.
- **lexical-sort**, which also sorts a letter with an accent beside the
  letter, like `é` beside `e`: unchanged since 2020.
- **icu_collator**, ICU4X's collator, which sorts as a language does, with
  numbers by their value as an option: the right order in every language,
  but a heavy dependency, with its data, and it needs each page's language,
  which is the theme's to give.
- **alphanumeric-sort**: one function, `compare_str`, with no dependencies,
  last released in 2026.

## Decision
- little compares titles as before, whatever their case, but each run of
  digits in them as a number, by its value, with alphanumeric-sort's
  `compare_str`: `2. Use Postgres` comes before `10. Use Redis`, `Step 9`
  before `Step 10`, and `v1.9` before `v1.10`.
- Everything else is compared as before, a character at a time, so a number
  still comes before a letter. Leading zeros only break ties: `1` comes
  before `01`.
- It orders each folder's pages and folders in the nav little builds, so
  `page.children`, previous and next pages, and breadcrumbs follow it. The
  home page is still first, and a `nav` in `little.yml` is kept as it's
  written.
- little requires alphanumeric-sort 1.5.8 or later, as earlier versions
  didn't always compare in a total order
  ([magiclen/alphanumeric-sort#8](https://github.com/magiclen/alphanumeric-sort/issues/8),
  [#14](https://github.com/magiclen/alphanumeric-sort/issues/14)). Before
  choosing it, a fuzz test of 1.5.8 found no case where it doesn't: every
  triple of 820 short titles, and 20 million random triples of longer ones,
  of digits, zeros, spaces, punctuation, and letters, some accented, some
  Chinese.

## Consequences
- Numbered titles read in order, in the sidebar and from page to page.
- A site whose titles start with numbers of different lengths changes order
  once it's rebuilt, as it should.
- Only ASCII digits are numbers: others, like `٣`, sort as characters. A
  decimal sorts by its parts, as a version does, so `0.5` comes before
  `0.25`.
- Titles still aren't sorted as a language sorts them: `é` comes after `z`.
  icu_collator could do that later, with each page's language.
- little has one more dependency, a small one, with none of its own.
