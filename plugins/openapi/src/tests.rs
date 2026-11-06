use little_api_reference::testing::{assert_has, folder_with, markdown, paths};

use super::*;

/// comrak's options, as little sets them, with every extension on, but
/// smart punctuation, which changes text, and autolinks only if
/// `autolink`.
fn options(autolink: bool) -> comrak::Options<'static> {
    let mut options = comrak::Options::default();
    let extension = &mut options.extension;
    extension.table = true;
    extension.strikethrough = true;
    extension.autolink = autolink;
    extension.tasklist = true;
    extension.footnotes = true;
    extension.alerts = true;
    extension.shortcodes = true;
    extension.math_dollars = true;
    extension.math_code = true;
    extension.superscript = true;
    extension.subscript = true;
    extension.highlight = true;
    extension.insert = true;
    extension.description_lists = true;
    extension.inline_footnotes = true;
    extension.block_directive = true;
    extension.multiline_block_quotes = true;
    extension.cjk_friendly_emphasis = true;
    extension.front_matter_delimiter = Some("---".into());
    extension.header_id_prefix = Some(String::new());
    options.render.hardbreaks = true;
    options.render.r#unsafe = true;
    options
}

/// The HTML comrak writes for `markdown`, as little writes a page, with
/// every extension on, but autolinks only if `autolink`.
pub fn html(markdown: &str, autolink: bool) -> String {
    comrak::markdown_to_html(markdown, &options(autolink))
}

fn settings(read: &[&str], url: &str) -> Settings {
    Settings {
        read: read.iter().map(|folder| folder.to_string()).collect(),
        url: url.to_string(),
    }
}

/// What the plugin adds from `files`, in a folder for the test called
/// `test`, which it reads, with its pages in `api`, and the folder, which
/// is gone by then.
fn built(test: &str, files: &[(&str, &str)]) -> (Added, String) {
    let root = folder_with("little-openapi", test, files);
    let added = document(&settings(&[&root], "api")).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    (added, root)
}

/// What the plugin warns of, building `added` from `root`, with `<root>`
/// in place of it.
fn warnings(added: &Added, root: &str) -> Vec<String> {
    (added.warnings.iter())
        .map(|warning| warning.replace(root, "<root>"))
        .collect()
}

/// The page of the one document in `file`, which holds `text`, for the
/// test called `test`, and what the plugin warns of.
fn page(test: &str, file: &str, text: &str) -> (String, Vec<String>) {
    let (added, root) = built(test, &[(file, text)]);
    let stem = file.rsplit_once('.').unwrap().0;
    let page = markdown(&added, &format!("api/{stem}.md")).to_string();
    (page, warnings(&added, &root))
}

