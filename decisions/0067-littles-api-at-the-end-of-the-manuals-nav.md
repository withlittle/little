# 0067 — little's API at the end of the manual's nav

- Status: accepted
- Date: 2026-11-05
- Amends: [0044](./0044-a-plugin-for-rusts-doc-comments.md),
  [0049](./0049-an-api-references-url-and-title.md)

## Context
The manual documents little's own code with the `rustdoc` plugin, at
`/api/`, titled "little's API", for those who work on little, as the
example on the page "Rust API reference"
([0044](./0044-a-plugin-for-rusts-doc-comments.md)). 0044 listed `/api/` in
the manual's `nav` under that page, in the Plugins section, so the manual
read in the order it did.

But Plugins is where people who use little find out which plugins there
are, how to use them, and how to write their own. little's API is none of
those: it's how little's code works, for those who work on it. Yet a reader
of Plugins meets it between the pages on Rust's and JavaScript's API
references, as if it were a plugin's page. The manual groups its pages by
what a reader is trying to do ([0032](./0032-docs-are-the-manual.md)).

And "little's API" is the only title in the nav that names little, though
every page of the manual is little's.

## Options considered
- **Under the page "Rust API reference":** as 0044 has it.
- **A link of its own, at the end of the nav, after Reference:** apart from
  the sections for people who use little, as Home is apart at its top.
- **A section of its own, for those who work on little:** a section of one
  page, in a manual for people who use little
  ([0065](./0065-decisions-at-the-root.md)).
- **Out of the nav:** reached only from the page "Rust API reference", and
  search, with no `page.prev` or `page.next`.
- **Under Reference:** which 0044 turned down, as there it looks like an
  API a reader calls.

## Decision
- `/api/` is a link of its own, "API", at the end of the manual's `nav`,
  after Reference, in no section.
- It's titled "API", not "little's API", as no other title in the nav
  names little. `rustdoc`'s `title` is "API" too, so the tab of `/api/`
  still says what its nav does, as
  [0049](./0049-an-api-references-url-and-title.md) has it.
- The page "Rust API reference" still links to it, as the plugin's
  example. It's still the only one of the plugin's pages in `nav`, and
  links to the rest.

## Consequences
- Plugins lists only plugins, and how to use and write them.
- "Rust API reference" is followed by "JavaScript API reference", and
  "Plugin hooks" by "API", which is the manual's last page.
- `/api/`'s breadcrumbs are only Home. The pages of little's modules still
  have none, as they aren't in `nav`.
- The built-in theme puts a group's gap above a page at the nav's top level
  that follows a group. Without it, little's API looked like Reference's
  last page.
- Its URLs don't change, so links to them still lead there.
- The home page's table of the manual's sections still doesn't list it.
