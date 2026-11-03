use super::*;

/// A folder for the test called `test`, holding `files`, each its path in
/// the folder and what it holds.
fn folder_with(test: &str, files: &[(&str, &str)]) -> String {
    let id = std::process::id();
    let root = std::env::temp_dir().join(format!("little-godoc-{test}-{id}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    root.to_str().unwrap().to_string()
}

fn settings(read: &str, url: Option<&str>) -> Settings {
    Settings {
        read: read.to_string(),
        title: None,
        url: url.map(String::from),
    }
}

/// What the plugin adds for the module `example.com/demo`, in a folder for
/// the test called `test`, holding its `go.mod` and `files`, with its pages
/// in `api`, and the folder, which is gone by then.
fn built(test: &str, files: &[(&str, &str)]) -> (Added, String) {
    let go_mod = ("go.mod", "module example.com/demo\n\ngo 1.22\n");
    let root = folder_with(test, &[&[go_mod], files].concat());
    let added = document(&settings(&root, Some("api"))).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    (added, root)
}

/// The paths of the pages `added` adds, in order.
fn paths(added: &Added) -> Vec<&str> {
    let mut paths: Vec<_> = added.pages.iter().map(|(path, _)| path.as_str()).collect();
    paths.sort();
    paths
}

fn markdown<'a>(added: &'a Added, path: &str) -> &'a str {
    let page = added.pages.iter().find(|(page, _)| page == path);
    &page.unwrap_or_else(|| panic!("no page at {path}")).1
}

/// Assert that `page` has each of `parts`, in order.
#[track_caller]
fn assert_has(page: &str, parts: &[&str]) {
    let mut rest = page;
    for part in parts {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => panic!("no {part:?}, in order, in:\n{page}"),
        }
    }
}

