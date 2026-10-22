use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Value, json};

use super::{Part, as_shown, check_settings, read};

/// The site's `markdown` settings, as little gives them, with raw HTML let
/// in, as little lets it in by default.
const RAW_HTML: &str = r#"{ "raw_html": true }"#;

/// The site's `markdown` settings, with raw HTML left out.
const NO_RAW_HTML: &str = r#"{ "raw_html": false }"#;

/// A notebook of `cells`, with Python's metadata, as Jupyter saves it.
fn notebook(cells: Value) -> String {
    json!({
        "cells": cells,
        "metadata": {
            "kernelspec": {
                "display_name": "Python 3",
                "language": "python",
                "name": "python3"
            },
            "language_info": { "name": "python", "version": "3.12.4" }
        },
        "nbformat": 4,
        "nbformat_minor": 5
    })
    .to_string()
}

/// The parts of a notebook of `cells`, read with `markdown`, but the first,
/// its frontmatter, which is empty.
fn parts_with(cells: Value, markdown: &str) -> Vec<Part> {
    let mut parts = read(&notebook(cells), markdown).unwrap();
    assert_eq!(parts.remove(0), md(""), "the frontmatter");
    parts
}

/// The parts of a notebook of `cells`, with raw HTML let in, but its
/// frontmatter.
fn parts(cells: Value) -> Vec<Part> {
    parts_with(cells, RAW_HTML)
}

fn markdown(source: &str) -> Value {
    json!({ "cell_type": "markdown", "id": "a1", "metadata": {}, "source": source })
}

fn code(source: &str, outputs: Value) -> Value {
    json!({
        "cell_type": "code",
        "execution_count": 1,
        "id": "b2",
        "metadata": {},
        "outputs": outputs,
        "source": source
    })
}

fn raw(source: &str) -> Value {
    json!({ "cell_type": "raw", "metadata": {}, "source": source })
}

fn stream(name: &str, text: &str) -> Value {
    json!({ "output_type": "stream", "name": name, "text": text })
}

fn display(data: Value) -> Value {
    json!({ "output_type": "display_data", "data": data, "metadata": {} })
}

fn md(markdown: &str) -> Part {
    Part::Markdown(markdown.to_string())
}

fn in_language(language: &str, text: &str) -> Part {
    Part::Code {
        language: language.to_string(),
        text: text.to_string(),
    }
}

fn python(code: &str) -> Part {
    in_language("python", code)
}

fn output(text: &str) -> Part {
    in_language("output", text)
}

#[test]
fn cells_and_outputs_are_parts() {
    let cells = json!([
        markdown("What the data says."),
        code(
            "print(\"Hello\")\nx = 6 * 7\nx\n",
            json!([
                stream("stdout", "Hello\n"),
                {
                    "output_type": "execute_result",
                    "data": { "text/plain": "42" },
                    "execution_count": 1,
                    "metadata": {}
                }
            ])
        ),
    ]);
    assert_eq!(
        parts(cells),
        [
            md("What the data says."),
            python("print(\"Hello\")\nx = 6 * 7\nx"),
            output("Hello"),
            output("42"),
        ]
    );
}

#[test]
fn text_is_a_string_or_a_list_of_lines() {
    let lines = json!({
        "cell_type": "markdown",
        "metadata": {},
        "source": ["# Analysis\n", "\n", "What the data says."]
    });
    assert_eq!(
        parts(json!([lines])),
        [md("# Analysis\n\nWhat the data says.")]
    );
}

#[test]
fn each_markdown_cell_is_a_part_of_its_own() {
    // So little reads each on its own, as Jupyter shows it: a code block
    // left open ends with its cell.
    let cells = json!([markdown("```python\nprint(1)"), markdown("After")]);
    assert_eq!(parts(cells), [md("```python\nprint(1)"), md("After")]);
}

