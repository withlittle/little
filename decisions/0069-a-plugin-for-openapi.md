# 0069 — A plugin for OpenAPI

- Status: accepted
- Date: 2026-11-06
- Amends: [0051](./0051-semantic-versioning-and-conventional-commits.md)

## Context
[0031](./0031-a-plugin-for-pythons-docstrings.md),
[0044](./0044-a-plugin-for-rusts-doc-comments.md),
[0048](./0048-a-plugin-for-javascripts-doc-comments.md), and
[0064](./0064-a-plugin-for-gos-doc-comments.md) added plugins that build
an API reference from a language's code. An HTTP API is documented
another way: in an OpenAPI document, a file of JSON or YAML, like
`openapi.yaml`, that lists its paths, the operations on each, like
`GET /pets/{petId}`, with their parameters, request bodies, and
responses, the schemas of what they send and receive, in JSON Schema, and
how a client authenticates, with descriptions in CommonMark. OpenAPI 3.0,
3.1, which takes JSON Schema 2020-12 as it is, and 3.2 differ only in
details; Swagger 2.0, before them, is another format. A document can be
split over files, each part led to with a `$ref`, like
`$ref: ./schemas/Pet.yaml`, and a schema that's used in many places is
named in `components.schemas`, and led to with
`$ref: '#/components/schemas/Pet'`.

Redoc, Swagger UI, and Scalar show a document on a page of their own, with
a script, apart from a project's other docs; widdershins writes it as
Markdown, with Node.

## Options considered
- **Which hook:** `formats`, which reads a file in an extension it claims,
  as `jupyter` reads `.ipynb`; but an OpenAPI document is a `.json` or
  `.yaml` file, so a plugin that claimed those would be given every one in
  the source, as a page, and not only documents; or `files`, which adds a
  page for each document in the folders it reads, as the plugins that add
  an API reference add theirs.
- **How it reads YAML:** with serde_yaml, archived; its forks,
  serde_yaml_ng and serde_norway, last released in 2024; serde-saphyr,
  kept up, which reads YAML 1.1's `yes` and `on` as `true`, unless it's
  told not to, and `1.10` as `1.1`, and brings encoding_rs, under
  BSD-3-Clause too; or yaml-rust2, kept up, small, under MIT or Apache-2.0,
  which reads YAML 1.2, as OpenAPI recommends, and keeps a number's text,
  but has no serde, so the plugin turns its tree into JSON's.
- **How a page is ordered:** by each operation's tags, as Redoc and Swagger
  UI group them, or by its path; with each schema where it's used, or each
  named one once, under its own heading, linked to from where it's used.
- **How many pages:** one for each document, so a reader finds an
  operation with the browser's own search, as on Redoc's page; or one for
  each tag, or operation, which keeps a big API's pages short, but needs a
  page to list them.

## Decision
- `plugins/openapi/` is the plugin, a crate, `little-openapi`, in its own
  Cargo workspace, which builds `little_openapi.wasm`:

  ```sh
  cargo build --release --target wasm32-wasip2 --manifest-path plugins/openapi/Cargo.toml
  ```

  It exports `setup`, and `files`, which adds a page for each OpenAPI 3
  document in the folders it reads. It's named for its format, as
  `asciidoc` and `orgmode` are
  ([0062](./0062-plugins-for-asciidoc-and-orgmode.md)), not `<language>doc`,
  as [0047](./0047-naming-api-reference-plugins.md) names those that read a
  language's code, since OpenAPI isn't one. It shares
  `plugins/api_reference/`'s glue, settings, and helpers for pages and
  their Markdown, but not its model of a page, which is of code.
- A site names it, with the folders its documents are in, in `read`, and,
  in `url`, the folder its pages go in, `/` unless it's set:

  ```yaml
  plugins:
    openapi:
      wasm: ./plugins/openapi/target/wasm32-wasip2/release/little_openapi.wasm
      read: api
  ```

  A document's page is at its path in the folder, without its extension,
  so `api/openapi.yaml` is the page `/openapi`; two that would be the same
  page, like `pets.json` and `pets.yaml`, stop the build. It has no
  `title`: a page's title is its document's `info.title`, and `nav` can
  call it something else.
