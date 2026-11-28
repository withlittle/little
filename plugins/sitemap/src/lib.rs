//! A little plugin that writes a site's sitemap, `sitemap.xml`, for search
//! engines: the address of each page, and when it last changed. It reads
//! where the site is published in its `site` hook, notes each page in its
//! `page` hook, and writes the file in its `destination` hook, at the root
//! of the site, or of the version of it being built.

// Built for anything but WebAssembly, the plugin is only its tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::collections::BTreeMap;
use std::io;

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesText, Event};
use serde::Deserialize;
use serde_json::Value;
use time::format_description::parse_borrowed;
use time::format_description::well_known::Rfc3339;
use time::{Date, OffsetDateTime};
use url::Url;

/// The namespace of a sitemap, as <https://www.sitemaps.org/protocol.html>
/// sets it.
const NAMESPACE: &str = "http://www.sitemaps.org/schemas/sitemap/0.9";

/// The most pages the protocol lets one sitemap list.
const MOST_PAGES: usize = 50_000;

/// What the plugin reads of `site`, as its `site` hook is given it.
#[derive(Deserialize)]
struct Site {
    url: Option<Value>,
}

/// What the plugin reads of a `page`, as its `page` hook is given it.
#[derive(Deserialize)]
struct Page {
    #[serde(rename = "type")]
    page_type: Option<Value>,
    url: Option<Value>,
    modified: Option<Value>,
}

/// A site's sitemap, as a build gathers it, from hook to hook.
struct Sitemap {
    /// The scheme and host the site is published at, like
    /// `https://example.com/`, once `site` has read them.
    origin: Option<Url>,
    /// Each page's URL, as the theme gets it, like `/docs/guide/install`,
    /// with when it last changed, if it says. They're in order of URL, so a
    /// site builds the same sitemap each time.
    pages: BTreeMap<String, Option<String>>,
}

/// Check the plugin's settings, `json`, as little gives them to it: it has
/// none, but the `read` every plugin has.
fn check_settings(json: &str) -> Result<(), String> {
    let settings: serde_json::Map<String, Value> = serde_json::from_str(json)
        .map_err(|_| format!("its settings must be a mapping, not {json}"))?;
    match settings.keys().find(|name| *name != "read") {
        Some(name) => Err(format!(
            "it has no setting `{name}`, only `read`, which it doesn't need: little gives it each \
             page"
        )),
        None => Ok(()),
    }
}

impl Sitemap {
    /// A sitemap with nothing in it yet.
    const fn new() -> Sitemap {
        Sitemap {
            origin: None,
            pages: BTreeMap::new(),
        }
    }

    /// Reads where the site is published from `site`, as the `site` hook is
    /// given it: the scheme and host of its `url`, which each page's address
    /// starts with. A site that doesn't set one has no addresses to list.
    fn site(&mut self, site: &str) -> Result<(), String> {
        let site: Site = serde_json::from_str(site).map_err(|e| e.to_string())?;
        let not_an_address = |url: String| {
            format!(
                "`site.url` is {url}, which isn't where a site is published, like \
                 https://example.com/docs"
            )
        };
        let address = match &site.url {
            Some(Value::String(address)) => address,
            Some(other) => return Err(not_an_address(other.to_string())),
            None => {
                return Err(
                    "a sitemap lists each page's whole address, so set `site.url` to where the \
                     site is published, like https://example.com/docs"
                        .to_string(),
                );
            }
        };
        // Read as little reads it, so both agree on the host: in lowercase,
        // and without a port that's its scheme's own.
        let origin = (Url::parse(address).ok())
            .filter(|url| matches!(url.scheme(), "http" | "https"))
            .and_then(|url| Url::parse(&url.origin().ascii_serialization()).ok())
            .ok_or_else(|| not_an_address(format!("`{address}`")))?;
        self.origin = Some(origin);
        Ok(())
    }

    /// Notes `page`, as the `page` hook is given it, unless it's the 404
    /// page or a redirect, which aren't pages to find. A page whose
    /// `modified` isn't a date is still noted, without it, and the error
    /// says so, for little to warn of.
    fn page(&mut self, page: &str) -> Result<(), String> {
        let page: Page = serde_json::from_str(page).map_err(|e| e.to_string())?;
        let page_type = page.page_type.as_ref().and_then(Value::as_str);
        if matches!(page_type, Some("404" | "redirect")) {
            return Ok(());
        }
        // Frontmatter has the last word on `page.url`, so it can be anything.
        let url = match page.url {
            Some(Value::String(url)) if url.starts_with('/') && !url.starts_with("//") => url,
            Some(other) => {
                return Err(format!(
                    "its `url` is {other}, not a URL from the root of the site, like \
                     `/guide/install`, so it isn't in the sitemap"
                ));
            }
            None => return Err("it has no `url`, so it isn't in the sitemap".to_string()),
        };
        let (modified, not_a_date) = match &page.modified {
            None => (None, None),
            Some(value) => match last_modified(value) {
                Some(date) => (Some(date), None),
                None => (None, Some(value)),
            },
        };
        self.pages.insert(url, modified);
        match not_a_date {
            Some(value) => Err(format!(
                "its `modified` is {value}, which isn't a date, like `2026-11-19`, so the \
                 sitemap doesn't say when it last changed"
            )),
            None => Ok(()),
        }
    }