#[test]
fn code_is_in_the_notebooks_language() {
    let language = |metadata: Value| {
        let notebook = json!({
            "cells": [code("x <- 1", json!([]))],
            "metadata": metadata,
            "nbformat": 4,
            "nbformat_minor": 2
        });
        let mut parts = read(&notebook.to_string(), RAW_HTML).unwrap();
        match parts.pop() {
            Some(Part::Code { language, text }) if text == "x <- 1" => language,
            part => panic!("{part:?}"),
        }
    };
    // As its language says, or else its kernel.
    let language_info =
        json!({ "language_info": { "name": "R" }, "kernelspec": { "language": "python" } });
    assert_eq!(language(language_info), "R");
    assert_eq!(language(json!({ "kernelspec": { "language": "R" } })), "R");
    assert_eq!(language(json!({})), "");
}

#[test]
fn an_empty_code_cell_is_left_out() {
    let cells = json!([code("\n  \n", json!([stream("stdout", "Hi\n")]))]);
    assert_eq!(parts(cells), [output("Hi")]);
}

#[test]
fn a_stream_is_shown_as_a_terminal_shows_it() {
    let cells = json!([code(
        "train()",
        json!([
            stream("stdout", "Epoch 1\n"),
            // A progress bar, drawn over its line, and in colour.
            stream("stdout", "10%\r50%\r100%\r\n\x1b[32mDone\x1b[0m\r\n"),
            stream("stderr", "\x1b[1;31mWarning\x1b[0m: slow\n"),
            stream("stdout", "\n"),
        ])
    )]);
    assert_eq!(
        parts(cells),
        [
            python("train()"),
            output("Epoch 1\n100%\nDone"),
            output("Warning: slow"),
        ]
    );
}

#[test]
fn an_error_shows_its_traceback() {
    let error = |traceback: Value| {
        let error = json!({
            "output_type": "error",
            "ename": "ZeroDivisionError",
            "evalue": "division by zero",
            "traceback": traceback
        });
        parts(json!([code("1 / 0", json!([error]))]))
    };
    let traceback = json!([
        "\x1b[0;31m-----------\x1b[0m",
        "\x1b[0;31mZeroDivisionError\x1b[0m: division by zero"
    ]);
    assert_eq!(
        error(traceback),
        [
            python("1 / 0"),
            output("-----------\nZeroDivisionError: division by zero"),
        ]
    );
    assert_eq!(
        error(json!([])),
        [
            python("1 / 0"),
            output("ZeroDivisionError: division by zero")
        ]
    );
}

#[test]
fn html_is_shown_when_markdown_lets_raw_html_in() {
    // As pandas shows a table, with a blank line that would end raw HTML in
    // Markdown, so it's a part of its own.
    let table = "<div>\n<style scoped>\n    .dataframe { color: red; }\n\n</style>\n\
                 <table class=\"dataframe\"><tr><td>1</td></tr></table>\n</div>\n";
    let cells = json!([code(
        "df",
        json!([display(
            json!({ "text/html": table, "text/plain": "   a\n0  1" })
        )])
    )]);
    assert_eq!(
        parts(cells.clone()),
        [python("df"), Part::Html(table.to_string())]
    );

    // Else it's text, as it is when the settings don't say.
    let text = [python("df"), output("   a\n0  1")];
    assert_eq!(parts_with(cells.clone(), NO_RAW_HTML), text);
    assert_eq!(parts_with(cells, "{}"), text);
}

#[test]
fn markdown_is_shown_as_markdown() {
    let cells = json!([code(
        "Markdown(\"**Bold**\")",
        json!([display(
            json!({ "text/markdown": "**Bold**", "text/plain": "<Markdown>" })
        )])
    )]);
    assert_eq!(
        parts(cells),
        [python("Markdown(\"**Bold**\")"), md("**Bold**")]
    );
}

#[test]
fn an_image_is_in_the_page() {
    let image = |data: Value| parts(json!([code("plot()", json!([display(data)]))])).pop();
    let png = || Some(md("![](data:image/png;base64,iVBORw0KGgo=)"));
    // As Jupyter saves it, with a line's end, with text for a terminal.
    let data = json!({ "image/png": "iVBORw0K\nGgo=\n", "text/plain": "<Figure>" });
    assert_eq!(image(data), png());
    let data = json!({ "image/jpeg": "/9j/4AAQ", "image/png": "iVBORw0KGgo=" });
    assert_eq!(image(data), png());
    let data = json!({ "image/gif": "R0lGODlh" });
    assert_eq!(image(data), Some(md("![](data:image/gif;base64,R0lGODlh)")));
}

