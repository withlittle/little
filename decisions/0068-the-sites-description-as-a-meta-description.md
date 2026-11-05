# 0068 — The site's description as a meta description

- Status: accepted
- Date: 2026-11-05
- Amends: [0033](./0033-the-built-in-themes-design.md),
  [0039](./0039-a-long-site-name-in-the-built-in-themes-header.md)

## Context
The built-in theme ([0033](./0033-the-built-in-themes-design.md)) wrote
the site's `description`, `site.description`, in its header, beside the
site's name, in muted text after a divider, from 40rem wide, and hid it
below that. [0039](./0039-a-long-site-name-in-the-built-in-themes-header.md)
gave it only the room the name and the buttons left, so it was cut short
with an ellipsis before a long name wrapped.

That was the theme's only use of `site.description`. A page's own
`description`, in its frontmatter, is its `<meta name="description">`, for
search engines and link previews, and a page without one had none.

The manual's description is now little's tagline, "The little static
documentation site generator for big things.", 370px wide in the header's
14px text. It's hidden on a phone, and from 640px wide to about 653px, it's
cut short, as "…for big thi…".

The docs themes 0033 drew on, Starlight, Furo, VitePress, GOV.UK's, and
Material for MkDocs, show the site's name in their header, but no
description. VitePress, like Docusaurus, has a tagline on the home page
instead, in its hero, which its author writes.

Starlight, VitePress, and Material for MkDocs make the site's description
the meta description of each page without one of its own, as
jekyll-seo-tag does, after a page's excerpt; MkDocs' own theme makes it
only the home page's. Google advises a site's description on its home page, and
pages that list others, and each page's own everywhere else, and shows a
page's meta description only where it thinks it describes the page better
than its content does.

## Options considered
- **In the header,** as 0039 has it.
- **On the home page,** below its title, as VitePress's and Docusaurus's
  heroes have a tagline: but the theme would write into the author's page,
  and a home page that starts by saying what the site is, as this manual's
  does, would say it twice.
- **Out of the header, and unused by the built-in theme:** other themes
  would still get `site.description`, but the `little.yml` `little init`
  writes, which has it commented out
  ([0057](./0057-starting-a-site-with-little-init.md)), and "Your first
  site", which sets it, would suggest a setting that does nothing.
- **Out of the header, and the meta description of a page without its
  own:** of the home page only, as in MkDocs' own theme, and as Google
  advises, or of each page without its own, as in Starlight, VitePress,
  and Material for MkDocs.

## Decision
- The built-in theme's header has no description: the site's name, with
  the menu of versions, on a site with them, at its left, and its buttons
  at its right. The `.description` span, and its CSS, are gone.
- The site's `description` is the `<meta name="description">` of each page
  without a `description` of its own, as
  `page.description or site.description`, the 404 page too, as in
  Starlight, VitePress, and Material for MkDocs, so a preview of a link to
  any page says what the site is. A page with its own keeps it, and a page
  with neither has none.
- `description` is still one of the built-in theme's settings, for that,
  so `little init` and "Your first site" still suggest it. Other themes get
  it as `site.description`, as before.
- It isn't a breaking change: how the built-in theme looks isn't public
  ([0051](./0051-semantic-versioning-and-conventional-commits.md)), and
  the setting stays, with a new use. It's a feature, so it raises little's
  minor version ([0058](./0058-features-raise-the-minor-version.md)).

## Consequences
- The header is the same with a description as without: from 40rem, the
  room the description took is empty, between the name and the buttons.
- A long site name has all the room the buttons leave, at every width, so
  it wraps onto a second line only where it doesn't fit beside them, and
  0039's divider beside a long name, with none of the description's text,
  is gone.
- Readers no longer see the description on the page. Search engines and
  link previews do, on each page without its own: in this manual, the
  pages of little's API, which the `rustdoc` plugin adds, and the 404 page.
- Google may show a snippet of a page's content in place of the site's
  description, where it describes the page less well, so a page's own is
  still best.
- A page on a site with a description can't leave its meta description
  out: `description: false`, or `""`, gets the site's.
