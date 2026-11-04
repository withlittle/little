---
title: Making a theme
description: A theme's templates, static files, and config, the filters for its URLs, and its search box.
---

# Making a theme

A theme is a folder. It owns 100% of a site's markup, as little bakes in no
HTML:

```text
my-theme/
├── _sass/        # optional: Sass for the stylesheets in _static/ to load
├── _static/      # optional: copied to the site's root, Sass compiled to CSS
├── _templates/   # the templates; default.html is required
└── little.yml    # optional: the theme's config
```

A site uses it with `theme: ./my-theme`. See
[Choosing a theme](choosing-a-theme.md).

little's built-in theme is the
[`theme/`](https://github.com/withlittle/little/tree/main/theme) folder in
little's repository, compiled into little. Its
[`default.html`](https://github.com/withlittle/little/blob/main/theme/_templates/default.html)
and [`404.html`](https://github.com/withlittle/little/blob/main/theme/_templates/404.html)
are complete working examples.

## Templates

Templates are [MiniJinja](https://docs.rs/minijinja) (Jinja2-style). Each
gets the site's config as `site`, with its nav as `site.nav`. It gets the
page as `page`, with its HTML as `page.content`.
[Template data](../reference/template-data.md) lists all they hold. A
`default.html` can be as small as this:

```html
<!doctype html>
<html lang="{{ site.lang | default('en') }}">
<head>
  <meta charset="utf-8">
  <title>{{ page.title }} · {{ site.name }}</title>
  <link rel="stylesheet" href="{{ '/assets/style.css' | url }}">
</head>
<body>
  <nav>
    <ul>
      {% for item in site.nav recursive %}
      <li>
        {% if item.url %}<a href="{{ item.url | safe }}">{{ item.title }}</a>{% else %}{{ item.title }}{% endif %}
        {% if item.children %}<ul>{{ loop(item.children) }}</ul>{% endif %}
      </li>
      {% endfor %}
    </ul>
  </nav>
  <main>{{ page.content | safe }}</main>
</body>
</html>
```

little renders each page with the template for its type. If the theme
doesn't have that template, it uses `default.html`:

| Page type | Pages | Template |
| --------- | ----- | -------- |
| `home` | `docs/README.md` | `home.html`, else `default.html` |
| `section` | a folder's `README.md`, like `docs/guide/README.md` | `section.html`, else `default.html` |
| `page` | every other `.md` file | `page.html`, else `default.html` |
| `404` | the page a host shows for a URL with no page | `404.html`, else none |

A file a [plugin](../plugins/using-plugins.md#pages-in-other-formats) reads
as a page, like a notebook, has the type a `.md` file in its place would.
So `docs/README.ipynb` is the home page.

- Templates can `{% extends %}` and `{% include %}` any others in
  `_templates/`, in subfolders too. So `default.html` can be a layout that
  the rest extend.
- Write the page's HTML with the `safe` filter, as
  `{{ page.content | safe }}`. Otherwise it's escaped.
- MiniJinja escapes each `/` too, as `&#x2f;`. So the built-in theme writes
  the URLs little gives it with `safe` as well, like `{{ item.url | safe }}`.
  The [`url` and `absolute_url` filters](#urls-in-a-theme) need none.
- A page leaves out anything it doesn't have, rather than setting it to
  null. So `{% if page.description %}` and
  `{{ page.description | default("…") }}` both work.

## A 404 page

A 404 page has no content for `default.html` to show. So a theme without a
`404.html` gets no 404 page, and the host shows its own.

A 404 page has just `page.type` and `page.url` (`/404.html`). On a site with
[versions](#a-menu-of-versions), it has `page.versions` too.

## Static files

Everything in `_static/` is copied to the site's root as it is, except
[Sass](sass.md). So `_static/assets/style.css` becomes `/assets/style.css`.

- A file at the same path in the site's source replaces it.
- If a static file would take the place of a page, or of a file little
  writes for [search](../writing/search.md), the build stops and names both.
- A site with [versions](#a-menu-of-versions) has the static files once, at
  its root, for every version.

## A theme's config

A theme's `little.yml` goes over little's defaults, and beneath the site's.
So it sets defaults for what its templates use, like `name`, and the
settings of little's that suit it:

```yaml
# my-theme/little.yml
name: Documentation
search: true
toc_max_depth: 3
```

The built-in theme's
[`little.yml`](https://github.com/withlittle/little/blob/main/theme/little.yml)
is like this. A theme might set:

- `search: true`, for a theme with a [search box](#a-search-box).
- `markdown.highlight`, for the
  [classes its CSS colours](../writing/code.md#choosing-how-code-is-highlighted).
- `markdown.math: tex` and `markdown.mermaid: false`, for a theme that
  renders math, or draws diagrams, with a script.
- `mermaid`, for [its diagrams' look](../writing/diagrams.md#choosing-how-diagrams-look).
- `toc_max_depth`, for the headings its
  [tables of contents](../writing/navigation.md#tables-of-contents) list.
- `theme`, to [build on another theme](choosing-a-theme.md#building-on-another-theme).
- `plugins`, for [plugins](../plugins/using-plugins.md) its sites use.

The site's `little.yml` has the last word on each.

## URLs in a theme

Every URL little gives a theme, like `page.url` and those in `site.nav`,
already has the path of the site's
[`url`](../getting-started/publishing.md#choosing-the-sites-url) before it,
if it has one. For the URLs a theme writes itself, little adds two filters,
`url` and `absolute_url`:

```html
<link rel="stylesheet" href="{{ '/assets/style.css' | url }}">
<link rel="canonical" href="{{ page.url | absolute_url }}">
<meta property="og:image" content="{{ '/og.png' | url | absolute_url }}">
```

- `url` puts the path before a URL from the root of the site. So
  `{{ '/assets/style.css' | url }}` is `/little/assets/style.css` for a site
  at `https://withlittle.github.io/little`, and `/assets/style.css` for one
  at the root of its domain.
- Don't put little's URLs through `url`, or they get the path twice.
- `absolute_url` puts the site's scheme and host before a URL that has the
  path, for a link preview, a canonical link, or a feed. So
  `{{ page.url | absolute_url }}` is
  `https://withlittle.github.io/little/guide/install`.
- For one of the theme's own URLs, use both, as in
  `{{ '/og.png' | url | absolute_url }}`.
- A site without a `url` gets the URL back from `absolute_url` as it is.
- Both leave a URL that isn't from the root of the site, like `https://…`
  or `#…`, as it is.
- A theme's CSS can't use them. So link to fonts and images from a
  stylesheet relative to it, like `url(fonts/inter.woff2)`.

## A search box

A theme with a search box sets `search: true` in its `little.yml`. So
little [indexes](../writing/search.md) each site built with it into
`pagefind/`. The quickest box is [Pagefind](https://pagefind.app/)'s own,
which the built-in theme uses:

```html
<!-- In <head>, before the theme's stylesheet -->
{% if site.search %}
<link rel="stylesheet" href="{{ '/pagefind/pagefind-component-ui.css' | url }}">
<script src="{{ '/pagefind/pagefind-component-ui.js' | url }}" defer></script>
{% endif %}

<!-- Where its button goes -->
{% if site.search %}
<pagefind-modal-trigger></pagefind-modal-trigger>
<pagefind-modal></pagefind-modal>
{% endif %}
```

Show the box only when `site.search` is on, since a site can turn search
off. On a site that [leaves pages out](../writing/search.md#leaving-pages-out),
`site.search` is its settings, not `true`. So test it with
`{% if site.search %}`, as above, not `== true`.

- Load its script with `defer`, not as the module Pagefind's docs show.
  Deferred, it finds the index beside it, even under the path of the
  site's [`url`](../getting-started/publishing.md#choosing-the-sites-url).
- A module looks in `/pagefind/` at the root of the domain, unless
  `<pagefind-config bundle-path="{{ '/pagefind/' | url }}">` tells it where
  the index is.
- Restyle it with Pagefind's `--pf-*` CSS variables, as the built-in theme's
  [`style.css`](https://github.com/withlittle/little/blob/main/theme/_static/assets/style.css)
  does. Pagefind keeps the rest of a theme's CSS out of it.
- For a box of your own, search with Pagefind's
  [JavaScript API](https://pagefind.app/docs/api/), from
  `{{ '/pagefind/pagefind.js' | url }}`.
- Each page is indexed in the language of its `<html lang>`.

## A menu of versions

On a site with [versions](../writing/versions.md), `page.versions` lists
them all, in the order `versions` does. Each has the `url` of this page in
it, if it has one at the same URL, or else of its home page. So a menu can
keep the reader on the page they're reading. `site.version` is the `title`
of the one being built:

```html
{% if site.versions and site.versions | length > 1 %}
<nav aria-label="Versions">
  <ul>
    {% for version in page.versions %}
    <li><a href="{{ version.url | safe }}"{% if version.title == site.version %} aria-current="true"{% endif %}>{{ version.title }}</a></li>
    {% endfor %}
  </ul>
</nav>
{% endif %}
```

- A site without `versions` has no `site.versions`, `site.version`, or
  `page.versions`. A site with one version needs no menu.
- Each `url` already has the path of the site's `url`. So write it with
  `safe`, not through the `url` filter.
- Each version is built as if it were published at its `url`. So the `url`
  filter puts that before the theme's own URLs too: `{{ '/' | url }}` is
  the version's home page, and `{{ '/pagefind/pagefind.js' | url }}` its
  search.
- But the theme's static files are at the root of the site, for every
  version. So `{{ '/assets/style.css' | url }}` is the one stylesheet,
  unless the version has a file of its own at that path.
- Each entry has the keys it's written with in `versions`, like `ref`. But
  little sets its `url`, and its `root`.
- A key of your own, like `released`, is for the theme to show. One of
  little's [settings](../reference/settings.md), like `search`, stops the
  build, except those a version can set as
  [its own](../writing/versions.md#a-versions-own-nav-and-plugins).
- `root` is true for the version at `/`, and false for the others. So
  `page.versions | selectattr("root") | first` is the version at `/`. The
  built-in theme's
  [`default.html`](https://github.com/withlittle/little/blob/main/theme/_templates/default.html)
  uses it for a note on each of the others that links to it, at the same
  page if it has one.

## A copyright notice

`site.year` is the year little builds the site in. So a theme's copyright
notice stays current without editing it each January:

```html
<footer>&copy; {{ site.year }} {{ site.name }}. All rights reserved.</footer>
```

- It's the year in UTC, as CI builds in. So in Sydney it turns over at
  11am on 1 January.
- Every page of a build has the same year. A site that isn't rebuilt keeps
  the year it was last built in.
- With [`SOURCE_DATE_EPOCH`](https://reproducible-builds.org/specs/source-date-epoch/)
  set, as reproducible builds set it, it's that time's year. So the same
  source always builds the same site.
- `SOURCE_DATE_EPOCH` must be a whole number of seconds since 1970, as
  `date +%s` prints it, or the build stops.
- A `year` in `little.yml` replaces it.
- The built-in theme shows no notice. Whether a site has one, and what it
  says, is up to the site.

## Styling a page

[Styling Markdown](styling-markdown.md) shows the HTML little writes for
each part of a page, from alerts to diagrams, for a theme's CSS. It also
shows how a theme loads a script, like KaTeX's, only for a site that needs
it.