#[test]
fn svg_is_shown_when_markdown_lets_raw_html_in() {
    let image = |markdown: &str, data: Value| {
        parts_with(json!([code("plot()", json!([display(data)]))]), markdown).pop()
    };
    // SVG is saved as text, and preferred.
    let data = json!({ "image/svg+xml": ["<svg>", "</svg>"], "image/png": "iVBORw0KGgo=" });
    let svg = format!(
        "![](data:image/svg+xml;base64,{})",
        BASE64.encode("<svg></svg>")
    );
    assert_eq!(image(RAW_HTML, data.clone()), Some(md(&svg)));

    let png = md("![](data:image/png;base64,iVBORw0KGgo=)");
    assert_eq!(image(NO_RAW_HTML, data), Some(png));
    let data = json!({ "image/svg+xml": "<svg></svg>", "text/plain": "<Figure>" });
    assert_eq!(image(NO_RAW_HTML, data), Some(output("<Figure>")));
}

#[test]
fn what_a_page_cant_show_falls_back_to_text() {
    let shown = |data: Value| parts(json!([code("show()", json!([display(data)]))]));
    let data = json!({
        "application/javascript": "alert(1)",
        "application/vnd.jupyter.widget-view+json": { "model_id": "1a" },
        "text/latex": "$x^2$",
        "text/plain": "x**2"
    });
    assert_eq!(shown(data), [python("show()"), output("x**2")]);
    // And without text, there's nothing to show.
    let data = json!({ "application/vnd.jupyter.widget-view+json": { "model_id": "1a" } });
    assert_eq!(shown(data), [python("show()")]);
}

#[test]
fn an_attached_image_is_in_the_page() {
    let cell = json!({
        "cell_type": "markdown",
        "metadata": {},
        "source": "![Chart](attachment:chart.png) ![Map](attachment:the%20map.png) \
                   ![Lost](attachment:lost.png) ![Logo](logo.png)",
        "attachments": {
            "chart.png": { "image/png": "iVBORw0KGgo=\n" },
            "the map.png": { "image/jpeg": "/9j/4AAQ" }
        }
    });
    assert_eq!(
        parts(json!([cell])),
        [md("![Chart](data:image/png;base64,iVBORw0KGgo=) \
             ![Map](data:image/jpeg;base64,/9j/4AAQ) \
             ![Lost](attachment:lost.png) ![Logo](logo.png)")]
    );
}

#[test]
fn an_attached_images_url_is_replaced_where_markdown_writes_it() {
    // As Jupyter names an image pasted twice, in angle brackets, beside a
    // longer name, in a reference's definition, and in a footnote.
    let cell = json!({
        "cell_type": "markdown",
        "metadata": {},
        "source": "![A](attachment:image%20(1).png) ![B](<attachment:b.png>) \
                   ![C](attachment:b.png.png) ![D][d][^1]\n\n\
                   [d]: attachment:d.png\n\n\
                   [^1]: ![E](attachment:e.png)",
        "attachments": {
            "image (1).png": { "image/png": "AAAA" },
            "b.png": { "image/png": "BBBB" },
            "b.png.png": { "image/png": "CCCC" },
            "d.png": { "image/png": "DDDD" },
            "e.png": { "image/png": "EEEE" }
        }
    });
    assert_eq!(
        parts(json!([cell])),
        [md(
            "![A](data:image/png;base64,AAAA) ![B](<data:image/png;base64,BBBB>) \
             ![C](data:image/png;base64,CCCC) ![D][d][^1]\n\n\
             [d]: data:image/png;base64,DDDD\n\n\
             [^1]: ![E](data:image/png;base64,EEEE)"
        )]
    );
}

