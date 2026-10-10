# 0034 — The built-in theme's script

- Status: accepted
- Date: 2026-10-10
- Amends: [0033](./0033-the-built-in-themes-design.md)

## Context
The built-in theme needs no JavaScript
([0033](./0033-the-built-in-themes-design.md)): its colours follow the
reader's system, its drawer is a popover, and the table of contents above
the page is a `<details>`. 0033's Consequences list what that leaves it
without, for now:
- a reader can't choose light or dark for a site: it's as their system is;
- if a window widens past 60rem while the drawer is open, the drawer stays
  open, over the page, until it's closed;
- the table of contents above the page stays open after a link in it is
  followed.

The docs themes 0033 learned from let a reader choose: Starlight, VitePress,
and Furo each have a control for light and dark, and remember it, for a
reader who wants dark docs on a light system, or light ones in the sun. Most
also put a button on each code block to copy it, and mark the heading the
reader is at in the table of contents beside the page.

CSS that every browser has can't do these alone: a choice that lasts from
page to page needs storage, copying needs the clipboard, and closing a
popover, or a `<details>`, when something else happens needs a script.

## Options considered
- **Following the system only**, as 0033 has it, with no script: nothing to
  load, or to fail, but a reader who wants dark docs on a light system, or
  light ones on a dark system, can't have them. It was turned down for a
  toggle, as on Starlight, VitePress, and Furo.
- **A menu of three**, light, dark, and the system's, as Starlight has, or a
  button that cycles through them, as Furo has: a reader sees they can go
  back to the system's, but a menu is more to build, and in a cycle of three,
  a click doesn't always change the colours.
- **A button of two**, light and dark, that forgets a choice that's what the
  system shows, as VitePress has: as simple as a switch, and a reader goes
  back to the system's by choosing what it shows.
- **A checkbox, in CSS**, with `:root:has(#dark:checked)`: no script, but
  nothing remembers it, so each page opens in the system's colours again.
- **Copy buttons in the HTML little writes**: every theme would have them,
  whether it wanted them or not, and without a script, each would do
  nothing.
- **`IntersectionObserver`**, to find where the reader is: it says when a
  heading crosses a line, but the heading the reader is at is the last above
  the top of the window, which may have crossed it long ago, so it would
  still need to measure.

## Decision
- The built-in theme has one script, `assets/script.js`, of about 5 KB,
  loaded with `defer`, which only adds to a page: every page works without
  it, as 0033 has it. Each part of it looks for the markup it works on, and
  does nothing where that isn't there.
- **Before the page is drawn**, a few lines inline at the top of `<head>`,
  before the stylesheets, add the class `js` to `<html>`, and set
  `data-theme` on it to the reader's choice, if they've made one. They're
  inline so they run before the first paint, which a deferred script
  doesn't, so a reader who chose dark never sees the page flash light, and
  before the stylesheets so they don't wait for them. `js` hides what needs
  the script where it can't run.
- **Light or dark:** a button in the header, between search and the
  repository, switches the page between light and dark, as 0033's tokens
  are:
  - `data-theme`, `light` or `dark`, sets `color-scheme` on `:root`, so
    every token's `light-dark()` pair switches with it, and so do the
    browser's own controls and scrollbars. Without it,
    `color-scheme: light dark` follows the system.
  - The choice is kept in `localStorage`, under `little-theme`, as `light`
    or `dark`. Choosing what the system shows forgets it, as VitePress does,
    so the page follows the system again, even when the system changes, like
    at night. So one button of two has three states: light, dark, and the
    system's. Where the browser blocks storage, a choice lasts only as long
    as the page.
  - It's a `<button>` named "Dark mode", with `aria-pressed` `true` when the
    page is dark, as ARIA's pattern for a toggle button has it: its name
    stays the same, and its state changes, when the system changes too. It
    shows a sun in light and a moon in dark, hidden from screen readers.
    Without the script, it's hidden, as it would do nothing.