    /// The sitemap, as the file `destination` adds, or why it can't be
    /// written.
    fn write(&self) -> Result<Vec<u8>, String> {
        // little starts a plugin that crashes again, without what it kept,
        // and half a sitemap is worse than a build that stops.
        let Some(origin) = &self.origin else {
            return Err(
                "it crashed earlier in the build, and lost the pages it had noted".to_string(),
            );
        };
        if self.pages.len() > MOST_PAGES {
            return Err(format!(
                "the site has {} pages, and one sitemap lists {MOST_PAGES} at most",
                self.pages.len()
            ));
        }
        self.xml(origin).map_err(|e| e.to_string())
    }

    /// The sitemap, as XML: each page it noted, by its address, which is
    /// `origin`, the site's scheme and host, then the page's URL, with what
    /// a URL can't hold encoded, as a browser encodes a link.
    fn xml(&self, origin: &Url) -> io::Result<Vec<u8>> {
        let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
        writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
        let urlset = writer.create_element("urlset");
        let urlset = urlset.with_attribute(("xmlns", NAMESPACE));
        urlset.write_inner_content(|writer| {
            for (url, modified) in &self.pages {
                let mut address = origin.clone();
                address.set_path(url);
                writer.create_element("url").write_inner_content(|writer| {
                    let loc = writer.create_element("loc");
                    loc.write_text_content(BytesText::new(address.as_str()))?;
                    if let Some(modified) = modified {
                        let lastmod = writer.create_element("lastmod");
                        lastmod.write_text_content(BytesText::new(modified))?;
                    }
                    Ok(())
                })?;
            }
            Ok(())
        })?;
        let mut xml = writer.into_inner();
        xml.push(b'\n');
        Ok(xml)
    }
}

/// When a page last changed, from its `modified`, as a sitemap writes it: a
/// date and time, as RFC 3339 writes it, like `2026-11-19T02:37:42Z`, which
/// little gives a page from git, or a date alone, like `2026-11-19`, as an
/// author writes one in frontmatter. These are the dates little's own
/// `dateformat` filter reads. None, if it's neither.
fn last_modified(modified: &Value) -> Option<String> {
    let text = modified.as_str()?;
    if let Ok(time) = OffsetDateTime::parse(text, &Rfc3339) {
        return time.format(&Rfc3339).ok();
    }
    let date = parse_borrowed::<2>("[year]-[month]-[day]").ok()?;
    Date::parse(text, &date).ok()?.format(&date).ok()
}

#[cfg(target_arch = "wasm32")]
mod plugin {
    wit_bindgen::generate!({
        path: "../../wit/plugin.wit",
        inline: "
            package little:sitemap;

            world sitemap {
                export little:plugin/setup@0.2.1;
                export little:plugin/site@0.2.1;
                export little:plugin/page@0.2.1;
                export little:plugin/destination@0.2.1;
            }
        ",
        world: "little:sitemap/sitemap",
        generate_all,
    });

    use std::sync::Mutex;

    use exports::little::plugin::destination::{self, File};
    use exports::little::plugin::{page, setup, site};

    /// The sitemap of the site, or the version of it, that's being built: a
    /// build starts the plugin once for each, and it keeps its state from
    /// hook to hook.
    static SITEMAP: Mutex<super::Sitemap> = Mutex::new(super::Sitemap::new());

    struct Plugin;

    impl setup::Guest for Plugin {
        fn setup(settings: String) -> Result<(), String> {
            super::check_settings(&settings)
        }
    }

    impl site::Guest for Plugin {
        fn site(site: String) -> Result<String, String> {
            SITEMAP.lock().unwrap().site(&site)?;
            Ok(site)
        }
    }

    impl page::Guest for Plugin {
        fn page(page: String) -> Result<String, String> {
            SITEMAP.lock().unwrap().page(&page)?;
            Ok(page)
        }
    }

    impl destination::Guest for Plugin {
        fn destination(_: Vec<String>) -> Result<Vec<File>, String> {
            Ok(vec![File {
                path: "sitemap.xml".to_string(),
                contents: SITEMAP.lock().unwrap().write()?,
            }])
        }
    }

    export!(Plugin);
}

#[cfg(test)]
mod tests;
