---
title: OpenAPI
description: Add a page for each OpenAPI document, with its operations and schemas, with the openapi plugin.
---

# OpenAPI

[`openapi`](https://github.com/withlittle/little/tree/main/plugins/openapi),
a plugin in little's repository, adds the reference of an HTTP API to a
site. It adds a page for each [OpenAPI](https://www.openapis.org) document,
in JSON or YAML, as Redoc and Swagger UI show them, but in the site's
theme.

It reads OpenAPI 3.0, 3.1, and 3.2, and follows each `$ref`, to other files
too. It needs no Node, and puts no script on the page. So a site builds the
same each time, offline, and its search finds each operation.

## Using it

Name it in your site's `little.yml`, with the folder your documents are in.
If you like, set where their pages go:

```yaml
# little.yml
plugins:
  openapi:
    wasm: https://github.com/withlittle/little/releases/download/openapi-v0.1.0/little_openapi.wasm
    read: api
    url: /api
```

- `read` names the folder your documents are in, from the site's folder,
  like `api`. Name more than one in a list, like `[api, events]`.
- `url` is the folder its pages go in, from the source's folder. It's the
  source's own, `/`, unless you set it.

It has no other settings. One it doesn't have, or no `read`, stops the
build, saying why.

A page's title is its document's `info.title`, like `Petstore`. To call it
something else in the nav, give its URL a `title` in your
[nav](../writing/navigation.md#writing-the-nav-yourself).

## Getting it

Each version of `openapi` is released in little's repository, as a `.wasm`
file, about 700 KB. A site names it by its URL, with the version in it, like
`openapi-v0.1.0` above.

little downloads it in the site's first build, and uses that copy from then
on, as for [any plugin](using-plugins.md#naming-a-plugin). So to update it,
change the URL to a later version's.

- Its releases, among
  [little's](https://github.com/withlittle/little/releases), list what
  changed in each version, and the version of little's
  [hooks](../reference/plugin-hooks.md) it's built for.
- Each release carries a `THIRD-PARTY-NOTICES.md`, with the licences of
  the crates it's built from.
- To build it yourself, see its
  [README](https://github.com/withlittle/little/tree/main/plugins/openapi#building-it).

## Its pages

It reads each `.json`, `.yaml`, and `.yml` file in `read`, and in the
folders in it. It skips files and folders whose names start with `.`, and
`node_modules`. A file whose `openapi`, at its top level, is 3, like
`3.1.0`, is a document. It passes over other files quietly, like a schema
in a file of its own, but for those [Warnings](#warnings) lists.

Each document is a page, at its path in `read`, without its extension, in
`url`. Take the example in [Using it](#using-it). There, `api/openapi.yaml`
is the page `api/openapi.md`, at `/api/openapi`. And `api/v2/openapi.yaml`
is at `/api/v2/openapi`.

Each page is built as if it were a Markdown file at that path in the site's
source. So it's in the nav and search, and its links are checked. A
document in the source, like `docs/api/openapi.yaml`, is copied too, as any
file is, so readers can download it.

These stop the build:

- A file in the source at the same path as a page, like
  `docs/api/openapi.md`.
- Two documents that would be the same page, like `pets.json` and
  `pets.yaml`.
- Two folders in `read` with documents of the same name, like
  `api/v1/openapi.yaml` and `api/v2/openapi.yaml`, with
  `read: [api/v1, api/v2]`. Read `api` instead. Or give each folder an
  entry of its own under `plugins`, with another name, the same `wasm`, and
  its own `url`:

  ```yaml
  # little.yml
  plugins:
    v1_api:
      wasm: https://github.com/withlittle/little/releases/download/openapi-v0.1.0/little_openapi.wasm
      read: api/v1
      url: /v1
    v2_api:
      wasm: https://github.com/withlittle/little/releases/download/openapi-v0.1.0/little_openapi.wasm
      read: api/v2
      url: /v2
  ```

## What a page shows

A page looks like Redoc's, in the site's theme. It shows:

- The document's `info.title`, as its heading. A document without one is
  titled with its file's name, like `openapi`.
- Its version, terms of service, contact, licence, and external docs.
- Its servers, with their variables, and the security it needs.
- Its description.
- `## Authentication`, with each of its security schemes, under a heading
  of its name, like ``### `api_key` ``. Each says what a client sends, like
  an API key in a header, or OAuth 2.0's flows and scopes.
- A `##` heading for each tag, with the tag's description. Under it is each
  operation with the tag, in the document's order. Tags are in the order
  `tags` lists them, then those it doesn't.
- `## Webhooks`, with each of its webhooks, shown as an operation is.
- `## Schemas`, with each schema in `components.schemas`, under a heading
  of its name, like ``### `Pet` ``. Each has its type, its description, a
  table of its fields, and its example.

`info.summary`, if it has one, is the page's
[description](../writing/frontmatter.md), for search engines and link
previews.

Each operation is under a heading of its `summary`, like
`### List all pets`, else its method and path, like
``### `GET /pets/{petId}` ``. It shows:

- Its method and path, in an `http` code block, unless they're its heading.
- That it's deprecated, if it is.
- Its description, else its path's.
- The security it needs, and its servers, if it has its own.
- A table of its parameters for each place they go: the path, the query,
  the headers, and the cookies.
- Its request body, with a table of its fields, and its examples.
- A table of its responses, with each one's status, description, and body.
  Then each response's headers, fields, and examples.

Some operations, tags, and parameters are shown otherwise, or not at all:

- An operation with more than one tag is under each. Those with none are
  under `## Other endpoints`, after the tags, or `## Endpoints`, if no
  operation has a tag.
- A tag's heading is its `summary`, new in 3.2, else its name.
- A tag with no operations isn't shown.
- A header parameter named `Accept`, `Content-Type`, or `Authorization`
  isn't shown, as OpenAPI ignores it. Nor is a response's `Content-Type`
  header.
- Extensions, like `x-logo`, aren't read.

## Types and fields

A schema is shown as its type, in a few words:

| Schema | Shown like |
| --- | --- |
| A type, with its format | `string(date-time)` |
| An array | array of `Pet` |
| `oneOf` or `anyOf` | `Cat` or `Dog` |
| `allOf` | `Pet` and `object` |
| A type that can be null | `string` or `null` |
| A map | map of `string` |

A map is an object with `additionalProperties`, but no fields.

A schema in `components.schemas` is shown by its name, linked to its
heading under Schemas. Any other object's fields are listed in a table,
with those of the objects in it, like `owner.name`, and of an array's
items, like `items[].sku`.

After each field's description are notes on it:

| Note | Like |
| --- | --- |
| Whether it's read-only, write-only, or deprecated | Read-only. |
| The values it can be | One of `dog` or `cat`. |
| Its default | Defaults to `9`. |
| Its limits | At most `100`. At least 1 character. |
| Its example | Example: `Rex`. |

## Descriptions

A description is Markdown, as OpenAPI says, and is shown as it's written.
But each heading in it is moved below the heading it's under. So a `#` in
an operation's description is a `####`, below the operation's `###`.

- A description in a table, like a field's, is on one line.
- Text that isn't Markdown, like a summary or a name, is escaped. So it
  shows as it's written.

## `$ref`s

A `$ref` leads to a part of the document, like
`$ref: '#/components/schemas/Pet'`, or to another file, like
`$ref: ./schemas/Pet.yaml`, from the folder of the file it's in. The plugin
follows each, and shows what it leads to.

A file a `$ref` leads to must be in a folder `read` names. The plugin reads
only those, in its [sandbox](using-plugins.md#the-sandbox).

## Warnings

What it can't read is warned of, and `little build --strict` fails on each:

- A document in Swagger 2.0, or in a version of OpenAPI other than 3. It
  leaves it out:

  ```text
  warning: plugin `openapi`: `api/swagger.json` is in Swagger 2.0, which it doesn't read: convert it to OpenAPI 3 first
  ```

- A file that mentions `openapi` or `swagger`, but can't be parsed. It
  leaves it out.
- A `$ref` it can't follow, like one to a URL, as it has no network, or to
  a file that isn't there. It shows the `$ref` as it's written.
- No OpenAPI 3 document in any folder in `read`.

## Limits, for now

- It reads only OpenAPI 3. Convert a document in Swagger 2.0 to OpenAPI 3
  first.
- It doesn't show callbacks, links, a media type's `encoding`, or a
  parameter's `style` and `explode`.
- It shows only JSON Schema's common keywords. So it doesn't show `not` or
  `if`, nor OpenAPI's `discriminator` or `xml`.
- The schemas of a `oneOf` or `anyOf`, and of a map's values, aren't listed
  in a table. Those in `components.schemas` link to their headings.
- In an `allOf`, a field in more than one schema is shown as the first
  defines it.
- A `$ref` to a URL, or to a schema's `$id` or `$anchor`, isn't followed.
- YAML's merge keys, `<<`, aren't merged, as YAML 1.2 has none.
- 3.2's nested tags are listed one after another.
- There's no console to try an operation in.
- Its examples are the document's own, not made from its schemas. An
  example that's an object or an array isn't in a field's notes.
- A document is one page, however big. A reader finds an operation on it
  with the browser's own search.
- A code block, comment, or `<pre>` in a description that isn't closed can
  swallow what follows it.