#[test]
fn settings_name_the_folders_it_reads_and_where_its_pages_go() {
    assert_eq!(
        Settings::parse(r#"{"read":["api"]}"#),
        Ok(settings(&["api"], ""))
    );
    let parsed = Settings::parse(r#"{"read":["./api","specs"],"url":"/api/"}"#);
    assert_eq!(parsed, Ok(settings(&["./api", "specs"], "api")));
    let parsed = Settings::parse(r#"{"read":["api"],"url":"/"}"#);
    assert_eq!(parsed, Ok(settings(&["api"], "")));
}

#[test]
fn settings_must_read_a_folder() {
    for json in ["{}", r#"{"read":[]}"#] {
        let error = Settings::parse(json).unwrap_err();
        assert!(error.contains("like `read: api`"), "{json}: {error}");
    }
    let error = Settings::parse("[]").unwrap_err();
    assert!(
        error.starts_with("its settings must be a mapping"),
        "{error}"
    );
}

#[test]
fn settings_must_be_known() {
    let error = Settings::parse(r#"{"read":["api"],"title":"API"}"#).unwrap_err();
    assert!(
        error.starts_with("it has no setting `title`, only `read`"),
        "{error}"
    );
    for url in [r#""../api""#, r#""api/./v1""#, r#""api//v1""#, "3"] {
        let json = format!(r#"{{"read":["api"],"url":{url}}}"#);
        let error = Settings::parse(&json).unwrap_err();
        assert!(error.starts_with("`url` must be"), "{url}: {error}");
    }
}

/// An OpenAPI 3 document with nothing but its title.
const EMPTY: &str = "openapi: 3.1.0\ninfo: {title: Empty, version: '1'}\n";

#[test]
fn each_openapi_3_document_in_the_folders_it_reads_is_a_page() {
    let (added, root) = built(
        "found",
        &[
            ("a.yaml", EMPTY),
            (
                "v1/b.json",
                r#"{"openapi": "3.0.3", "info": {"title": "B", "version": "1"}}"#,
            ),
            ("v2/c.YML", EMPTY),
            (".hidden/d.yaml", EMPTY),
            ("node_modules/e.yaml", EMPTY),
            ("f.txt", EMPTY),
            ("package.json", r#"{"name": "demo"}"#),
            (
                "notes.yaml",
                "# Notes on our openapi document\ntitle: Notes\n",
            ),
            ("broken.yaml", "title: [\n"),
        ],
    );
    assert_eq!(paths(&added), ["api/a.md", "api/v1/b.md", "api/v2/c.md"]);
    assert_eq!(warnings(&added, &root), Vec::<String>::new());
    assert_has(markdown(&added, "api/v1/b.md"), &["# B\n"]);
}

#[test]
fn it_reads_only_openapi_3() {
    let (added, root) = built(
        "versions",
        &[
            ("a.yaml", "openapi: 3\ninfo: {title: A, version: '1'}\n"),
            ("b.yaml", "openapi: 3.1\ninfo: {title: B, version: '1'}\n"),
            (
                "c.json",
                r#"{"swagger": "2.0", "info": {"title": "C", "version": "1"}}"#,
            ),
            ("d.yaml", "openapi: 4.0.0\ninfo: {title: D, version: '1'}\n"),
        ],
    );
    assert_eq!(paths(&added), ["api/a.md", "api/b.md"]);
    assert_eq!(
        warnings(&added, &root),
        [
            "`<root>/c.json` is in Swagger 2.0, which it doesn't read: convert it to OpenAPI 3 \
             first",
            "`<root>/d.yaml` is in OpenAPI 4.0.0, but it reads only OpenAPI 3, like `3.1.0`",
        ]
    );
}

#[test]
fn it_warns_of_a_document_it_cant_parse() {
    let root = folder_with(
        "little-openapi",
        "unparsed",
        &[
            ("a.json", r#"{"openapi": "3.1.0","#),
            ("b.yaml", "openapi: 3.1.0\ninfo: [\n"),
            (
                "c.yaml",
                "\u{feff}openapi: 3.1.0\ninfo: {title: C, version: '1'}\n",
            ),
        ],
    );
    let latin1 = b"openapi: 3.1.0\ninfo: {title: Caf\xe9, version: '1'}\n";
    std::fs::write(format!("{root}/d.yaml"), latin1).unwrap();
    let added = document(&settings(&[&root], "api")).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(paths(&added), ["api/c.md"]);
    assert_has(markdown(&added, "api/c.md"), &["# C\n"]);
    let warnings = warnings(&added, &root);
    let [json, yaml, utf8] = &warnings[..] else {
        panic!("{warnings:#?}");
    };
    assert!(json.starts_with("can't parse `<root>/a.json`: "), "{json}");
    assert!(yaml.starts_with("can't parse `<root>/b.yaml`: "), "{yaml}");
    assert_eq!(utf8, "can't parse `<root>/d.yaml`: it isn't UTF-8");
}

#[test]
fn it_warns_of_a_folder_it_cant_read_or_that_has_no_document() {
    let root = folder_with(
        "little-openapi",
        "none",
        &[("notes.yaml", "title: Notes\n")],
    );
    let missing = format!("{root}/missing");
    let added = document(&settings(&[&root, &missing], "api")).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    assert!(added.pages.is_empty());
    let warnings = warnings(&added, &root);
    let [unread, none] = &warnings[..] else {
        panic!("{warnings:#?}");
    };
    assert!(
        unread.starts_with("can't read `<root>/missing`: "),
        "{unread}"
    );
    assert_eq!(
        none,
        "there's no OpenAPI 3 document in `<root>`: a `.json`, `.yaml`, or `.yml` file whose \
         `openapi` is like `3.1.0`"
    );
}

#[test]
fn two_documents_cant_be_the_same_page() {
    let root = folder_with(
        "little-openapi",
        "clash",
        &[
            ("pets.json", r#"{"openapi": "3.1.0"}"#),
            ("pets.yaml", EMPTY),
            ("v1/api.yaml", EMPTY),
            ("v2/api.yaml", EMPTY),
        ],
    );
    let error = |read: &[&str]| {
        let error = document(&settings(read, "api")).unwrap_err();
        error.replace(&root, "<root>")
    };
    let (v1, v2) = (format!("{root}/v1"), format!("{root}/v2"));
    let (in_one, in_two) = (error(&[&root]), error(&[&v1, &v2]));
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(
        in_one,
        "`<root>/pets.json` and `<root>/pets.yaml` would both be the page `api/pets.md`: rename \
         or move one"
    );
    assert_eq!(
        in_two,
        "`<root>/v1/api.yaml` and `<root>/v2/api.yaml` would both be the page `api/api.md`: \
         rename or move one"
    );
}

const PETSTORE: &str = r#"openapi: 3.0.3
info:
  title: Petstore
  version: 1.0.0
  termsOfService: https://example.com/terms
  contact:
    name: API Support
    url: https://example.com/support
    email: support@example.com
  license:
    name: MIT
    url: https://opensource.org/licenses/MIT
  description: |
    A sample API that uses a petstore as an example.

    # Introduction

    It shows what OpenAPI can do.
externalDocs:
  description: Find out more
  url: https://example.com/docs
servers:
  - url: https://{environment}.example.com/v1
    description: The production API
    variables:
      environment:
        default: api
        enum: [api, staging]
        description: Which environment
security:
  - api_key: []
  - petstore_auth: [write:pets, read:pets]
tags:
  - name: pets
    description: Everything about your pets.
  - name: store
    description: Access to the store.
paths:
  /pets:
    get:
      summary: List all pets
      tags: [pets]
      parameters:
        - name: limit
          in: query
          description: How many items to return at one time
          schema:
            type: integer
            format: int32
            maximum: 100
      responses:
        '200':
          description: A paged array of pets
          headers:
            x-next:
              description: A link to the next page of responses
              schema:
                type: string
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Pets'
        default:
          description: unexpected error
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Error'
    post:
      summary: Create a pet
      tags: [pets]
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/Pet'
            example:
              name: Rex
              tag: dog
      responses:
        '201':
          description: Null response
  /pets/{petId}:
    parameters:
      - $ref: '#/components/parameters/PetId'
    get:
      deprecated: true
      tags: [pets]
      responses:
        '200':
          description: Expected response to a valid request
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Pet'
  /store/inventory:
    get:
      summary: Count pets
      tags: [store]
      security: []
      responses:
        '200':
          description: The number of pets with each status
          content:
            application/json:
              schema:
                type: object
                additionalProperties:
                  type: integer
                  format: int32
  /health:
    get:
      responses:
        '204':
          description: It's up
components:
  parameters:
    PetId:
      name: petId
      in: path
      required: true
      description: The id of the pet to retrieve
      schema:
        type: string
  schemas:
    Pet:
      type: object
      required: [id, name]
      properties:
        id:
          type: integer
          format: int64
          readOnly: true
        name:
          type: string
          example: Rex
        tag:
          type: string
          enum: [dog, cat]
        owner:
          type: object
          properties:
            name:
              type: string
              minLength: 1
    Pets:
      type: array
      maxItems: 100
      items:
        $ref: '#/components/schemas/Pet'
    Error:
      type: object
      required: [code, message]
      properties:
        code:
          type: integer
          format: int32
        message:
          type: string
  securitySchemes:
    api_key:
      type: apiKey
      name: X-API-Key
      in: header
    petstore_auth:
      type: oauth2
      flows:
        implicit:
          authorizationUrl: https://example.com/oauth/authorize
          scopes:
            write:pets: modify pets in your account
            read:pets: read your pets
"#;

#[test]
fn a_documents_page_has_its_info_operations_by_tag_and_schemas() {
    let (page, warned) = page("petstore", "petstore.yaml", PETSTORE);
    assert_has(
        &page,
        &[
            r#"# Petstore

- **Version:** `1.0.0`
- **Terms of service:** [`https://example.com/terms`](<https://example.com/terms>)
- **Contact:** [API Support](<https://example.com/support>), [support\@example.com](<mailto:support@example.com>)
- **Licence:** [MIT](<https://opensource.org/licenses/MIT>)
- **External docs:** [Find out more](<https://example.com/docs>)

**Servers:**

- `https://{environment}.example.com/v1`: The production API
  - `environment`: Which environment. One of `api` or `staging`. Defaults to `api`.

**Security:** [`api_key`](<#api_key>) or [`petstore_auth`](<#petstore_auth>) (`write:pets` and `read:pets`)

A sample API that uses a petstore as an example.
"#,
            r#"## Introduction

It shows what OpenAPI can do.
"#,
            r#"## Authentication

### `api_key`

An API key, in the `X-API-Key` header.

### `petstore_auth`

OAuth 2.0, with these flows:

- Implicit, with the authorization URL `https://example.com/oauth/authorize`.

| Scope | Description |
| --- | --- |
| `write:pets` | modify pets in your account |
| `read:pets` | read your pets |
"#,
            r#"## pets

Everything about your pets.

### List all pets

```http
GET /pets
```

**Query parameters:**

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| `limit` | `integer(int32)` | no | How many items to return at one time. At most `100`. |

**Responses:**

| Status | Description | Body |
| --- | --- | --- |
| `200` | A paged array of pets | [`Pets`](<#pets-1>), as `application/json` |
| `default` | unexpected error | [`Error`](<#error>), as `application/json` |

**Response `200` headers:**

| Name | Type | Description |
| --- | --- | --- |
| `x-next` | `string` | A link to the next page of responses |

### Create a pet

```http
POST /pets
```

**Request body** (required): [`Pet`](<#pet>), as `application/json`

**Request body example:**

```json
{
  "name": "Rex",
  "tag": "dog"
}
```

**Responses:**

| Status | Description |
| --- | --- |
| `201` | Null response |

### `GET /pets/{petId}`

**Deprecated.**

**Path parameters:**

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| `petId` | `string` | yes | The id of the pet to retrieve |

**Responses:**

| Status | Description | Body |
| --- | --- | --- |
| `200` | Expected response to a valid request | [`Pet`](<#pet>), as `application/json` |
"#,
            r#"## store

Access to the store.

### Count pets

```http
GET /store/inventory
```

**Security:** none

**Responses:**

| Status | Description | Body |
| --- | --- | --- |
| `200` | The number of pets with each status | map of `integer(int32)`, as `application/json` |
"#,
            r#"## Other endpoints

### `GET /health`

**Responses:**

| Status | Description |
| --- | --- |
| `204` | It's up |
"#,
            r#"## Schemas

### `Pet`

**Type:** `object`

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `id` | `integer(int64)` | yes | Read-only. |
| `name` | `string` | yes | Example: `Rex`. |
| `tag` | `string` | no | One of `dog` or `cat`. |
| `owner` | `object` | no |  |
| `owner.name` | `string` | no | At least 1 character. |

### `Pets`

**Type:** array of [`Pet`](<#pet>). At most 100 items.

### `Error`

**Type:** `object`

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `code` | `integer(int32)` | yes |  |
| `message` | `string` | yes |  |
"#,
        ],
    );
    assert_eq!(warned, Vec::<String>::new());
}

const ZOO: &str = r##"{
  "openapi": "3.1.0",
  "info": {
    "title": "Zoo",
    "summary": "A zoo's animals,\nand news of them.",
    "version": "2.0",
    "license": {"name": "Apache 2.0", "identifier": "Apache-2.0"}
  },
  "paths": {
    "/animals/{id}": {
      "get": {
        "summary": "Get an animal",
        "description": "# Animals\n\nEach animal has an id.",
        "parameters": [
          {"name": "id", "in": "path", "schema": {"$ref": "schemas/pet.json#/$defs/Id"}}
        ],
        "responses": {
          "200": {
            "description": "The animal",
            "content": {
              "application/json": {"schema": {"$ref": "#/components/schemas/Cat"}},
              "application/xml": {"schema": {"$ref": "#/components/schemas/Cat"}}
            }
          },
          "404": {"$ref": "https://example.com/responses/not-found.json"}
        }
      }
    },
    "/trees": {
      "post": {
        "summary": "Plant a tree",
        "requestBody": {
          "content": {"application/json": {"schema": {"$ref": "tree.json"}}}
        },
        "responses": {"201": {"description": "Planted"}}
      }
    }
  },
  "webhooks": {
    "newPet": {
      "post": {
        "summary": "A new pet",
        "requestBody": {
          "content": {"application/json": {"schema": {"$ref": "schemas/pet.json"}}}
        },
        "responses": {"200": {"description": "Received"}}
      }
    },
    "petGone": {
      "post": {"responses": {"200": {"description": "Received"}}}
    }
  },
  "components": {
    "schemas": {
      "Pet": {"$ref": "schemas/pet.json"},
      "Cat": {
        "description": "A cat.",
        "allOf": [
          {"$ref": "#/components/schemas/Pet"},
          {"type": "object", "properties": {"lives": {"type": "integer", "default": 9}}}
        ]
      },
      "Animal": {"$ref": "#/components/schemas/Cat", "description": "Any animal, for now a cat."},
      "Node": {
        "type": ["object", "null"],
        "properties": {
          "value": {"type": ["string", "null"]},
          "next": {"$ref": "#/components/schemas/Node"}
        }
      },
      "Owner": {"$ref": "https://example.com/schemas/owner.json"}
    }
  }
}
"##;

const PET: &str = r##"{
  "type": "object",
  "description": "A pet.",
  "required": ["name"],
  "properties": {
    "name": {"type": "string"},
    "kind": {"$ref": "#/$defs/Kind"}
  },
  "$defs": {
    "Id": {"type": "string", "format": "uuid"},
    "Kind": {"type": "string", "enum": ["cat", "dog"], "description": "What kind of pet it is."}
  }
}
"##;

const TREE: &str = r##"{
  "type": "object",
  "properties": {
    "name": {"type": "string"},
    "children": {"type": "array", "items": {"$ref": "#"}}
  }
}
"##;

#[test]
fn refs_lead_to_schemas_in_other_files_but_not_to_urls() {
    let (added, root) = built(
        "zoo",
        &[
            ("zoo.json", ZOO),
            ("schemas/pet.json", PET),
            ("tree.json", TREE),
        ],
    );
    assert_eq!(paths(&added), ["api/zoo.md"]);
    let page = markdown(&added, "api/zoo.md");
    assert_has(
        page,
        &[
            r#"---
description: "A zoo's animals, and news of them."
---

# Zoo

- **Version:** `2.0`
- **Licence:** Apache 2.0 (`Apache-2.0`)
"#,
            r#"## Endpoints

### Get an animal

```http
GET /animals/{id}
```

#### Animals

Each animal has an id.

**Path parameters:**

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| `id` | `string(uuid)` | yes |  |

**Responses:**

| Status | Description | Body |
| --- | --- | --- |
| `200` | The animal | [`Cat`](<#cat>), as `application/json` or `application/xml` |
| `404` | `https://example.com/responses/not-found.json` |  |

### Plant a tree

```http
POST /trees
```

**Request body:** `object`, as `application/json`

**Request body fields:**

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `name` | `string` | no |  |
| `children` | array of `tree.json` | no |  |

**Responses:**

| Status | Description |
| --- | --- |
| `201` | Planted |
"#,
            r#"## Webhooks

### A new pet

The API sends `newPet` as a `POST` request.

**Request body:** [`Pet`](<#pet>), as `application/json`

**Responses:**

| Status | Description |
| --- | --- |
| `200` | Received |

### `petGone`

The API sends a `POST` request.

**Responses:**

| Status | Description |
| --- | --- |
| `200` | Received |
"#,
            r#"## Schemas

### `Pet`

**Type:** `object`

A pet.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `name` | `string` | yes |  |
| `kind` | `string` | no | What kind of pet it is. One of `cat` or `dog`. |

### `Cat`

**Type:** [`Pet`](<#pet>) and `object`

A cat.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `name` | `string` | yes |  |
| `kind` | `string` | no | What kind of pet it is. One of `cat` or `dog`. |
| `lives` | `integer` | no | Defaults to `9`. |

### `Animal`

**Type:** [`Cat`](<#cat>)

Any animal, for now a cat.

### `Node`

**Type:** `object` or `null`

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `value` | `string` or `null` | no |  |
| `next` | [`Node`](<#node>) | no |  |

### `Owner`

**Type:** `https://example.com/schemas/owner.json`
"#,
        ],
    );
    assert_eq!(
        warnings(&added, &root),
        [
            "can't follow `$ref: https://example.com/responses/not-found.json`, in \
             `<root>/zoo.json`: it's a URL, and only files are read",
            "can't follow `$ref: https://example.com/schemas/owner.json`, in `<root>/zoo.json`: \
             it's a URL, and only files are read",
        ]
    );
}

#[test]
fn a_ref_back_to_the_document_leads_to_its_schema() {
    let openapi = "openapi: 3.1.0
info: {title: Pets, version: '1'}
components:
  schemas:
    Pet: {type: object, properties: {owner: {$ref: './owner.yaml'}}}
    Owner: {$ref: owner.yaml}
";
    let owner = "type: object
properties:
  pets: {type: array, items: {$ref: 'openapi.yaml#/components/schemas/Pet'}}
";
    let root = folder_with(
        "little-openapi",
        "back",
        &[("api/openapi.yaml", openapi), ("api/owner.yaml", owner)],
    );
    let added = document(&settings(&[&format!("{root}/./api")], "")).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(paths(&added), ["openapi.md"]);
    assert_has(
        markdown(&added, "openapi.md"),
        &[
            "### `Pet`",
            "| `owner` | [`Owner`](<#owner>) | no |  |",
            "### `Owner`",
            "| `pets` | array of [`Pet`](<#pet>) | no |  |",
        ],
    );
    assert_eq!(warnings(&added, &root), Vec::<String>::new());
}

const AUTH: &str = r#"openapi: 3.1.0
info: {title: Auth, version: '1'}
security:
  - key: []
    basic: []
  - oauth: [read]
components:
  securitySchemes:
    key: {type: apiKey, name: api_key, in: query, description: Get one from *your* settings.}
    cookie: {type: apiKey, name: session, in: cookie}
    basic: {type: http, scheme: basic}
    bearer: {type: http, scheme: bearer, bearerFormat: JWT}
    digest: {type: http, scheme: Digest}
    oidc: {type: openIdConnect, openIdConnectUrl: 'https://example.com/.well-known/openid-configuration'}
    mtls: {type: mutualTLS}
    custom: {type: x-custom}
    oauth:
      type: oauth2
      deprecated: true
      oauth2MetadataUrl: https://example.com/.well-known/oauth-authorization-server
      flows:
        authorizationCode:
          authorizationUrl: https://example.com/authorize
          tokenUrl: https://example.com/token
          refreshUrl: https://example.com/refresh
          scopes: {read: Read things, 'write:all': Write *everything*}
        clientCredentials:
          tokenUrl: https://example.com/token
          scopes: {}
"#;

#[test]
fn security_schemes_say_how_to_authenticate() {
    let (page, warned) = page("auth", "auth.yaml", AUTH);
    assert_has(
        &page,
        &[
            r#"# Auth

- **Version:** `1`

**Security:** either [`key`](<#key>) and [`basic`](<#basic>), or [`oauth`](<#oauth>) (`read`)
"#,
            r#"## Authentication

### `key`

An API key, in the `api_key` query parameter.

Get one from *your* settings.

### `cookie`

An API key, in the `session` cookie.

### `basic`

A username and password, in the `Authorization` header, with HTTP's `Basic` scheme.

### `bearer`

A bearer token, in the `Authorization` header, formatted as `JWT`.

### `digest`

Credentials, in the `Authorization` header, with HTTP's `Digest` scheme.

### `oidc`

OpenID Connect, with its configuration at `https://example.com/.well-known/openid-configuration`.

### `mtls`

Mutual TLS, with a client certificate.

### `custom`

Of type `x-custom`.

### `oauth`

**Deprecated.**

OAuth 2.0, with these flows:

- Authorization code, with the authorization URL `https://example.com/authorize`, the token URL `https://example.com/token`, and the refresh URL `https://example.com/refresh`.
- Client credentials, with the token URL `https://example.com/token`.

| Scope | Description |
| --- | --- |
| `read` | Read things |
| `write:all` | Write \*everything\* |

Its metadata is at `https://example.com/.well-known/oauth-authorization-server`.
"#,
        ],
    );
    assert_eq!(warned, Vec::<String>::new());
}

const TAGS: &str = r#"openapi: 3.2.0
info: {title: Tags, version: '1'}
tags:
  - name: b
    summary: The B's
    description: All about *b*.
    externalDocs: {url: 'https://example.com/b'}
  - name: empty
  - name: a
paths:
  /x:
    get: {summary: Get x, tags: [a, b, c], responses: {'200': {description: OK}}}
  /y:
    get: {summary: Get y, tags: [c], responses: {'200': {description: OK}}}
    put: {tags: [a], operationId: putY, responses: {'200': {description: OK}}}
"#;

#[test]
fn operations_are_under_each_of_their_tags_in_order() {
    let (page, warned) = page("tags", "tags.yaml", TAGS);
    assert_has(
        &page,
        &[
            r#"# Tags

- **Version:** `1`
"#,
            r#"## The B's

All about *b*.

**External docs:** [`https://example.com/b`](<https://example.com/b>)

### Get x

```http
GET /x
```

**Responses:**

| Status | Description |
| --- | --- |
| `200` | OK |
"#,
            r#"## a

### Get x

```http
GET /x
```

**Responses:**

| Status | Description |
| --- | --- |
| `200` | OK |

### `PUT /y`

**Responses:**

| Status | Description |
| --- | --- |
| `200` | OK |
"#,
            r#"## c

### Get x

```http
GET /x
```

**Responses:**

| Status | Description |
| --- | --- |
| `200` | OK |

### Get y

```http
GET /y
```

**Responses:**

| Status | Description |
| --- | --- |
| `200` | OK |
"#,
        ],
    );
    assert_eq!(warned, Vec::<String>::new());
    assert!(
        !page.contains("empty"),
        "a tag with no operations is hidden"
    );
    assert!(!page.contains("putY"), "an operation's id isn't its title");
}

const PARAMETERS: &str = r#"openapi: 3.2.0
info: {title: Parameters, version: '1'}
paths:
  /items/{id}:
    parameters:
      - {name: id, in: path, schema: {type: integer}, description: Old}
      - {name: verbose, in: query, schema: {type: boolean}}
      - {name: Accept, in: header, schema: {type: string}}
    get:
      summary: Get an item
      parameters:
        - {name: id, in: path, required: true, schema: {type: string}, description: The item's id.}
        - {name: X-Request-Id, in: header, schema: {type: string, format: uuid}, deprecated: true}
        - {name: session, in: cookie, required: true, schema: {type: string}}
        - name: filter
          in: query
          content:
            application/json:
              schema: {type: object, properties: {color: {type: string}}}
        - {name: Content-Type, in: header, schema: {type: string}}
        - {name: authorization, in: header, schema: {type: string}}
        - {name: q, in: querystring, content: {application/x-www-form-urlencoded: {schema: {type: object}}}}
      responses: {'200': {description: OK}}
"#;

#[test]
fn parameters_are_grouped_by_where_they_go() {
    let (page, warned) = page("parameters", "parameters.yaml", PARAMETERS);
    assert_has(
        &page,
        &[
            r#"# Parameters

- **Version:** `1`
"#,
            r#"## Endpoints

### Get an item

```http
GET /items/{id}
```

**Path parameters:**

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| `id` | `string` | yes | The item's id. |

**Query parameters:**

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| `verbose` | `boolean` | no |  |
| `filter` | `object` | no |  |

**Query string:**

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| `q` | `object` | no |  |

**Headers:**

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| `X-Request-Id` | `string(uuid)` | no | Deprecated. |

**Cookies:**

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| `session` | `string` | yes |  |

**Responses:**

| Status | Description |
| --- | --- |
| `200` | OK |
"#,
        ],
    );
    assert_eq!(warned, Vec::<String>::new());
    for hidden in ["Old", "`Accept`", "`Content-Type`", "`authorization`"] {
        assert!(!page.contains(hidden), "{hidden} is shown");
    }
}

const BODIES: &str = r#"openapi: 3.1.0
info: {title: Bodies, version: '1'}
paths:
  /orders:
    post:
      summary: Place an order
      requestBody:
        description: The order to place.
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [items]
              properties:
                items:
                  type: array
                  items:
                    type: object
                    required: [sku]
                    properties:
                      sku: {type: string, pattern: '^[A-Z]{3}$'}
                      quantity: {type: integer, minimum: 1, default: 1}
                note: {type: string, maxLength: 200}
            examples:
              one:
                summary: One item
                value: {items: [{sku: ABC}]}
              many:
                value: {items: [{sku: ABC, quantity: 2}, {sku: DEF}]}
          application/x-www-form-urlencoded:
            schema: {type: object, properties: {note: {type: string}}}
      responses:
        '201':
          description: Placed
          content:
            application/json:
              schema: {$ref: '#/components/schemas/Order'}
              example: {id: 1}
            application/xml:
              schema: {$ref: '#/components/schemas/Order'}
        '400':
          description: Bad request
          content:
            application/problem+json:
              schema:
                type: object
                properties:
                  title: {type: string}
                example: {title: Out of stock}
components:
  schemas:
    Order:
      type: object
      properties:
        id: {type: integer}
"#;

#[test]
fn bodies_have_their_fields_and_examples() {
    let (page, warned) = page("bodies", "bodies.yaml", BODIES);
    assert_has(
        &page,
        &[
            r#"# Bodies

- **Version:** `1`
"#,
            r#"## Endpoints

### Place an order

```http
POST /orders
```

**Request body** (required): `object`, as `application/json` or `application/x-www-form-urlencoded`

The order to place.

**Request body fields**, as `application/json`:

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `items` | array of `object` | yes |  |
| `items[].sku` | `string` | yes | Matches `^[A-Z]{3}$`. |
| `items[].quantity` | `integer` | no | Defaults to `1`. At least `1`. |
| `note` | `string` | no | At most 200 characters. |

**Request body fields**, as `application/x-www-form-urlencoded`:

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `note` | `string` | no |  |

**Request body example**, as `application/json`: One item

```json
{
  "items": [
    {
      "sku": "ABC"
    }
  ]
}
```

**Request body example**, as `application/json`: `many`

```json
{
  "items": [
    {
      "sku": "ABC",
      "quantity": 2
    },
    {
      "sku": "DEF"
    }
  ]
}
```

**Responses:**

| Status | Description | Body |
| --- | --- | --- |
| `201` | Placed | [`Order`](<#order>), as `application/json` or `application/xml` |
| `400` | Bad request | `object`, as `application/problem+json` |

**Response `201` example**, as `application/json`:

```json
{
  "id": 1
}
```

**Response `400` fields:**

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `title` | `string` | no |  |

**Response `400` example:**

```json
{
  "title": "Out of stock"
}
```
"#,
            r#"## Schemas

### `Order`

**Type:** `object`

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `id` | `integer` | no |  |
"#,
        ],
    );
    assert_eq!(warned, Vec::<String>::new());
}

const MORE: &str = r#"openapi: 3.0.3
info:
  title: More
  version: '1'
  description: |
    # Report

    Daily reports.
paths:
  x-internal:
    get: {summary: Hidden, responses: {'200': {description: OK}}}
  /reports:
    description: Reports, by day.
    servers:
      - url: https://reports.example.com
    get:
      externalDocs: {url: 'https://example.com/reports', description: How reports work}
      security:
        - key: []
      responses:
        '200':
          $ref: '#/components/responses/Reports'
        x-note: ignored
    post:
      servers:
        - url: https://write.example.com
          description: Where to write
      requestBody:
        $ref: '#/components/requestBodies/Missing'
      responses:
        '202':
          description: Accepted
          headers:
            Content-Type: {schema: {type: string}}
            Location:
              description: Where the report will be
              schema: {type: string, format: uri}
              example: /reports/1
  /reports/{day}:
    get:
      summary: Get a day's report
      parameters:
        - name: day
          in: path
          required: true
          schema: {type: string, format: date}
          examples:
            today: {value: '2024-01-01'}
            object: {value: {a: 1}}
      responses:
        '200':
          description: The report
          content:
            text/csv:
              schema: {type: string}
              example: "day,count\n2024-01-01,3\n"
            application/xml:
              schema: {type: string}
              examples:
                linked: {externalValue: 'https://example.com/report.xml'}
            application/json:
              schema: {$ref: '#/components/schemas/Report'}
components:
  securitySchemes:
    key: {type: apiKey, name: key, in: header}
  responses:
    Reports:
      description: The reports
      content:
        application/json:
          schema:
            type: array
            items: {$ref: '#/components/schemas/Report'}
  schemas:
    Report:
      type: object
      deprecated: true
      example: {day: '2024-01-01', count: 3}
      properties:
        day: {type: string, format: date, writeOnly: true}
        count: {type: integer, minimum: 0, exclusiveMinimum: true, multipleOf: 1}
        tags:
          type: array
          items: {type: string, enum: [a, b]}
          minItems: 2
          maxItems: 2
          uniqueItems: true
        status:
          oneOf:
            - {$ref: '#/components/schemas/Done'}
            - {type: string, nullable: true}
        meta: {type: object, additionalProperties: {type: string}}
    Done: {type: string, enum: [done], examples: [done]}
    Level: {type: integer, enum: [1, 2, 3], default: 1, example: 2}
"#;

#[test]
fn an_operation_has_its_path_items_description_and_servers() {
    let (page, warned) = page("more", "more.yaml", MORE);
    assert_has(
        &page,
        &[
            r#"# More

- **Version:** `1`
"#,
            r#"## Report

Daily reports.
"#,
            r#"## Authentication

### `key`

An API key, in the `key` header.
"#,
            r#"## Endpoints

### `GET /reports`

Reports, by day.

**External docs:** [How reports work](<https://example.com/reports>)

**Security:** [`key`](<#key>)

**Servers:**

- `https://reports.example.com`

**Responses:**

| Status | Description | Body |
| --- | --- | --- |
| `200` | The reports | array of [`Report`](<#report-1>), as `application/json` |

### `POST /reports`

Reports, by day.

**Servers:**

- `https://write.example.com`: Where to write

**Request body:** `#/components/requestBodies/Missing`

**Responses:**

| Status | Description |
| --- | --- |
| `202` | Accepted |

**Response `202` headers:**

| Name | Type | Description |
| --- | --- | --- |
| `Location` | `string(uri)` | Where the report will be. Example: `/reports/1`. |

### Get a day's report

```http
GET /reports/{day}
```

**Path parameters:**

| Name | Type | Required | Description |
| --- | --- | --- | --- |
| `day` | `string(date)` | yes | Example: `2024-01-01`. |

**Responses:**

| Status | Description | Body |
| --- | --- | --- |
| `200` | The report | `string`, as `text/csv` or `application/xml`; [`Report`](<#report-1>), as `application/json` |

**Response `200` example**, as `text/csv`:

```
day,count
2024-01-01,3
```

**Response `200` example**, as `application/xml`: `linked`

[`https://example.com/report.xml`](<https://example.com/report.xml>)
"#,
            r#"## Schemas

### `Report`

**Deprecated.**

**Type:** `object`

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `day` | `string(date)` | no | Write-only. |
| `count` | `integer` | no | More than `0`. A multiple of `1`. |
| `tags` | array of `string` | no | Each is one of `a` or `b`. Exactly 2 items. Its items are unique. |
| `status` | [`Done`](<#done>), `string`, or `null` | no |  |
| `meta` | map of `string` | no |  |

**Example:**

```json
{
  "day": "2024-01-01",
  "count": 3
}
```

### `Done`

**Type:** `string`. Always `done`.

**Example:** `done`

### `Level`

**Type:** `integer`. One of `1`, `2`, or `3`. Defaults to `1`.

**Example:** `2`
"#,
        ],
    );
    assert_eq!(
        warned,
        [
            "can't follow `$ref: #/components/requestBodies/Missing`, in `<root>/more.yaml`: \
             `<root>/more.yaml` has nothing at `/components/requestBodies/Missing`"
        ]
    );
    for hidden in [
        "x-internal",
        "Hidden",
        "x-note",
        "Content-Type",
        "{\n  \"a\"",
    ] {
        assert!(!page.contains(hidden), "{hidden} is shown");
    }
}

const METHODS: &str = r#"openapi: 3.2.0
info: {title: Methods, version: '1'}
paths:
  /search:
    query:
      summary: Search
      responses: {'200': {description: Found}}
    additionalOperations:
      LINK:
        summary: Link
        responses: {'204': {description: Linked}}
"#;

#[test]
fn openapi_3_2s_query_and_other_methods_are_operations() {
    let (page, warned) = page("methods", "methods.yaml", METHODS);
    assert_has(
        &page,
        &[
            r#"# Methods

- **Version:** `1`
"#,
            r#"## Endpoints

### Search

```http
QUERY /search
```

**Responses:**

| Status | Description |
| --- | --- |
| `200` | Found |

### Link

```http
LINK /search
```

**Responses:**

| Status | Description |
| --- | --- |
| `204` | Linked |
"#,
        ],
    );
    assert_eq!(warned, Vec::<String>::new());
}

#[test]
fn each_link_to_a_heading_on_a_page_has_its_id() {
    let fixtures = [PETSTORE, AUTH, TAGS, PARAMETERS, BODIES, MORE, METHODS];
    let mut pages: Vec<String> = (fixtures.iter().enumerate())
        .map(|(n, text)| page(&format!("ids-{n}"), "api.yaml", text).0)
        .collect();
    let (added, _) = built(
        "ids-zoo",
        &[
            ("zoo.json", ZOO),
            ("schemas/pet.json", PET),
            ("tree.json", TREE),
        ],
    );
    pages.push(markdown(&added, "api/zoo.md").to_string());
    let mut linked = Vec::new();
    for page in pages {
        let html = html(&page, true);
        let ids: Vec<&str> = (html.split(" id=\"").skip(1))
            .map(|rest| rest.split('"').next().unwrap())
            .collect();
        // Each heading links to itself too, from its anchor.
        let links = html.split("<a href=\"#").skip(1);
        for link in links.filter(|link| !link.contains("class=\"anchor\"")) {
            let fragment = link.split('"').next().unwrap();
            assert!(ids.contains(&fragment), "no #{fragment} in:\n{html}");
            linked.push(fragment.to_string());
        }
    }
    assert_eq!(
        linked,
        [
            "api_key",
            "petstore_auth",
            "pets-1",
            "error",
            "pet",
            "pet",
            "pet",
            "key",
            "basic",
            "oauth",
            "order",
            "key",
            "report-1",
            "report-1",
            "done",
            "cat",
            "pet",
            "pet",
            "cat",
            "node"
        ]
    );
}

#[test]
fn an_email_address_is_linked_once() {
    let (page, _) = page("email", "petstore.yaml", PETSTORE);
    let html = html(&page, true);
    assert!(
        html.contains("<a href=\"mailto:support@example.com\">support@example.com</a></li>"),
        "{html}"
    );
}
