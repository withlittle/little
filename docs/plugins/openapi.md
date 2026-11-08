---
title: OpenAPI
description: Add a page for each OpenAPI document, with its operations and schemas, with the openapi plugin.
---

# OpenAPI

`openapi`, one of
[little's own plugins](using-plugins.md#littles-own-plugins), adds the
reference of an HTTP API to your site: a
[page](using-plugins.md#pages-and-files) for each
[OpenAPI](https://www.openapis.org) document, 3.0, 3.1, or 3.2, in JSON or
YAML, as Redoc and Swagger UI show them, but in your site's theme, and with
no script, so your site's search finds each operation.

Name it in your site's [`plugins`](using-plugins.md#naming-a-plugin), with
the folder your documents are in:

```yaml
# little.yml
plugins:
  openapi:
    wasm: https://github.com/withlittle/little/releases/download/openapi-v0.1.0/little_openapi.wasm
    read: api
    url: /api
```

- `read` names the folder your documents are in, like `api`, or a list of
  folders, like `[api, events]`.
- `url` is the folder its pages go in, from the source's folder, like
  `/api`. It's the source's own, `/`, unless you set it.

## Its pages

It reads each `.json`, `.yaml`, and `.yml` file in `read`, and in the
folders in it, but for `node_modules`, and those whose names start with
`.`. A file whose `openapi`, at its top level, is 3, like `3.1.0`, is a
document. Other files, like a schema in a file of its own, are passed over,
but one in Swagger 2.0 or another version of OpenAPI, or one that mentions
`openapi` or `swagger` but can't be parsed, is
[warned of](using-plugins.md#when-a-plugin-fails). YAML is read as YAML
1.2, which has no merge keys, so `<<` is a key like any other.

Each document is a page, at its path in `read`, without its extension, in
`url`. For the example above, `api/openapi.yaml` is the page
`api/openapi.md`, at `/api/openapi`, and `api/v2/openapi.yaml` is at
`/api/v2/openapi`. A document in the source, like `docs/api/openapi.yaml`,
is copied too, as any file is, so readers can download it.

Two folders in `read` with documents of the same name, like
`api/v1/openapi.yaml` and `api/v2/openapi.yaml`, with
`read: [api/v1, api/v2]`, would be the same page, which stops the build.
Read `api` instead, or give each folder an entry of its own under
`plugins`, with another name, the same `wasm`, and its own `url`:

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

A page looks like Redoc's, in your site's theme. Its title and heading are
the document's `info.title`, like `Petstore`, else its file's name, like
`openapi`; to call it something else in the nav, give its URL a `title` in
[your nav](../writing/navigation.md#writing-the-nav-yourself). Its
[description](../writing/frontmatter.md), for search engines and link
previews, is `info.summary`, if it has one. Under its heading, it shows:

- The document's version, terms of service, contact, licence, and external
  docs.
- Its servers, with their variables, and the security it needs.
- Its description.
- `## Authentication`, with each of its security schemes, under a heading
  of its name, like ``### `api_key` ``, each saying what a client sends,
  like an API key in a header, or OAuth 2.0's flows and scopes.
- A `##` heading for each tag, with the tag's description, and under it
  each operation with the tag, in the document's order. Tags are in the
  order `tags` lists them, then those it doesn't.
- `## Webhooks`, with each of its webhooks, shown as an operation is.
- `## Schemas`, with each schema in `components.schemas`, under a heading
  of its name, like ``### `Pet` ``, with its type, its description, a
  table of its fields, and its example.

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
- A table of its responses, with each one's status, description, and body,
  then each response's headers, fields, and examples.

Some operations, tags, and parameters are shown otherwise, or not at all:

- An operation with more than one tag is under each. Those with none are
  under `## Other endpoints`, after the tags, or `## Endpoints`, if no
  operation has a tag.
- A tag's heading is its `summary`, new in 3.2, else its name.
- A tag with no operations isn't shown.
- A header parameter named `Accept`, `Content-Type`, or `Authorization`
  isn't shown, as OpenAPI ignores it, nor is a response's `Content-Type`
  header.
- Examples are the document's own, not made from its schemas.
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

A description is Markdown, as OpenAPI says, and is shown as it's written,
but each heading in it is moved below the heading it's under, so a `#` in
an operation's description is a `####`, below the operation's `###`. A
description in a table, like a field's, is on one line. Text that isn't
Markdown, like a summary or a name, is escaped, so it shows as it's
written.

## `$ref`s

A `$ref` leads to a part of the document, like
`$ref: '#/components/schemas/Pet'`, or to another file, like
`$ref: ./schemas/Pet.yaml`, from the folder of the file it's in. The plugin
follows each, and shows what it leads to.

A file a `$ref` leads to must be in a folder `read` names, as the plugin
reads only those, in its [sandbox](using-plugins.md#the-sandbox). A `$ref`
it can't follow, like one to a URL, or to a file that isn't there, is
warned of, and shown as it's written.