#[test]
fn settings_name_the_modules_folder_its_title_and_where_its_pages_go() {
    assert_eq!(
        Settings::parse(r#"{"read":["."]}"#),
        Ok(settings(".", None))
    );
    let parsed = Settings::parse(r#"{"read":["./go"],"url":"/api/"}"#);
    assert_eq!(parsed, Ok(settings("./go", Some("api"))));
    let parsed = Settings::parse(r#"{"read":["."],"url":"/"}"#);
    assert_eq!(parsed, Ok(settings(".", Some(""))));
    let parsed = Settings::parse(r#"{"read":["."],"title":"API"}"#);
    let titled = Settings {
        title: Some("API".to_string()),
        ..settings(".", None)
    };
    assert_eq!(parsed, Ok(titled));
}

#[test]
fn settings_must_read_one_folder() {
    for json in ["{}", r#"{"read":[]}"#] {
        let error = Settings::parse(json).unwrap_err();
        assert!(error.contains("like `read: .`"), "{json}: {error}");
    }
    let error = Settings::parse(r#"{"read":["a","b"]}"#).unwrap_err();
    assert!(error.starts_with("it documents one module"), "{error}");
    let error = Settings::parse("[]").unwrap_err();
    assert!(
        error.starts_with("its settings must be a mapping"),
        "{error}"
    );
}

#[test]
fn settings_must_be_known() {
    let error = Settings::parse(r#"{"read":["."],"package":"x"}"#).unwrap_err();
    assert!(
        error.starts_with("it has no setting `package`, only `read`"),
        "{error}"
    );
    for url in [r#""../api""#, r#""api/./go""#, r#""api//go""#, "3"] {
        let json = format!(r#"{{"read":["."],"url":{url}}}"#);
        let error = Settings::parse(&json).unwrap_err();
        assert!(error.starts_with("`url` must be"), "{url}: {error}");
    }
    for title in [r#""""#, r#"" ""#, "3", "null"] {
        let json = format!(r#"{{"read":["."],"title":{title}}}"#);
        let error = Settings::parse(&json).unwrap_err();
        assert!(error.starts_with("`title` must be"), "{title}: {error}");
    }
}

#[test]
fn the_modules_path_is_its_go_mods_module_line() {
    let path = |path: &str| Some(path.to_string());
    assert_eq!(
        module_line("module example.com/demo\n"),
        path("example.com/demo")
    );
    assert_eq!(
        module_line("// The demo.\n\nmodule example.com/demo/v2 // its path\n\ngo 1.22\n"),
        path("example.com/demo/v2")
    );
    assert_eq!(
        module_line("module\t\"example.com/demo\"\r\n"),
        path("example.com/demo")
    );
    assert_eq!(
        module_line("module `example.com/demo`"),
        path("example.com/demo")
    );
    assert_eq!(module_line("module demo"), path("demo"));
    for go_mod in [
        "go 1.22\n",
        "module\n",
        "modules example.com/demo\n",
        "// module example.com/demo\n",
        "module (\n\texample.com/demo\n)\n",
        "module example.com/../demo\n",
        "module example.com/demo/\n",
        "module example.com/de mo\n",
        "module \"example.com/demo\n",
    ] {
        assert_eq!(module_line(go_mod), None, "{go_mod:?}");
    }
}

#[test]
fn a_module_needs_a_go_mod_that_names_its_path() {
    let root = folder_with("go-mod", &[("a.go", "// Package a is a.\npackage a\n")]);
    let error = document(&settings(&root, None)).unwrap_err();
    assert_eq!(
        error,
        format!(
            "`{root}` has no `go.mod`: `read` names the folder of a Go module, with its `go.mod`"
        )
    );
    std::fs::write(format!("{root}/go.mod"), "go 1.22\n").unwrap();
    let error = document(&settings(&root, None)).unwrap_err();
    assert_eq!(
        error,
        format!(
            "`{root}/go.mod` doesn't name the module's path in a `module` line, like \
             `module example.com/m`"
        )
    );
    std::fs::remove_file(format!("{root}/go.mod")).unwrap();
    std::fs::create_dir(format!("{root}/go.mod")).unwrap();
    let error = document(&settings(&root, None)).unwrap_err();
    std::fs::remove_dir_all(&root).unwrap();
    assert!(
        error.starts_with(&format!("can't read `{root}/go.mod`: ")),
        "{error}"
    );
}

#[test]
fn the_pages_go_in_a_folder_named_for_the_module_unless_url_says() {
    let files = [
        ("go.mod", "module example.com/demo/v2\n"),
        ("a.go", "// Package demo runs things.\npackage demo\n"),
    ];
    let root = folder_with("url", &files);
    let named = document(&settings(&root, None)).unwrap();
    let at_root = document(&settings(&root, Some(""))).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(paths(&named), ["demo/README.md"]);
    assert_eq!(paths(&at_root), ["README.md"]);
}

#[test]
fn each_package_with_something_documented_has_a_page_as_does_each_folder_its_in() {
    let (added, _) = built(
        "pages",
        &[
            ("doc.go", "// Package demo runs things.\npackage demo\n"),
            (
                "config/config.go",
                "// Package config configures it.\npackage config\n",
            ),
            (
                "config/config_test.go",
                "// Package config is tested.\npackage config\n",
            ),
            (
                "cmd/demo/main.go",
                "// Demo runs things, from the command line.\npackage main\n\n// Run runs.\nfunc Run() {}\n",
            ),
            (
                "encoding/json/json.go",
                "package json\n\n// Encode encodes.\nfunc Encode() {}\n",
            ),
            ("encoding/xml/xml.go", "package xml\n\nfunc Encode() {}\n"),
            (
                "internal/secret/secret.go",
                "// Package secret is kept within the module.\npackage secret\n",
            ),
            (
                "config/testdata/data.go",
                "// Package data is data.\npackage data\n",
            ),
            (
                "vendor/example.com/x/x.go",
                "// Package x is vendored.\npackage x\n",
            ),
            ("tools/go.mod", "module example.com/demo/tools\n"),
            (
                "tools/tools.go",
                "// Package tools is a module of its own.\npackage tools\n",
            ),
            ("_examples/x.go", "// Package x is left out.\npackage x\n"),
            (".git/x.go", "// Package x is hidden.\npackage x\n"),
            ("docs/guide.md", "# Guide\n"),
        ],
    );
    assert_eq!(
        paths(&added),
        [
            "api/README.md",
            "api/cmd/README.md",
            "api/cmd/demo/README.md",
            "api/config/README.md",
            "api/encoding/README.md",
            "api/encoding/json/README.md",
            "api/internal/README.md",
            "api/internal/secret/README.md",
        ]
    );
    assert!(added.warnings.is_empty(), "{:?}", added.warnings);
    assert_eq!(
        markdown(&added, "api/README.md"),
        "# demo

`package`

```go
import \"example.com/demo\"
```

Package demo runs things.

**Packages:**

| Name | Description |
| --- | --- |
| [`cmd`](cmd/README.md) |  |
| [`config`](config/README.md) | Package config configures it. |
| [`encoding`](encoding/README.md) |  |
| [`internal`](internal/README.md) |  |
"
    );
    assert_eq!(
        markdown(&added, "api/cmd/README.md"),
        "# cmd

`directory`

**Packages:**

| Name | Description |
| --- | --- |
| [`demo`](demo/README.md) | Demo runs things, from the command line. |
"
    );
    assert_eq!(
        markdown(&added, "api/cmd/demo/README.md"),
        "# demo\n\n`package` `command`\n\nDemo runs things, from the command line.\n"
    );
    assert_has(
        markdown(&added, "api/encoding/json/README.md"),
        &[
            "# json\n\n`package`\n\n```go\nimport \"example.com/demo/encoding/json\"\n```\n\n",
            "| [`Encode`](#encode) | Encode encodes. |\n",
            "## `Encode`\n\n`function`\n\n```go\nfunc Encode()\n```\n\nEncode encodes.\n\n",
        ],
    );
}

#[test]
fn a_module_with_no_package_of_its_own_is_titled_with_its_path() {
    let (added, _) = built("no-root", &[("a/a.go", "// Package a is a.\npackage a\n")]);
    assert_eq!(paths(&added), ["api/README.md", "api/a/README.md"]);
    assert_eq!(
        markdown(&added, "api/README.md"),
        "# example.com/demo

`module`

**Packages:**

| Name | Description |
| --- | --- |
| [`a`](a/README.md) | Package a is a. |
"
    );
}

#[test]
fn a_command_at_the_modules_root_is_named_for_the_module() {
    let main = "// Demo runs things.\npackage main\n\nfunc main() {}\n";
    let (added, _) = built("command", &[("main.go", main)]);
    assert_eq!(
        markdown(&added, "api/README.md"),
        "# demo\n\n`package` `command`\n\nDemo runs things.\n"
    );
}

#[test]
fn a_module_with_nothing_documented_is_warned_of() {
    let (added, root) = built(
        "undocumented",
        &[
            ("a.go", "package demo\n\nfunc A() {}\n"),
            ("b/b.go", "package b\n\ntype B int\n"),
        ],
    );
    assert_eq!(paths(&added), ["api/README.md"]);
    assert_eq!(
        markdown(&added, "api/README.md"),
        "# demo\n\n`package`\n\n```go\nimport \"example.com/demo\"\n```\n"
    );
    assert_eq!(
        added.warnings,
        [format!(
            "`{root}` has nothing documented: no doc comment on a package, or on what one exports"
        )]
    );
}

#[test]
fn a_module_with_no_go_files_is_warned_of() {
    let (added, root) = built(
        "empty",
        &[("README.md", "# Demo\n"), ("a_test.go", "package demo\n")],
    );
    assert!(added.pages.is_empty());
    assert_eq!(
        added.warnings,
        [format!("`{root}` holds no `.go` files, other than tests")]
    );
}

#[test]
fn a_packages_page_shows_how_its_imported_and_what_it_exports_by_kind() {
    let (added, root) = built(
        "page",
        &[(
            "shapes/shapes.go",
            "// Package shapes draws shapes.
package shapes

// Sides is how many sides a square has.
const Sides = 4

// The kinds of shape.
const (
\tSquare Kind = iota
\tCircle
)

// A Kind is a kind of shape.
type Kind int

// String names it.
func (k Kind) String() string { return \"\" }

// A Shape is drawn.
type Shape interface {
\t// Area is its area.
\tArea() float64
\tperimeter() float64
}

// A Box holds a shape.
type Box struct {
\t// Shape is what it holds.
\tShape Shape
\tlabel string
}

// Draw draws it.
//
// Deprecated: Use [Shape.Area].
func Draw(s Shape)
",
        )],
    );
    let page = markdown(&added, "api/shapes/README.md");
    let source = format!("{root}/shapes/shapes.go");
    assert_has(
        page,
        &[
            "# shapes

`package`

```go
import \"example.com/demo/shapes\"
```

Package shapes draws shapes.

**Constants:**

| Name | Description |
| --- | --- |
| [`Circle`](#circle) | The kinds of shape. |
| [`Sides`](#sides) | Sides is how many sides a square has. |
| [`Square`](#square) | The kinds of shape. |

**Functions:**

| Name | Description |
| --- | --- |
| [`Draw`](#draw) | Draw draws it. |

**Types:**

| Name | Description |
| --- | --- |
| [`Box`](#box) | A Box holds a shape. |
| [`Kind`](#kind) | A Kind is a kind of shape. |
| [`Shape`](#shape) | A Shape is drawn. |

## `Circle`

`constant`

```go
const Circle Kind
```

The kinds of shape.

",
            "## `Square`\n\n`constant`\n\n```go\nconst Square Kind = iota\n```\n\n",
            "## `Draw`

`function` `deprecated`

```go
func Draw(s Shape)
```

Draw draws it.

Deprecated\\: Use [`Shape.Area`](<#shapearea>).

## `Box`

`type`

```go
type Box struct {
\tShape Shape
\t// contains filtered or unexported fields
}
```

A Box holds a shape.

**Fields:**

| Name | Type | Description |
| --- | --- | --- |
| `Shape` | `Shape` | Shape is what it holds. |

",
            &format!("<summary>Source code in <code>{source}</code></summary>\n\n"),
            "```go
type Box struct {
\t// Shape is what it holds.
\tShape Shape
\tlabel string
}
```",
            "## `Kind`

`type`

```go
type Kind int
```

A Kind is a kind of shape.

**Methods:**

| Name | Description |
| --- | --- |
| [`String`](#kindstring) | String names it. |

### `Kind.String`

`method`

```go
func (k Kind) String() string
```

String names it.

",
            "```go\nfunc (k Kind) String() string { return \"\" }\n```",
            "## `Shape`

`type`

```go
type Shape interface {
\tArea() float64
\t// contains filtered or unexported methods
}
```

A Shape is drawn.

**Methods:**

| Name | Description |
| --- | --- |
| [`Area`](#shapearea) | Area is its area. |

",
            "### `Shape.Area`\n\n`method`\n\n```go\nArea() float64\n```\n\nArea is its area.\n",
        ],
    );
    let shape = &page[page.find("## `Shape`").unwrap()..];
    assert_has(
        shape,
        &["<summary>Source code in", "\tperimeter() float64\n"],
    );
}

#[test]
fn links_lead_to_what_they_name_in_the_module_and_are_code_outside_it() {
    let (added, _) = built(
        "links",
        &[
            (
                "doc.go",
                "// Package demo runs things, as [example.com/demo/config] and
// [example.com/demo/config.Config.Load] say, or [Run].
package demo

// Run runs.
func Run() {}
",
            ),
            (
                "config/config.go",
                "// Package config configures [demo.Run]. See [Config], [Config.Path],
// [json.Encode], [fmt.Println], [Undocumented], and [example.com/demolition.Thing].
package config

import (
\t\"fmt\"

\t\"example.com/demo\"
\t\"example.com/demo/encoding/json\"
)

// A Config is read by [Config.Load].
type Config struct {
\t// Path is where it's read from, by [Config.Load].
\tPath string
}

// Load loads it.
func (c *Config) Load() error { return nil }

type Undocumented int
",
            ),
            (
                "encoding/json/json.go",
                "package json\n\n// Encode encodes.\nfunc Encode() {}\n",
            ),
        ],
    );
    assert_has(
        markdown(&added, "api/README.md"),
        &[
            "Package demo runs things, as [`example.com/demo/config`](<config/README.md>) and \
             [`example.com/demo/config.Config.Load`](<config/README.md#configload>) say, or \
             [`Run`](<#run>).",
            "| [`config`](config/README.md) | Package config configures [`demo.Run`](<#run>). \
             See [`Config`](<config/README.md#config-1>),",
        ],
    );
    assert_has(
        markdown(&added, "api/config/README.md"),
        &[
            "Package config configures [`demo.Run`](<../README.md#run>). See \
             [`Config`](<#config-1>), [`Config.Path`](<#config-1>), \
             [`json.Encode`](<../encoding/json/README.md#encode>), `fmt.Println`, \
             `Undocumented`, and `example.com/demolition.Thing`.",
            "| [`Config`](#config-1) | A Config is read by [`Config.Load`](<#configload>). |",
            "| `Path` | `string` | Path is where it's read from, by \
             [`Config.Load`](<#configload>). |",
        ],
    );
}

#[test]
fn what_cant_be_read_is_warned_of_and_left_out() {
    let (added, root) = built(
        "warnings",
        &[
            ("a.go", "// Package demo runs things.\npackage demo\n"),
            ("b.go", "package demo\n\n// B is b.\nconst B = 1\n"),
            ("broken.go", "package demo\n\nfunc F( {\n}\n"),
            ("gen.go", "//go:build ignore\n\npackage main\n"),
            ("tools.go", "//go:build tools\n\npackage tools\n"),
        ],
    );
    assert_eq!(paths(&added), ["api/README.md"]);
    assert_has(markdown(&added, "api/README.md"), &["## `B`"]);
    assert_eq!(added.warnings.len(), 2, "{:?}", added.warnings);
    assert!(
        added.warnings[0].starts_with(&format!("can't parse `{root}/broken.go`: ")),
        "{}",
        added.warnings[0]
    );
    assert_eq!(
        added.warnings[1],
        format!(
            "`{root}` holds files of more than one package, `demo` and `tools`; only `demo` is \
             documented"
        )
    );
}

#[test]
fn a_title_is_in_the_frontmatter_of_the_modules_page_alone() {
    let files = [
        ("go.mod", "module example.com/demo\n"),
        ("a.go", "// Package demo runs things.\npackage demo\n"),
        (
            "config/config.go",
            "// Package config configures it.\npackage config\n",
        ),
    ];
    let root = folder_with("title", &files);
    let mut settings = settings(&root, Some("api"));
    settings.title = Some("API: \"demo\"".to_string());
    let added = document(&settings).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    let module = markdown(&added, "api/README.md");
    let frontmatter = "---\ntitle: \"API: \\\"demo\\\"\"\n---\n\n# demo\n";
    assert!(module.starts_with(frontmatter), "{module}");
    let config = markdown(&added, "api/config/README.md");
    assert!(config.starts_with("# config\n"), "{config}");
}
