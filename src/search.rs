//! Searching a site: each build indexes the site's pages with Pagefind, built
//! in, and writes the files that search it to `pagefind/` in the destination,
//! as Pagefind does, where the theme's search box finds them.
//!
//! What's indexed is each page's own content, rendered from its Markdown,
//! under its title, and not the theme's markup around it: nothing a theme
//! puts around a page, like its header, turns up in results, and a theme has
//! nothing to mark. Each page is indexed in the language its theme gives it
//! with `<html lang>`, so in English, "installing" finds "install". The
//! build gives it only the pages to find: not the 404 page, nor those the
//! site's `search.exclude`, or a page's own `search`, leaves out.

use anyhow::{Context, Result};
use pagefind::api::PagefindIndex;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// The folder in the destination that search's files go in: Pagefind's
/// default, the one its docs and search boxes expect.
pub const DIR: &str = "pagefind";

/// A page to index.
pub struct Page<'a> {
    /// Its URL from the root of the site, like `/guide/`, without the site's
    /// path: Pagefind puts that back, from where it finds the index.
    pub url: &'a str,
    pub title: &'a str,
    /// Its own content, rendered from its Markdown.
    pub content: &'a str,
    /// The language its theme gives it, like `en`, if it gives it one.
    pub lang: Option<String>,
}

/// One of the files search needs in the destination.
pub struct File {
    /// Where it goes, from the root of the destination, like
    /// `pagefind/pagefind.js`.
    pub path: PathBuf,
    pub contents: Vec<u8>,
}

/// Index `pages`, and return the files that hold the index and the scripts
/// that search it.
pub fn index(pages: &[Page]) -> Result<Vec<File>> {
    let runtime = tokio::runtime::Runtime::new().context("starting the search indexer")?;
    let files = runtime.block_on(async {
        // Built in like this, Pagefind prints nothing.
        let mut pagefind = PagefindIndex::new(None).context("starting the search indexer")?;
        for page in pages {
            pagefind
                .add_html_file(None, Some(page.url.to_string()), document(page))
                .await
                .with_context(|| format!("indexing {} for search", page.url))?;
        }
        pagefind
            .get_files()
            .await
            .context("building the search index")
    })?;
    let files = files.into_iter().map(|file| File {
        path: Path::new(DIR).join(file.filename),
        contents: file.contents,
    });
    Ok(files.collect())
}

/// The language a page's theme gives it with `<html lang>`, like `en`, from
/// the page as the theme renders it.
pub fn lang(html: &str) -> Option<String> {
    // A language's tag is letters, digits, and hyphens, like `pt-BR`, so it
    // needs no escaping in `document`.
    static LANG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)<html\s(?:[^>]*\s)?lang\s*=\s*["']?([a-z0-9-]+)"#).unwrap()
    });
    LANG.captures(html).map(|captures| captures[1].to_string())
}

/// The page as Pagefind reads it: its content alone, in its language, with
/// its title as the title of its results, rather than the first `<h1>` that
/// Pagefind would take.
fn document(page: &Page) -> String {
    let lang = (page.lang.as_ref())
        .map(|lang| format!(" lang=\"{lang}\""))
        .unwrap_or_default();
    // Escaped for an attribute's value, which Pagefind unescapes.
    let mut title = String::new();
    comrak::html::escape(&mut title, page.title).expect("writing to a String");
    format!(
        "<html{lang}><head><meta data-pagefind-meta=\"title[content]\" content=\"{title}\"></head>\
         <body>{}</body></html>",
        page.content
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_language_a_theme_gives_a_page() {
        for (html, expected) in [
            ("<!doctype html>\n<html lang=\"en\">", Some("en")),
            ("<html lang='pt-BR'>", Some("pt-BR")),
            ("<html lang=fr>", Some("fr")),
            ("<HTML LANG = \"de\">", Some("de")),
            ("<html class=\"js\"\n  lang=\"ja\" dir=\"ltr\">", Some("ja")),
            // Not without a `lang`, or one that's blank...
            ("<html><body lang=\"en\">", None),
            ("<html lang=\"\">", None),
            // ...nor from another attribute, or another element.
            ("<html data-lang=\"en\">", None),
            ("<htmlx lang=\"en\">", None),
            ("<p>no html at all</p>", None),
        ] {
            assert_eq!(lang(html).as_deref(), expected, "{html}");
        }
    }

    #[test]
    fn a_document_holds_a_pages_content_title_and_language() {
        let page = Page {
            url: "/guide/",
            title: "Q&A: \"<tags>\"",
            content: "<h1>Q&amp;A</h1>",
            lang: Some("en".into()),
        };
        assert_eq!(
            document(&page),
            "<html lang=\"en\"><head><meta data-pagefind-meta=\"title[content]\" \
             content=\"Q&amp;A: &quot;&lt;tags&gt;&quot;\"></head>\
             <body><h1>Q&amp;A</h1></body></html>"
        );

        let page = Page { lang: None, ..page };
        assert!(document(&page).starts_with("<html><head>"));
    }
}
