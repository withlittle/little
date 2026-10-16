use super::*;

/// A folder for the test called `test`, holding `files`, each its path in
/// the folder and what it holds.
fn folder_with(test: &str, files: &[(&str, &str)]) -> String {
    let id = std::process::id();
    let root = std::env::temp_dir().join(format!("little-jsdoc-{test}-{id}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    root.to_str().unwrap().to_string()
}

fn settings(read: &str, package: &str, url: &str) -> Settings {
    Settings {
        read: read.to_string(),
        package: package.to_string(),
        url: url.to_string(),
    }
}

/// What the plugin adds for the package called `demo`, in a folder for the
/// test called `test`, holding `files`, with its pages in `api`, and the
/// folder, which is gone by then.
fn built(test: &str, files: &[(&str, &str)]) -> (Added, String) {
    let root = folder_with(test, files);
    let added = document(&settings(&root, "demo", "api"));
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
fn settings_name_the_folder_read_its_package_and_where_the_pages_go() {
    let parsed = Settings::parse(r#"{"read":["src"],"package":"my-lib"}"#);
    assert_eq!(parsed, Ok(settings("src", "my-lib", "reference/my-lib")));
    let parsed = Settings::parse(r#"{"read":["."],"package":"@scope/lib","url":"/api/"}"#);
    assert_eq!(parsed, Ok(settings(".", "@scope/lib", "api")));
    let parsed = Settings::parse(r#"{"read":["src"],"package":"@scope/lib"}"#);
    assert_eq!(
        parsed,
        Ok(settings("src", "@scope/lib", "reference/@scope/lib"))
    );
    let parsed = Settings::parse(r#"{"read":["src"],"package":"x","url":"/"}"#);
    assert_eq!(parsed, Ok(settings("src", "x", "")));
}

#[test]
fn settings_must_read_one_folder_and_name_its_package() {
    let error = Settings::parse(r#"{"package":"x"}"#).unwrap_err();
    assert!(error.contains("`read: ./src`"), "{error}");
    let error = Settings::parse(r#"{"read":["a","b"],"package":"x"}"#).unwrap_err();
    assert!(error.contains("another entry under `plugins`"), "{error}");
    let error = Settings::parse(r#"{"read":["src"]}"#).unwrap_err();
    assert!(error.contains("`package: my-lib`"), "{error}");
    for name in [r#""""#, r#""../x""#, r#""/x""#, r#""a//b""#, "3"] {
        let json = format!(r#"{{"read":["src"],"package":{name}}}"#);
        let error = Settings::parse(&json).unwrap_err();
        assert!(error.starts_with("`package` must be"), "{name}: {error}");
    }
    let error = Settings::parse("[]").unwrap_err();
    assert!(
        error.starts_with("its settings must be a mapping"),
        "{error}"
    );
}

#[test]
fn settings_must_be_known() {
    let json = r#"{"read":["src"],"package":"x","crate":"x"}"#;
    let error = Settings::parse(json).unwrap_err();
    assert!(error.starts_with("it has no setting `crate`"), "{error}");
    for url in [r#""../api""#, r#""api/./js""#, r#""api//js""#, "3"] {
        let json = format!(r#"{{"read":["src"],"package":"x","url":{url}}}"#);
        let error = Settings::parse(&json).unwrap_err();
        assert!(error.starts_with("`url` must be"), "{url}: {error}");
    }
}

#[test]
fn relative_links_go_up_to_the_folder_two_pages_share() {
    assert_eq!(relative("api/README.md", "api/config.md"), "config.md");
    assert_eq!(
        relative("api/util/strings.md", "api/config.md"),
        "../config.md"
    );
    assert_eq!(
        relative("api/config.md", "api/util/README.md"),
        "util/README.md"
    );
    assert_eq!(relative("README.md", "a/b/c.md"), "a/b/c.md");
    assert_eq!(relative("a/b/c.md", "README.md"), "../../README.md");
    assert_eq!(relative("api/config.md", "api/config.md"), "config.md");
}

#[test]
fn each_module_with_something_documented_has_a_page() {
    let (added, root) = built(
        "walk",
        &[
            (
                "index.ts",
                "/**\n * The package.\n * @packageDocumentation\n */\n",
            ),
            (
                "config.ts",
                "/** Load it. */\nexport function load(): void {}\n",
            ),
            (
                "util/strings.js",
                "/** @module */\n\n/** Pad it. */\nexport function pad(text) {}\n",
            ),
            ("util/numbers.js", "export function add(a, b) {}\n"),
            (
                "types.d.ts",
                "/** Options. */\nexport interface Options {}\n",
            ),
            (
                "app.jsx",
                "/** @file The app. */\n\n/** Show it. */\nexport const App = () => <div />;\n",
            ),
            (
                "config.js",
                "/** Shadowed. */\nexport const shadowed = 1;\n",
            ),
            (
                "node_modules/dep/index.js",
                "/** Dep. */\nexport const dep = 1;\n",
            ),
            ("_private.ts", "/** Private. */\nexport const secret = 1;\n"),
            (".hidden/x.ts", "/** Hidden. */\nexport const x = 1;\n"),
            ("broken.ts", "/** Broken. */\nexport function broken(\n"),
            ("data.json", "{}"),
        ],
    );
    assert_eq!(
        paths(&added),
        [
            "api/README.md",
            "api/app.md",
            "api/config.md",
            "api/types.md",
            "api/util/README.md",
            "api/util/strings.md",
        ]
    );
    assert_eq!(added.warnings.len(), 1, "{:?}", added.warnings);
    let broken = format!("can't parse `{root}/broken.ts`: ");
    assert!(
        added.warnings[0].starts_with(&broken),
        "{:?}",
        added.warnings
    );
    assert!(
        added.warnings[0].ends_with(" on line 3"),
        "{:?}",
        added.warnings
    );

    let package = markdown(&added, "api/README.md");
    assert_has(
        package,
        &[
            "# demo\n\n`package`\n\nThe package.\n\n**Modules:**\n\n",
            "| [`app`](app.md) | The app. |\n",
            "| [`config`](config.md) |  |\n",
            "| [`types`](types.md) |  |\n",
            "| [`util`](util/README.md) |  |\n",
        ],
    );
    let app = markdown(&added, "api/app.md");
    assert_has(
        app,
        &[
            "# app\n\n`module`\n\nThe app.\n\n",
            "```jsx\nfunction App()\n```",
        ],
    );
    let config = markdown(&added, "api/config.md");
    assert!(
        config.contains("Load it.") && !config.contains("shadowed"),
        "{config}"
    );
    let util = markdown(&added, "api/util/README.md");
    assert_eq!(
        util,
        "# util\n\n`module`\n\n**Modules:**\n\n| Name | Description |\n| --- | --- |\n\
         | [`strings`](strings.md) |  |\n"
    );
}

#[test]
fn a_folder_with_no_code_or_nothing_documented_is_warned_of() {
    let (added, root) = built("empty", &[("README.md", "# Hi\n")]);
    assert!(added.pages.is_empty());
    let warning =
        format!("`{root}` holds no `.js`, `.ts`, or other JavaScript or TypeScript files");
    assert_eq!(added.warnings, [warning]);

    let (added, root) = built("undocumented", &[("index.js", "export const x = 1;\n")]);
    assert_eq!(paths(&added), ["api/README.md"]);
    let warning = format!("`{root}` has nothing documented: ");
    assert!(
        added.warnings[0].starts_with(&warning),
        "{:?}",
        added.warnings
    );

    let missing = document(&settings("/no/such/folder", "demo", "api"));
    assert!(missing.pages.is_empty());
    let warning = "can't read `/no/such/folder`: ";
    assert!(
        missing.warnings[0].starts_with(warning),
        "{:?}",
        missing.warnings
    );
}

const TYPESCRIPT: &str = r##"/**
 * Loading and checking a site's config.
 * @module
 */

/** The default port. */
export const PORT = 8080;

/**
 * How a site is configured.
 * @template T The type of its extra settings.
 */
export interface Config<T = unknown> extends Base {
  /** The site's name. */
  readonly name: string;
  /** Whether to print more. */
  verbose?: boolean;
  /** Load the config at `path`. */
  load(path: string): T;
  /** The site's port. */
  get port(): number;
}

/**
 * The options of {@link load}.
 */
export type Options = {
  /** Where to look. */
  root: string;
  /**
   * Whether to follow links.
   * @default false
   */
  follow?: boolean;
  /** @hidden */
  secret: string;
};

/** A level of logging. */
export enum Level {
  /** Only errors. */
  Error = 1,
  /** Everything. */
  Debug,
  'c-d' = 'x',
}

/**
 * Load a config.
 * @param path Where it is.
 * @param options How to load it.
 * @returns The config.
 * @throws {Error} If it can't be read.
 * @example
 * load("little.yml")
 */
export function load(path: string, options?: Options): Config;
/** @param paths Where they are. */
export function load(paths: string[]): Config[];
export function load(path: string | string[], options: Options = { root: "." }): Config | Config[] {
  return null!;
}

/** A loader of configs. */
export abstract class Loader<T> extends Base<T> implements Readable, Writable {
  /** How many it's loaded. */
  count = 0;
  /** @hidden */
  hidden = 1;
  private secret = 2;
  #own = 3;

  /**
   * Make a loader.
   * @param root Where to load from.
   */
  constructor(protected root: string) {
    super();
  }

  /** Load the config called `name`. */
  async load(name: string): Promise<T> {
    return null!;
  }

  /** The loader's root. */
  get where(): string {
    return this.root;
  }
  set where(value: string) {}

  /** Make a loader. */
  static create<T>(): Loader<T> {
    return null!;
  }

  /** Check a config. */
  abstract check(config: T): boolean;

  /** Called on each config. */
  onLoad = (config: T): void => {};
}

/** Parse YAML. */
export const parse = (text: string): unknown => JSON.parse(text);

/** A function. */
export default function main(): void {}

/**
 * A point.
 * @typedef {Object} Point
 * @property {number} x The x.
 * @property {number} [y] The y.
 */

/**
 * Called with each result.
 * @callback Each
 * @param {Error|null} error What went wrong.
 * @param {...string} values What it found.
 * @returns {boolean}
 */
"##;

#[test]
fn a_typescript_module_documents_what_it_exports() {
    let (added, root) = built("typescript", &[("config.ts", TYPESCRIPT)]);
    assert_eq!(added.warnings, Vec::<String>::new());
    let page = markdown(&added, "api/config.md");
    assert_has(
        page,
        &[
            "# config\n\n`module`\n\nLoading and checking a site's config.\n\n**Classes:**",
            "| [`Loader`](#loader) | A loader of configs. |\n",
            "| [`Config`](#config-1) | How a site is configured. |\n",
            "| [`Level`](#level) | A level of logging. |\n",
            "| [`PORT`](#port) | The default port. |\n",
            "| [`load`](#load) | Load a config. |\n| [`main`](#main) | A function. |\n\
             | [`parse`](#parse) | Parse YAML. |\n",
            "| [`Each`](#each) | Called with each result. |\n\
             | [`Options`](#options) | The options of [`load`](#load). |\n\
             | [`Point`](#point) | A point. |\n",
            "## `Loader`\n\n`class` `abstract`\n\n```ts\n\
             class Loader<T> extends Base<T> implements Readable, Writable {\n  \
             constructor(protected root: string);\n}\n```\n\n\
             A loader of configs.\n\nMake a loader.\n\n",
            "| `root` | `string` | Where to load from. | *required* |\n",
            "| [`count`](#loadercount) | How many it's loaded. |\n\
             | [`where`](#loaderwhere) | The loader's root. |\n",
            "| [`check`](#loadercheck) | Check a config. |\n\
             | [`create`](#loadercreate) | Make a loader. |\n\
             | [`load`](#loaderload) | Load the config called `name`. |\n\
             | [`onLoad`](#loaderonload) | Called on each config. |\n",
            &format!("<summary>Source code in <code>{root}/config.ts</code></summary>"),
            "```ts\nconstructor(protected root: string) {\n  super();\n}\n```",
            "### `Loader.count`\n\n`property`\n\n```ts\ncount = 0\n```",
            "### `Loader.where`\n\n`property`\n\n```ts\nwhere: string\n```",
            "```ts\nget where(): string {\n  return this.root;\n}\n\nset where(value: string) {}\n```",
            "### `Loader.check`\n\n`method` `abstract`\n\n```ts\ncheck(config: T): boolean\n```\n\n\
             Check a config.\n\n### `Loader.create`\n\n`method` `static`\n\n\
             ```ts\ncreate<T>(): Loader<T>\n```",
            "### `Loader.load`\n\n`method` `async`\n\n```ts\nload(name: string): Promise<T>\n```",
            "### `Loader.onLoad`\n\n`method`\n\n```ts\nonLoad(config: T): void\n```",
            "## `Config`\n\n`interface`\n\n```ts\ninterface Config<T = unknown> extends Base {\n  \
             readonly name: string;\n  verbose?: boolean;\n  load(path: string): T;\n  \
             get port(): number;\n}\n```",
            "| `T = unknown` | The type of its extra settings. |\n",
            "### `Config.port`\n\n`property` `readonly`\n\n```ts\nport: number\n```",
            "### `Config.verbose`\n\n`property` `optional`\n\n```ts\nverbose?: boolean\n```",
            "## `Level`\n\n`enum`\n\n```ts\nenum Level {\n  Error = 1,\n  Debug,\n  'c-d' = 'x',\n}\n```",
            "| `Error` | `1` | Only errors. |\n| `Debug` |  | Everything. |\n| `c-d` | `'x'` |  |\n",
            "## `PORT`\n\n`variable`\n\n```ts\nconst PORT = 8080\n```",
            "## `load`\n\n`function`\n\n```ts\nfunction load(path: string, options?: Options): Config\n\
             function load(paths: string[]): Config[]\n```",
            "| `path` | `string \\| string[]` | Where it is. | *required* |\n\
             | `options` | `Options` | How to load it. | `{ root: \".\" }` |\n",
            "| `Config \\| Config[]` | The config. |\n",
            "| `Error` | If it can't be read. |\n",
            "**Example:**\n\n```ts\nload(\"little.yml\")\n```",
            "## `main`\n\n`function` `default`\n\n```ts\nfunction main(): void\n```",
            "## `parse`\n\n`function`\n\n```ts\nfunction parse(text: string): unknown\n```",
            "## `Each`\n\n`type alias`\n\n```ts\n\
             type Each = (error: Error|null, ...values: string[]) => boolean\n```",
            "| `values` | `...string` | What it found. |  |\n",
            "## `Options`\n\n`type alias`\n\n```ts\ntype Options = {\n  root: string;\n  \
             follow?: boolean;\n}\n```\n\nThe options of [`load`](#load).\n\n",
            "| `follow?` | `boolean` | `false` | Whether to follow links. |\n",
            "## `Point`\n\n`type alias`\n\n```ts\ntype Point = {\n  x: number;\n  y?: number;\n}\n```",
            "| `y?` | `number` | The y. |\n",
        ],
    );
    for hidden in ["secret", "Loader.hidden", "#own", "Loader.own"] {
        assert!(!page.contains(hidden), "{hidden} in:\n{page}");
    }
}

const UTIL: &str = r##"/** @file Helpers for strings. */
const path = require('path');
const { load } = require('./config');

/**
 * Pad `text` to `width`.
 * @param {string} text What to pad.
 * @param {number} [width=80] How wide.
 * @param {Object} options How to pad.
 * @param {string} options.fill What to pad with.
 * @returns {string} The padded text.
 */
function pad(text, width = 80, { fill } = {}) {
  return text;
}

/** A counter. */
class Counter {
  /**
   * Make a counter.
   * @param {number} start Where to start.
   */
  constructor(start) {
    /** The count. */
    this.count = start;
  }

  /**
   * Add one.
   * @returns {number} The count.
   */
  increment() {
    return ++this.count;
  }
}

/** The version. */
const VERSION = '1.0';

module.exports = { pad, Counter, VERSION, load, path,
  /** Trim it. */
  trim: (text) => text.trim(),
};
"##;

#[test]
fn a_javascript_module_documents_jsdoc_types_and_commonjs_exports() {
    let (added, _) = built(
        "javascript",
        &[
            ("util.js", UTIL),
            (
                "config.js",
                "/** @module */\n\n/** Load it. */\nexport function load() {}\n",
            ),
            (
                "cjs/exports.js",
                "/** Add. */\nexports.add = function (a, b) { return a + b; };\n\
                 /** The zero. */\nexports.ZERO = 0;\n",
            ),
            (
                "cjs/function.js",
                "/** Main. */\nmodule.exports = function main() {};\n",
            ),
            (
                "cjs/class.js",
                "/** A thing. */\nmodule.exports = class {};\n",
            ),
            (
                "cjs/app.js",
                "/** The app. */\nvar app = exports = module.exports = {};\n\
                 /** Use it. */\napp.use = function use(fn) {};\n\
                 /** Get a header. */\napp.get = app.header = function header(name) {};\n\
                 /** @private */\napp.init = function init() {};\n",
            ),
            (
                "cjs/make.js",
                "/** Make one. */\nfunction make() {}\n\
                 /** Its version. */\nmake.version = '1';\n\
                 exports = module.exports = make;\n",
            ),
        ],
    );
    assert_eq!(added.warnings, Vec::<String>::new());
    assert_eq!(
        paths(&added),
        [
            "api/README.md",
            "api/cjs/README.md",
            "api/cjs/app.md",
            "api/cjs/class.md",
            "api/cjs/exports.md",
            "api/cjs/function.md",
            "api/cjs/make.md",
            "api/config.md",
            "api/util.md",
        ]
    );
    let app = markdown(&added, "api/cjs/app.md");
    assert_has(
        app,
        &[
            "| [`app`](#app-1) | The app. |\n",
            "| [`get`](#get) | Get a header. |\n| [`use`](#use) | Use it. |\n",
            "## `app`\n\n`variable` `default`\n\n```js\nvar app\n```\n\nThe app.\n",
            "## `get`\n\n`function`\n\n```js\nfunction get(name)\n```\n\nGet a header.\n",
            "## `use`\n\n`function`\n\n```js\nfunction use(fn)\n```\n\nUse it.\n",
        ],
    );
    assert!(!app.contains("header`") && !app.contains("init"), "{app}");
    assert_has(
        markdown(&added, "api/cjs/make.md"),
        &[
            "## `version`\n\n`variable`\n\n```js\nmake.version = '1'\n```\n\nIts version.\n",
            "## `make`\n\n`function` `default`\n\n```js\nfunction make()\n```\n\nMake one.\n",
        ],
    );
    assert_has(
        markdown(&added, "api/util.md"),
        &[
            "# util\n\n`module`\n\nHelpers for strings.\n\n**Re-exports:**",
            "| [`load`](config.md#load) | Load it. |\n| `path` |  |\n",
            "| [`Counter`](#counter) | A counter. |\n",
            "| [`VERSION`](#version) | The version. |\n",
            "| [`pad`](#pad) | Pad `text` to `width`. |\n| [`trim`](#trim) | Trim it. |\n",
            "## `Counter`\n\n`class`\n\n```js\nclass Counter {\n  constructor(start);\n}\n```\n\n\
             A counter.\n\nMake a counter.\n\n",
            "| `start` | `number` | Where to start. | *required* |\n",
            "### `Counter.increment`\n\n`method`\n\n```js\nincrement()\n```\n\nAdd one.\n\n",
            "| `number` | The count. |\n",
            "## `VERSION`\n\n`variable`\n\n```js\nconst VERSION = '1.0'\n```",
            "## `pad`\n\n`function`\n\n```js\nfunction pad(text, width = 80, { fill } = {})\n```",
            "| `text` | `string` | What to pad. | *required* |\n\
             | `width` | `number` | How wide. | `80` |\n\
             | `options` | `Object` | How to pad. | `{}` |\n\
             | `options.fill` | `string` | What to pad with. |  |\n",
            "| `string` | The padded text. |\n",
            "## `trim`\n\n`function`\n\n```js\nfunction trim(text)\n```\n\nTrim it.\n\n",
            "```js\ntrim: (text) => text.trim()\n```",
        ],
    );
    assert_has(
        markdown(&added, "api/cjs/exports.md"),
        &[
            "## `ZERO`\n\n`variable`\n\n```js\nexports.ZERO = 0\n```\n\nThe zero.\n\n\
             ## `add`\n\n`function`\n\n```js\nfunction add(a, b)\n```",
            "```js\nexports.add = function (a, b) { return a + b; };\n```",
        ],
    );
    assert_has(
        markdown(&added, "api/cjs/function.md"),
        &["## `main`\n\n`function` `default`\n\n```js\nfunction main()\n```\n\nMain.\n\n"],
    );
    assert_has(
        markdown(&added, "api/cjs/class.md"),
        &[
            "| [`default`](#default) | A thing. |\n",
            "## `default`\n\n`class`\n\n```js\nclass default\n```\n\nA thing.\n",
        ],
    );
}

#[test]
fn a_default_export_is_called_default_or_labelled_so() {
    let (added, _) = built(
        "defaults",
        &[
            (
                "anonymous.js",
                "/** Run it. */\nexport default function () {}\n",
            ),
            (
                "named.ts",
                "/** A widget. */\nexport default class Widget {}\n",
            ),
            (
                "identifier.js",
                "/** The answer. */\nconst answer = 42;\nexport default answer;\n",
            ),
            (
                "value.js",
                "/** The settings. */\nexport default { a: 1 };\n",
            ),
        ],
    );
    assert_eq!(added.warnings, Vec::<String>::new());
    assert_has(
        markdown(&added, "api/anonymous.md"),
        &["## `default`\n\n`function`\n\n```js\nfunction default()\n```\n\nRun it.\n\n"],
    );
    assert_has(
        markdown(&added, "api/named.md"),
        &["## `Widget`\n\n`class` `default`\n\n```ts\nclass Widget\n```\n\nA widget.\n"],
    );
    assert_has(
        markdown(&added, "api/identifier.md"),
        &["## `answer`\n\n`variable` `default`\n\n```js\nconst answer = 42\n```\n\nThe answer.\n"],
    );
    assert_eq!(
        markdown(&added, "api/value.md"),
        "# value\n\n`module`\n\n**Variables:**\n\n| Name | Description |\n| --- | --- |\n\
         | [`default`](#default) | The settings. |\n\n## `default`\n\n`variable`\n\nThe settings.\n"
    );
}

#[test]
fn a_barrel_lists_what_it_exports_from_other_modules() {
    let (added, _) = built(
        "barrels",
        &[
            (
                "index.ts",
                "/**\n * Everything.\n * @packageDocumentation\n */\n\
                 export * from './a';\nexport { b as bee } from './b';\n\
                 export * as c from './c';\n",
            ),
            ("a.ts", "/** A. */\nexport const a = 1;\n"),
            ("b.ts", "/** B. */\nexport function b() {}\n"),
            ("c.ts", "/** C. */\nexport class C {}\n"),
        ],
    );
    assert_eq!(added.warnings, Vec::<String>::new());
    assert_has(
        markdown(&added, "api/README.md"),
        &[
            "# demo\n\n`package`\n\nEverything.\n\n**Modules:**",
            "**Re-exports:**\n\n| Name | Description |\n| --- | --- |\n\
             | [`* from './a'`](a.md) |  |\n| [`bee`](b.md#b-1) | B. |\n| [`c`](c.md) |  |\n",
        ],
    );
}

#[test]
fn links_lead_to_what_they_name_wherever_it_is() {
    let (added, _) = built(
        "links",
        &[
            (
                "links/index.ts",
                "/**\n * See {@link Thing}, {@link Thing#run}, {@link helper}, {@link shout},\n \
                 * {@link module:links/other}, {@link module:demo/links/other.shout},\n \
                 * {@link Gizmo#spin}, {@link Gizmo.spin()}, and {@link nowhere}.\n \
                 * @module\n */\nimport { helper } from './other';\n\
                 export { shout } from './other';\n\n\
                 /** A thing. */\nexport class Thing {\n  /** Run it. */\n  run() {}\n}\n",
            ),
            (
                "links/other.ts",
                "/** @module */\n\n/** Help. */\nexport function helper() {}\n\n\
                 /** Shout. */\nexport function shout() {}\n\n\
                 /** A gizmo. */\nexport class Gizmo {\n  /** Spin it. */\n  spin() {}\n}\n",
            ),
        ],
    );
    assert_eq!(added.warnings, Vec::<String>::new());
    assert_has(
        markdown(&added, "api/links/README.md"),
        &[
            "See [`Thing`](#thing), [`Thing#run`](#thingrun), [`helper`](other.md#helper), \
           [`shout`](other.md#shout),\n[`module:links/other`](other.md), \
           [`module:demo/links/other.shout`](other.md#shout),\n\
           [`Gizmo#spin`](other.md#gizmospin), [`Gizmo.spin()`](other.md#gizmospin), \
           and `nowhere`.\n",
        ],
    );
    assert_has(
        markdown(&added, "api/README.md"),
        &[
            "| [`links`](links/README.md) | See [`Thing`](links/README.md#thing), \
           [`Thing#run`](links/README.md#thingrun), [`helper`](links/other.md#helper), ",
        ],
    );
}

#[test]
fn a_script_documents_whatever_it_declares_with_a_doc_comment() {
    let (added, _) = built(
        "scripts",
        &[(
            "script.js",
            "/** Greet. */\nfunction greet() {}\n\n/** The count. */\nvar count = 1;\n\n\
             function undocumented() {}\n",
        )],
    );
    assert_eq!(added.warnings, Vec::<String>::new());
    let page = markdown(&added, "api/script.md");
    assert_has(
        page,
        &[
            "| [`count`](#count) | The count. |\n",
            "| [`greet`](#greet) | Greet. |\n",
            "## `count`\n\n`variable`\n\n```js\nvar count = 1\n```\n\nThe count.\n\n",
            "## `greet`\n\n`function`\n\n```js\nfunction greet()\n```\n\nGreet.\n\n",
        ],
    );
    assert!(!page.contains("undocumented"), "{page}");
}

#[test]
fn whats_private_internal_ignored_or_hidden_is_left_out() {
    let (added, _) = built(
        "hidden",
        &[(
            "hidden.ts",
            "/** Shown. */\nexport function shown() {}\n/** @private */\nexport function a() {}\n\
             /** @internal */\nexport function b() {}\n/** @ignore */\nexport function c() {}\n\
             /** @access private */\nexport function d() {}\n/** @hidden */\nexport function e() {}\n",
        )],
    );
    assert_eq!(added.warnings, Vec::<String>::new());
    let page = markdown(&added, "api/hidden.md");
    assert_has(page, &["| [`shown`](#shown) | Shown. |\n\n## `shown`"]);
    assert_eq!(page.matches("\n## ").count(), 1, "{page}");
}

#[test]
fn a_folder_without_an_index_shows_the_module_beside_it() {
    let (added, _) = built(
        "folders",
        &[
            (
                "foo.ts",
                "/**\n * Foo.\n * @module\n */\n\n/** One. */\nexport const one = 1;\n",
            ),
            ("foo/bar.ts", "/** Bar. */\nexport const bar = 2;\n"),
        ],
    );
    assert_eq!(added.warnings, Vec::<String>::new());
    assert_eq!(
        paths(&added),
        ["api/README.md", "api/foo/README.md", "api/foo/bar.md"]
    );
    assert_has(
        markdown(&added, "api/README.md"),
        &["| [`foo`](foo/README.md) | Foo. |\n"],
    );
    assert_has(
        markdown(&added, "api/foo/README.md"),
        &[
            "# foo\n\n`module`\n\nFoo.\n\n**Modules:**",
            "| [`bar`](bar.md) |  |\n",
            "## `one`\n\n`variable`\n\n```ts\nconst one = 1\n```",
        ],
    );
}

#[test]
fn a_declaration_file_documents_what_it_declares() {
    let (added, _) = built(
        "declarations",
        &[(
            "index.d.ts",
            "/**\n * Load it.\n * @param path Where.\n */\n\
             export declare function load(path: string): void;\n\
             /** A. */\nexport declare const a: number;\n\
             /** Two forms. */\nexport function two(a: string): void;\n\
             export function two(a: number): void;\n\
             /** A widget. */\nexport declare class Widget {\n  /** Make one. */\n  \
             constructor(size: number);\n  /** Grow it. */\n  grow(by?: number): this;\n}\n",
        )],
    );
    assert_eq!(added.warnings, Vec::<String>::new());
    assert_has(
        markdown(&added, "api/README.md"),
        &[
            "## `Widget`\n\n`class`\n\n```ts\nclass Widget {\n  constructor(size: number);\n}\n```\n\n\
             A widget.\n\nMake one.\n\n",
            "### `Widget.grow`\n\n`method`\n\n```ts\ngrow(by?: number): this\n```\n\nGrow it.\n",
            "## `a`\n\n`variable`\n\n```ts\nconst a: number\n```\n\nA.\n",
            "## `load`\n\n`function`\n\n```ts\nfunction load(path: string): void\n```\n\nLoad it.\n\n",
            "| `path` | `string` | Where. | *required* |\n",
            "## `two`\n\n`function`\n\n```ts\nfunction two(a: string): void\n\
             function two(a: number): void\n```\n\nTwo forms.\n",
        ],
    );
}
