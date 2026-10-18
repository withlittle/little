# 0039 — A long site name in the built-in theme's header

- Status: accepted; amended by [0040](./0040-versions-from-git-refs.md),
  which keeps the name to one line, cut short with an ellipsis, on a
  phone, where the menu of versions goes below it
- Date: 2026-10-11
- Amends: [0033](./0033-the-built-in-themes-design.md)

## Context
The built-in theme ([0033](./0033-the-built-in-themes-design.md)) writes
the site's name, `site.name`, at the left of its header, one row 3.75rem
tall, after the menu button below 60rem, and before the buttons for search
and the theme at its right; and at the top of the drawer, beside its close
button. The name was `white-space: nowrap`, so it never wrapped.

On a site named "Quantum Computing Applications", 270px wide in the
header's bold 17px:
- the header's row was wider than the screen, by 18px at 390px wide, 48px
  at 360px, and 88px at 320px, which pushed the buttons at its right off
  it;
- the menu button, and the drawer's close button, were squeezed from
  2.5rem to 22px, the width of their icons, as a flex item can shrink to
  its content;
- and the close button was pushed out of the drawer, which scrolled
  sideways, by 1px at 390px wide, 15px at 360px, and 49px at 320px.

This manual's name, "little", fits.

## Options considered
- **How a long name fits:**
  - on one line, cut short with an ellipsis, as Material for MkDocs does:
    the header looks the same whatever the name, but a phone shows little of
    a long one;
  - on one line, clipped where the buttons start, as Starlight does: nothing
    shows that it's cut;
  - in smaller text on a phone: it fits only a name a little too long, and
    needs a size for each width;
  - or on two lines, then cut short with an ellipsis: a name of a few words
    is whole, even on a phone, and two lines fit in the header, where a
    third wouldn't.

## Decision
- The site's name, `.brand`, in the header and the drawer, wraps onto a
  second line when it's too long for one, and is cut short with an
  ellipsis when it's too long for two, with `-webkit-line-clamp: 2`, which
  needs `display: -webkit-box` and `overflow: hidden`:
  - `text-wrap: balance` makes its two lines about as long as each other,
    so a word isn't left on its own on the second;
  - `overflow-wrap: anywhere` breaks a name without spaces where it has to,
    rather than run it under the buttons;
  - its `line-height` is 1.3, tighter than the text's 1.6, so its two lines
    read as one name; at 1.25, `overflow: hidden` clipped the ring of an
    "Å";
  - and it has no padding above or below it, which would show the top of a
    third line.
- The site's description, beside the name from 40rem, is `flex: 1 1 0`, so
  it has only the room the name leaves, and is cut short first: the name
  keeps to one line while the description has room to give.
- An icon button, like the menu button or the drawer's close button, is
  `flex-shrink: 0`, so it keeps its 2.5rem beside a long name.

## Consequences
- On the site named "Quantum Computing Applications", at 320px, 360px,
  390px, and 430px wide, with and without a link to its repository, and
  with the drawer open, nothing runs off the screen, every button is 40px
  wide, and the close button is in the drawer. The name takes two lines,
  and is whole from 360px wide; at 320px, it's "Quantum Computing…".
- A name that fits on one line looks as it did: from 500px wide, the header
  is the same to the pixel, but for the edge of the name's last letter,
  which its negative letter-spacing takes a fraction of a pixel past its
  box, where `overflow: hidden` now clips it, too little to see.
- A name too long for two lines is cut short on the screen, but a screen
  reader reads it whole, and it's whole in each page's `<title>`.
- On a site with a description, between about 40rem and 60rem wide, a name
  too long for one line, about 45 characters there, leaves the
  description's divider, its border, beside it, with none of its text.
- `-webkit-line-clamp`, despite its prefix, is in every browser, and
  `text-wrap: balance` in every browser 0033 supports, from 2024; without
  it, the lines aren't balanced.