- **The drawer:** when the window widens to 60rem, where the nav is a column
  beside the page, the script closes the drawer, if it's open, with
  `hidePopover()`. An open popover is in the top layer, over the page,
  whatever its CSS, so it would otherwise stay there.
- **The table of contents above the page** folds up again once a link in it
  is picked, so it doesn't fill the top of the page. Following the link
  moves where the keyboard goes next to its heading, so a keyboard user
  carries on from there.
- **Copying code:** where the browser has a clipboard, as it has on HTTPS,
  and on `localhost` and `127.0.0.1`, the script puts each code block in a
  page's content in a `<div class="code-block">`, with a button in its
  corner that copies the block's text, without the line break at its end.
  - It's a `<button>` named "Copy code", shown when the block is hovered or
    the button has focus from the keyboard, and always where the reader
    can't hover, as on a touch screen.
  - Once the code is copied, the button's icon is a tick for two seconds,
    and "Copied" is announced through a `role="status"` element. That's on
    the page from the start, as screen readers announce a change only to
    one they already know.
  - A notebook's output, a block in `output`, isn't code to copy, so it has
    none. Print hides it.
- **Where the reader is:** in the table of contents beside the page, the
  script marks the link to the heading the reader is at with
  `data-current`, which the theme shows in the accent colour, with the bar
  beside it. Forced colours show every link's transparent bar, so there the
  others' bars are drawn in `Canvas`, and the current link's in
  `Highlight`: it's marked by its bar, not by colour alone. It's the last
  heading whose top is above the top of the window, below the header, or
  the last of all at the bottom of the page, as the last headings may never
  reach the top. It measures them at most once a frame, as the page
  scrolls.
  - It's `data-current`, not `aria-current`: it's where the window is, not
    where a screen reader is reading, so it's for the eye, and would
    otherwise change, unasked for, as the page scrolls.
- **Why the script adds copy buttons, not little:** the theme owns all the
  markup ([0002](./0002-theme-structure.md)), so little doesn't put buttons
  in the HTML it writes, where every theme would have them, wanted or not.
  A template gets a page's content as HTML, so it can't add a button to each
  block in it, and a button to copy needs a script anyway: without one, it
  would do nothing.
- **Browsers:** what the script uses is Baseline 2024, as the theme's CSS
  is. `hidePopover()` and `:popover-open` have been in every major browser
  since April 2024. It checks `:popover-open` rather than the `popover`
  property, `HTMLElement.popover`, which is Baseline only from January
  2025, as Safari on iOS added it in 18.3. Everything else it uses is older:
  the clipboard's `writeText()`, and `matchMedia()`'s `change` event, since
  2020, `Array.prototype.at()` since 2022, and media queries' range syntax,
  like `(width >= 60rem)`, since 2023.

## Consequences
- A reader can choose light or dark, copy code with a click, and see where
  they are in a long page. Three of 0033's limits are gone: a reader can
  choose light or dark for a site, the drawer closes when the window widens
  past 60rem, and the table of contents above the page folds up once a link
  in it is followed.
- Every page loads one more file, which doesn't hold it up, as it's
  deferred, and runs a few lines before it's drawn.
- A choice is kept by the browser for the site's origin, so sites published
  at the same one, like project sites under one `github.io` address, share
  it.
- If `assets/script.js` doesn't load, the toggle is shown, but does nothing.
  A Content Security Policy that forbids inline scripts stops the lines at
  the top of `<head>`, so the toggle is hidden, and the page follows the
  system, but `assets/script.js` still adds the rest.
- A site served over plain HTTP, but for `localhost` and `127.0.0.1`, has no
  clipboard, so no copy buttons: `little serve --host 0.0.0.0` opened on a
  phone by the computer's address, say.
- A theme that builds on the built-in one can replace `assets/script.js`,
  like any static file, or leave it out of its own `default.html`. Its CSS
  meets each code block in a `<div class="code-block">` once the script has
  run, so `.content > pre` no longer matches it.
