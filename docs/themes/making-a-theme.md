---
title: Making a theme
description: A theme's templates, static files, and config, the filters for its URLs, and its search box.
---

# Making a theme

A theme is a folder that writes all of a site's HTML:

```text
my-theme/
├── _sass/        # optional: Sass for the stylesheets in _static/ to load
├── _static/      # optional: copied to the site's root, Sass compiled to CSS
├── _templates/   # the templates; default.html is required
└── little.yml    # optional: the theme's config
```

Use it in a site with `theme: ./my-theme`, or
[build it on another theme](choosing-a-theme.md#building-on-another-theme)
to change only part of that one. The built-in theme's
[`default.html`](https://github.com/withlittle/little/blob/main/theme/_templates/default.html)
and [`404.html`](https://github.com/withlittle/little/blob/main/theme/_templates/404.html)
are complete examples.

## Templates

Templates are [MiniJinja](https://docs.rs/minijinja), which is like Jinja2.
Each gets the site's config as `site`, with its nav as `site.nav`, and the
page as `page`, with its HTML as `page.content`.
[Template data](../reference/template-data.md) lists everything they get. A
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

little renders each page with the template for its type:

| Page type | Pages | Template |
| --------- | ----- | -------- |
| `home` | `docs/README.md` | `home.html`, else `default.html` |
| `section` | a folder's `README.md`, like `docs/guide/README.md` | `section.html`, else `default.html` |
| `page` | every other `.md` file | `page.html`, else `default.html` |
| `404` | the page a host shows for a URL with no page | `404.html`, else none |

A file a [plugin](../plugins/using-plugins.md#pages-in-other-formats) reads
as a page, like a notebook, has the type a `.md` file in its place would,
so `docs/README.ipynb` is the home page. A theme without a `404.html` has
no 404 page, so the host shows its own.

Templates can `{% extends %}` and `{% include %}` each other, in subfolders
of `_templates/` too, so `default.html` can be a layout that the others
extend. Write the page's HTML as `{{ page.content | safe }}`, or it's
escaped. MiniJinja escapes each `/` too, as `&#x2f;`, so write the URLs
little gives you with `safe` as well, like `{{ item.url | safe }}`. The
[`url` and `absolute_url` filters](#urls-in-a-theme) need none.

## Static files

Everything in `_static/` is copied to the site's root as it is, except
[Sass](sass.md), which is compiled to CSS, so `_static/assets/style.css`
becomes `/assets/style.css`. A site can
[replace any of them](../writing/pages-and-files.md#images-and-other-files).
[Styling Markdown](styling-markdown.md) shows the HTML your stylesheets
style.

## A theme's config

A theme's `little.yml` sets defaults beneath the site's own, for what its
templates use, like `name`, and for those of little's
[settings](../reference/settings.md) that suit it:

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
  [renders math, or draws diagrams, with a script](styling-markdown.md#loading-a-script-a-site-needs).
- `mermaid`, for [its diagrams' look](../writing/diagrams.md#choosing-how-diagrams-look).
- `toc_max_depth`, for the headings its
  [tables of contents](../writing/navigation.md#tables-of-contents) list.
- `theme`, to [build on another theme](choosing-a-theme.md#building-on-another-theme).
- `plugins`, for [plugins](../plugins/using-plugins.md) its sites use.

## URLs in a theme

Every URL little gives a theme, like `page.url` and those in `site.nav`,
already has the path of the site's
[`url`](../getting-started/publishing.md#choosing-the-sites-url) before it.
For the URLs a theme writes itself, little adds two filters, `url` and
`absolute_url`:

```html
<link rel="stylesheet" href="{{ '/assets/style.css' | url }}">
<link rel="canonical" href="{{ page.url | absolute_url }}">
<meta property="og:image" content="{{ '/og.png' | url | absolute_url }}">
```

`url` puts the path before a URL from the root of the site, so
`{{ '/assets/style.css' | url }}` is `/little/assets/style.css` for a site
at `https://withlittle.github.io/little`. Don't put little's own URLs
through it, or they get the path twice.

`absolute_url` puts the site's scheme and host before a URL that has the
path, for a link preview, a canonical link, or a feed, so
`{{ page.url | absolute_url }}` is
`https://withlittle.github.io/little/guide/install`. On a site without a
`url`, it gives the URL back as it is. For one of the theme's own URLs,
like `/og.png`, use both filters. Both leave a URL that isn't from the root
of the site, like `https://…` or `#…`, as it is.

On a site with [versions](../writing/versions.md), each version is built as
if it were published at its `url`, so `{{ '/' | url }}` is the version's
home page, and `{{ '/pagefind/pagefind.js' | url }}` its search. The
theme's static files are at the root of the site, for every version, so
`{{ '/assets/style.css' | url }}` is the same stylesheet in each, unless a
version has a file of its own at that path.

A theme's CSS can't use the filters, so link to fonts and images from a
stylesheet relative to it, like `url(fonts/inter.woff2)`.

## A search box

To give your theme a search box, set `search: true` in its `little.yml`, so
little [indexes](../writing/search.md) each site built with it. The
quickest box is [Pagefind](https://pagefind.app/)'s own, which the built-in
theme uses:

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

Wrap the box in `{% if site.search %}`, as above, since a site can turn
search off. Don't test `== true`: on a site that
[leaves pages out](../writing/search.md#leaving-pages-out), `site.search`
is its settings. Load Pagefind's script with `defer`, not as the module its
docs show, so it finds the index beside it, under the path of the site's
[`url`](../getting-started/publishing.md#choosing-the-sites-url).

Pagefind's own selectors are as specific as an ID, so restyle its box with
its `--pf-*` CSS variables, like those at the end of the built-in theme's
[`style.css`](https://github.com/withlittle/little/blob/main/theme/_static/assets/style.css).
For a box of your own, use Pagefind's
[JavaScript API](https://pagefind.app/docs/api/), from
`{{ '/pagefind/pagefind.js' | url }}`. little indexes each page in the
language of its `<html lang>`, so give it one.

## A menu of versions

On a site with [versions](../writing/versions.md), `page.versions` lists
them in the order of `versions`, each with the `url` of this page in that
version, if it has one, else of its home page, so a menu can keep the
reader on the page they're reading. `site.version` is the `title` of the
one being built:

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

A site without `versions` has no `site.versions`, `site.version`, or
`page.versions`, and one with a single version needs no menu. Each `url`
already has the path of the site's `url`, so write it with `safe`, not
through the `url` filter.

Each entry has the keys it's written with in `versions`, like `ref`, and
any of your own, like `released`, for the theme to show. little sets its
`url`, and its `root`, which is true only for the version at `/`, so
`page.versions | selectattr("root") | first` is that one.

## A copyright notice

`site.year` is the year little builds the site in, in UTC, so a theme's
copyright notice stays current without editing it each January:

```html
<footer>&copy; {{ site.year }} {{ site.name }}. All rights reserved.</footer>
```

With [`SOURCE_DATE_EPOCH`](https://reproducible-builds.org/specs/source-date-epoch/)
set, for a reproducible build, little builds the site as if at that time,
so `site.year` is that time's year. A `year` in `little.yml` replaces it.
The built-in theme shows no notice.