#[test]
fn frontmatter_is_in_a_raw_or_markdown_cell_at_the_top() {
    let page = |cells: Value| read(&notebook(cells), RAW_HTML).unwrap();

    // A raw cell is only frontmatter, as Quarto writes it.
    assert_eq!(
        page(json!([
            raw("---\ntitle: Analysis\n---\nFor LaTeX"),
            markdown("Body")
        ])),
        [md("---\ntitle: Analysis\n---\n"), md("Body")]
    );

    // A Markdown cell goes on after it, as MyST writes it.
    assert_eq!(
        page(json!([markdown("---\ntitle: Analysis\n---\nBody")])),
        [md("---\ntitle: Analysis\n---\n"), md("Body")]
    );

    // Anywhere else, or unclosed, it's Markdown, and a raw cell is left
    // out, so the first part, where little finds frontmatter, is empty.
    assert_eq!(
        page(json!([
            code("1", json!([])),
            markdown("---\ntitle: Analysis\n---"),
            raw("---\ntitle: Raw\n---"),
        ])),
        [md(""), python("1"), md("---\ntitle: Analysis\n---")]
    );
    assert_eq!(
        page(json!([markdown("---\ntitle: Analysis")])),
        [md(""), md("---\ntitle: Analysis")]
    );
    // Even an output's Markdown.
    let shown = display(json!({ "text/markdown": "---\ntitle: Output\n---" }));
    assert_eq!(
        page(json!([code("", json!([shown]))])),
        [md(""), md("---\ntitle: Output\n---")]
    );
}

#[test]
fn a_notebook_is_read_as_leniently_as_jupyter_reads_it() {
    // As `nbstripout --keep-output` saves a result, older notebooks save
    // cells without `id`s, and other tools leave out what's empty.
    let notebook = json!({
        "cells": [
            { "cell_type": "markdown", "source": "# Analysis", "id": "a.b" },
            { "cell_type": "markdown", "source": "", "id": "a.b" },
            { "cell_type": "code", "source": "x" },
            {
                "cell_type": "code",
                "source": "x",
                "outputs": [
                    { "output_type": "execute_result", "data": { "text/plain": "1" }, "execution_count": null },
                    { "output_type": "display_data", "data": { "application/json": { "a": 1 } } },
                    { "output_type": "stream", "text": "Hi" }
                ],
                "execution_count": null
            }
        ],
        "nbformat": 4,
        "nbformat_minor": 6
    });
    assert_eq!(
        read(&notebook.to_string(), RAW_HTML).unwrap(),
        [
            md(""),
            md("# Analysis"),
            md(""),
            in_language("", "x"),
            in_language("", "x"),
            output("1"),
            output("Hi"),
        ]
    );
}

#[test]
fn a_file_that_isnt_a_notebook_the_plugin_reads_is_an_error() {
    let error = |text: &str| read(text, RAW_HTML).unwrap_err();
    assert_eq!(
        error(r#"{ "nbformat": 3, "nbformat_minor": 0, "metadata": {}, "worksheets": [] }"#),
        "it's in version 3 of Jupyter's notebook format, and the plugin reads version 4: \
         open it in Jupyter, and save it, to upgrade it"
    );
    let error_of_text = error("# Analysis");
    assert!(
        error_of_text.starts_with("reading the notebook: expected value at line 1"),
        "{error_of_text}"
    );
    let error = error(r#"{ "nbformat": 4, "cells": [{ "cell_type": "heading" }] }"#);
    assert!(error.contains("unknown variant `heading`"), "{error}");
}

#[test]
fn the_terminal_shows_text_as_it_was_drawn_last() {
    for (text, shown) in [
        ("plain\n", "plain\n"),
        ("a\r\nb\r\n", "a\nb\n"),
        ("1/3\r2/3\r3/3", "3/3"),
        ("1/3\r3/3\r\r\n", "3/3\n"),
        ("\x1b[1m\x1b[31mred\x1b[0m", "red"),
        ("\x1b]8;;https://example.com\x07link\x1b]8;;\x07", "link"),
        ("\x1b(B\x1b[mreset", "reset"),
    ] {
        assert_eq!(as_shown(text), shown, "{text:?}");
    }
}

#[test]
fn it_has_no_settings_of_its_own() {
    assert_eq!(check_settings(r#"{ "read": [] }"#), Ok(()));
    let error = check_settings(r#"{ "read": [], "raw_html": true }"#).unwrap_err();
    assert!(
        error.starts_with("it has no setting `raw_html`, only `read`"),
        "{error}"
    );
}
