# 0059 — The menu of versions as a popover

- Status: accepted
- Date: 2026-10-21
- Amends: [0033](./0033-the-built-in-themes-design.md),
  [0034](./0034-the-built-in-themes-script.md),
  [0040](./0040-versions-from-git-refs.md)

## Context
The built-in theme's menu of versions
([0040](./0040-versions-from-git-refs.md)) was a `<details>`, beside the
site's name, whose `<summary>` showed the version being read, and opened a
list of them all, below it, with `position: absolute`. A `<details>` stays
open until its summary is clicked again, so the theme's script
([0034](./0034-the-built-in-themes-script.md)) closed it on a click
elsewhere, or Escape, in 12 lines; without the script, nothing did.

The drawer is a popover ([0033](./0033-the-built-in-themes-design.md)),
which the browser closes on a click elsewhere, or Escape, with or without a
script, and the list could be one too. But a popover is drawn over the
page, in the top layer, not where its button is, and only CSS anchor
positioning places it beside an element without a script: in every major
browser since January 2026, when Firefox 147 had it too, later than the
2024 that 0033 supports.

## Options considered
- **Keeping the `<details>`, and the script:** the list opens below its
  summary in every browser 0033 supports, but closes on a click elsewhere
  only with the script.
- **A popover, anchored below its button where the browser can, and in the
  middle of the window, where the browser puts a popover, where it
  can't:** no script, and a browser without anchor positioning still opens
  and closes it, with the list away from its button.
- **A popover, placed by a script where the browser can't anchor it:** more
  script than the 12 lines it would save.

## Decision
- The menu is a `<button popovertarget>`, showing the version being read,
  and a `<ul popover>` of them all, in a `<div class="version-menu">`, as
  the drawer is a `<nav popover>` that the menu button opens. The browser
  opens and closes it, on a click elsewhere, or Escape, and gives the focus
  back to the button; the script no longer touches it.
- Where the browser has anchor positioning, as
  `@supports (position-area: bottom)` finds, the list is anchored below the
  button, with `position-area: bottom span-right`, and is at least as wide
  as it, with `anchor-size(width)`, as it was. The button is named with
  `anchor-name`, as Chrome anchors a popover to the button that opens it
  only from 133.
- Where the browser hasn't, the list opens in the middle of the window, as
  a popover does, over the page.

## Consequences
- The script is 16 lines shorter, and the stylesheet 5 longer.
- In Chrome, Firefox, and Safari's WebKit, the menu opens where it did, and
  in Chrome, looks as it did: to the pixel, closed, and, open, but for a
  shade in its shadow, too faint to see.
- Without a script, a click elsewhere, or Escape, closes the menu, which it
  didn't.
- Its button tells a screen reader whether the list is expanded, as the
  summary did.
- In a browser without anchor positioning, like Safari before 26, or
  Firefox before 147, the list opens in the middle of the window, not below
  its button, and closes as anywhere else. This amends 0033, whose theme
  has no fallbacks for older browsers: anchor positioning only places the
  list, as `@starting-style` only slides the drawer in.
- A theme that builds on the built-in one, and styles its
  `.version-menu summary`, styles `.version-menu button` instead; how the
  built-in theme looks isn't public
  ([0051](./0051-semantic-versioning-and-conventional-commits.md)).