- It reads each `.json`, `.yaml`, and `.yml` file in the folders, and the
  folders in them, but those whose names start with `.`, and
  `node_modules`, and each whose `openapi`, at its top level, is 3, like
  `3.1.0`, is a document. It reads JSON with serde_json, keeping the order
  of each object's keys, and each number as it's written, and YAML with
  yaml-rust2, as YAML 1.2. A document of another version, or Swagger 2.0's,
  is warned of, and left out, as is a file that mentions `openapi` or
  `swagger` but can't be parsed; other files are passed over quietly, and
  no document at all is warned of.
- It follows each `$ref`, within a file, and to another, from the folder of
  the file it's in, to a part of it, by a JSON pointer, like
  `./schemas.yaml#/Pet`, as OpenAPI does. One to a URL, or to what it can't
  read or find, is warned of, and shown as it's written.
- A page has the document's `info.title` as its `#` heading, then its
  version, its terms of service, contact, licence, and external docs, its
  servers, and the security it needs, then its description, as Redoc
  shows them, so the description's own headings don't end up above them;
  and `info.summary` is the page's `description`, for search engines and
  link previews. Then it has:
  - `## Authentication`, with each security scheme in
    `components.securitySchemes`, under a `###` heading of its name: what a
    client sends, like an API key in a header, and OAuth 2.0's flows and
    scopes;
  - a `##` heading for each tag, in the order `tags` lists them, then those
    it doesn't, with the tag's description, then each operation with the
    tag, in the document's order, under a `###` heading of its `summary`,
    else its method and path, like `` `GET /pets/{petId}` ``. An operation
    with more than one tag is under each, and those with none are under
    "Endpoints", or "Other endpoints", after the tags;
  - `## Webhooks`, with each of 3.1's webhooks, as an operation is;
  - `## Schemas`, with each schema in `components.schemas`, under a `###`
    heading of its name, with its type, its description, a table of its
    fields, and its example.
- Each operation has its method and path in an `http` code block, unless
  they're its heading, its description, the security it needs, if it sets
  its own, its servers, if it has its own, a table of its parameters for
  each place they go, the path, query, headers, or cookies, its request
  body, and a table of its responses, with each one's status, description,
  and body; then each response's headers, fields, and examples. What's
  `deprecated` is marked so.
- A schema is written as its type, like `` `string(date-time)` ``,
  ``array of `string` ``, or `` `Cat` or `Dog` ``, as a link to its heading
  under Schemas, if it has one. Another object's fields are listed in a
  table, with each field of an object in one, like `owner.name`, and each
  field's notes, like its default, the values it can be, and its limits,
  after its description. JSON Schema's other keywords aren't shown.
- A description is written as its Markdown, with each heading in it moved
  below the heading it's under, so a `#` in an operation's is a `####`,
  below the operation's `###`; in a table's cell, it's on one line. Text
  that isn't Markdown, like a summary, is escaped.
  Examples, from a media type's `example` or `examples`, else its schema's,
  are code blocks, in JSON, unless they're a string, like XML. Extensions,
  `x-`, aren't read.
- `openapi` is a scope for commits
  ([0051](./0051-semantic-versioning-and-conventional-commits.md)), and
  its release's notes list the commits that changed its folder or
  `plugins/api_reference/`, as the API reference plugins' do
  ([0052](./0052-releasing-plugins-from-tags.md)).

## Consequences
- An HTTP API gets a reference in its site, in its theme, and found by its
  search, built offline, in the sandbox, the same each time, with no Node,
  and no script on the page.
- A big API's page is long, but a reader finds an operation on it with the
  browser's own search, as on Redoc's.
- The plugin adds yaml-rust2, and finl_unicode, which comrak uses, so a
  link to a schema's heading has the id comrak gives it, and uses
  pulldown-cmark, as `rustdoc` does, to find the headings in descriptions.
- little has no HTTP API, so the manual's page "HTTP API reference" can't
  use what it documents, as the pages on Python's, JavaScript's, and Go's
  code can't.
- Limits, for now:
  - Swagger 2.0 isn't read, so a document in it is converted to OpenAPI 3
    first;
  - a `$ref` to a URL isn't followed, as the sandbox has no network, and
    nor is one to a schema's `$id` or `$anchor`;
  - callbacks, links, a media type's `encoding`, and a parameter's `style`
    and `explode` aren't shown, nor are JSON Schema's keywords but the
    common ones, like `not` and `if`, or `discriminator` and `xml`;
  - YAML's merge keys, `<<`, aren't merged, as YAML 1.2 has none;
  - 3.2's nested tags are listed one after another;
  - there's no console to try an operation in, and its examples are the
    document's own, not made from its schemas.
