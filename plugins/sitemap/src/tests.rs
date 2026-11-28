use serde_json::{Value, json};

use super::{MOST_PAGES, Sitemap, check_settings};

/// A sitemap of the site published at `url`, once `site` has read it.
fn sitemap(url: &str) -> Sitemap {
    let mut sitemap = Sitemap::new();
    let site = json!({ "name": "A site", "url": url, "navigation": { "items": [] } });
    sitemap.site(&site.to_string()).unwrap();
    sitemap
}

/// A `page`, as little gives one to the `page` hook, of `page_type`, at
/// `url`, with each key of `more`, like its `modified`.
fn page(page_type: &str, url: &str, more: Value) -> String {
    let mut page = json!({
        "type": page_type,
        "url": url,
        "title": "A page",
        "content": "<p>Its content, with a <a href=\"/docs/\">link</a>.</p>",
        "search": { "enabled": true }
    });
    for (key, value) in more.as_object().unwrap() {
        page[key] = value.clone();
    }
    page.to_string()
}

/// A page at `url`, which last changed when `modified` says, if it does.
fn page_at(url: &str, modified: Option<&str>) -> String {
    match modified {
        Some(modified) => page("page", url, json!({ "modified": modified })),
        None => page("page", url, json!({})),
    }
}

/// A sitemap, as it's written, of `pages`: each one's address, and when it
/// last changed, if it says.
fn xml(pages: &[(&str, Option<&str>)]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for (address, modified) in pages {
        xml.push_str(&format!("  <url>\n    <loc>{address}</loc>\n"));
        if let Some(modified) = modified {
            xml.push_str(&format!("    <lastmod>{modified}</lastmod>\n"));
        }
        xml.push_str("  </url>\n");
    }
    xml.push_str("</urlset>\n");
    xml
}

/// What `sitemap` writes, as text.
fn written(sitemap: &Sitemap) -> String {
    String::from_utf8(sitemap.write().unwrap()).unwrap()
}

#[test]
fn it_has_no_settings_of_its_own() {
    assert_eq!(check_settings(r#"{ "read": [] }"#), Ok(()));
    let error = check_settings(r#"{ "read": [], "exclude": ["/changelog"] }"#).unwrap_err();
    assert!(
        error.starts_with("it has no setting `exclude`, only `read`"),
        "{error}"
    );
}

#[test]
fn it_lists_each_page_by_its_address_in_order_of_url() {
    let mut sitemap = sitemap("https://example.com/docs");
    // In the order little builds them, which isn't the order of their URLs.
    for (url, modified) in [
        ("/docs/guide/install", Some("2026-11-02T21:05:10Z")),
        ("/docs/", Some("2026-11-19T02:37:42Z")),
        ("/docs/reference/python/", None),
        ("/docs/guide/", Some("2026-10-30T08:00:00Z")),
    ] {
        assert_eq!(sitemap.page(&page_at(url, modified)), Ok(()));
    }
    assert_eq!(
        written(&sitemap),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/docs/</loc>
    <lastmod>2026-11-19T02:37:42Z</lastmod>
  </url>
  <url>
    <loc>https://example.com/docs/guide/</loc>
    <lastmod>2026-10-30T08:00:00Z</lastmod>
  </url>
  <url>
    <loc>https://example.com/docs/guide/install</loc>
    <lastmod>2026-11-02T21:05:10Z</lastmod>
  </url>
  <url>
    <loc>https://example.com/docs/reference/python/</loc>
  </url>
</urlset>
"#
    );
}

#[test]
fn it_lists_a_home_and_a_folders_page_but_not_the_404_page_or_a_redirect() {
    let mut sitemap = sitemap("https://example.com");
    for (page_type, url) in [
        ("home", "/"),
        ("section", "/guide/"),
        ("page", "/guide/install"),
        ("404", "/404.html"),
        ("redirect", "/guide/setup"),
    ] {
        assert_eq!(sitemap.page(&page(page_type, url, json!({}))), Ok(()));
    }
    assert_eq!(
        written(&sitemap),
        xml(&[
            ("https://example.com/", None),
            ("https://example.com/guide/", None),
            ("https://example.com/guide/install", None),
        ])
    );
}

#[test]
fn a_site_needs_a_url() {
    let site = |site: Value| Sitemap::new().site(&site.to_string());
    assert_eq!(
        site(json!({ "name": "A site" })),
        Err(
            "a sitemap lists each page's whole address, so set `site.url` to where the site is \
             published, like https://example.com/docs"
                .to_string()
        )
    );
    // little checks `site.url` itself, but a plugin before this one can
    // change the `site` it's given.
    for (url, shown) in [
        (json!("example.com/docs"), "`example.com/docs`"),
        (json!("ftp://example.com/docs"), "`ftp://example.com/docs`"),
        (json!(8080), "8080"),
    ] {
        assert_eq!(
            site(json!({ "url": url })),
            Err(format!(
                "`site.url` is {shown}, which isn't where a site is published, like \
                 https://example.com/docs"
            ))
        );
    }
}

#[test]
fn an_address_starts_with_the_scheme_and_host_of_the_sites_url() {
    // As little reads them: the host in lowercase, without its scheme's own
    // port, and without the path, which each page's URL has already.
    for (url, address) in [
        ("https://example.com", "https://example.com/guide"),
        ("https://example.com/", "https://example.com/guide"),
        ("https://Example.COM:443/docs/", "https://example.com/guide"),
        ("http://localhost:8080/docs", "http://localhost:8080/guide"),
    ] {
        let mut sitemap = sitemap(url);
        assert_eq!(sitemap.page(&page_at("/guide", None)), Ok(()));
        assert_eq!(written(&sitemap), xml(&[(address, None)]), "{url}");
    }
}

#[test]
fn a_page_last_changed_on_a_date_or_at_a_date_and_time() {
    let mut sitemap = sitemap("https://example.com");
    for (url, modified) in [
        // As little gives it, from git.
        ("/from-git", "2026-11-19T02:37:42Z"),
        // As an author writes it in frontmatter.
        ("/a-date", "2026-11-19"),
        ("/a-zone", "2026-11-19T12:37:42+10:00"),
        ("/lowercase", "2026-11-19t02:37:42z"),
    ] {
        assert_eq!(sitemap.page(&page_at(url, Some(modified))), Ok(()), "{url}");
    }
    assert_eq!(
        written(&sitemap),
        xml(&[
            ("https://example.com/a-date", Some("2026-11-19")),
            (
                "https://example.com/a-zone",
                Some("2026-11-19T12:37:42+10:00")
            ),
            ("https://example.com/from-git", Some("2026-11-19T02:37:42Z")),
            (
                "https://example.com/lowercase",
                Some("2026-11-19T02:37:42Z")
            ),
        ])
    );
}

#[test]
fn a_modified_that_isnt_a_date_is_warned_of_and_left_out() {
    let mut sitemap = sitemap("https://example.com");
    for (url, modified, shown) in [
        ("/words", json!("last week"), r#""last week""#),
        ("/no-such-day", json!("2026-02-30"), r#""2026-02-30""#),
        ("/a-year", json!(2026), "2026"),
    ] {
        assert_eq!(
            sitemap.page(&page("page", url, json!({ "modified": modified }))),
            Err(format!(
                "its `modified` is {shown}, which isn't a date, like `2026-11-19`, so the \
                 sitemap doesn't say when it last changed"
            ))
        );
    }
    // Each is still a page to find.
    assert_eq!(
        written(&sitemap),
        xml(&[
            ("https://example.com/a-year", None),
            ("https://example.com/no-such-day", None),
            ("https://example.com/words", None),
        ])
    );
}

#[test]
fn an_address_is_encoded_as_a_link_is_and_escaped_for_xml() {
    let mut sitemap = sitemap("https://example.com/docs");
    for url in [
        "/docs/my page",
        "/docs/ünïcode",
        "/docs/a&b",
        "/docs/what's-new",
        "/docs/why?",
        "/docs/c#",
    ] {
        assert_eq!(sitemap.page(&page_at(url, None)), Ok(()), "{url}");
    }
    assert_eq!(
        written(&sitemap),
        xml(&[
            ("https://example.com/docs/a&amp;b", None),
            ("https://example.com/docs/c%23", None),
            ("https://example.com/docs/my%20page", None),
            ("https://example.com/docs/what&apos;s-new", None),
            ("https://example.com/docs/why%3F", None),
            ("https://example.com/docs/%C3%BCn%C3%AFcode", None),
        ])
    );
}

#[test]
fn a_page_whose_url_isnt_from_the_root_of_the_site_is_warned_of_and_left_out() {
    let mut sitemap = sitemap("https://example.com");
    // Frontmatter can set a page's `url` to anything.
    for (url, shown) in [
        (
            json!("https://example.org/guide"),
            r#""https://example.org/guide""#,
        ),
        (json!("//example.org/guide"), r#""//example.org/guide""#),
        (json!("guide"), r#""guide""#),
        (json!(7), "7"),
    ] {
        assert_eq!(
            sitemap.page(&page("page", "/guide", json!({ "url": url }))),
            Err(format!(
                "its `url` is {shown}, not a URL from the root of the site, like \
                 `/guide/install`, so it isn't in the sitemap"
            ))
        );
    }
    assert_eq!(
        sitemap.page(r#"{ "type": "page", "title": "A page" }"#),
        Err("it has no `url`, so it isn't in the sitemap".to_string())
    );
    assert_eq!(written(&sitemap), xml(&[]));
}

#[test]
fn a_sitemap_lists_fifty_thousand_pages_at_most() {
    let mut sitemap = sitemap("https://example.com");
    for number in 0..MOST_PAGES {
        sitemap.pages.insert(format!("/page-{number}"), None);
    }
    assert_eq!(written(&sitemap).matches("<loc>").count(), MOST_PAGES);
    sitemap.pages.insert("/one-more".to_string(), None);
    assert_eq!(
        sitemap.write(),
        Err("the site has 50001 pages, and one sitemap lists 50000 at most".to_string())
    );
}

#[test]
fn half_a_sitemap_isnt_written_after_a_crash() {
    // little starts a plugin that crashed again, so it's without the site's
    // `url`, and the pages it had noted.
    let mut sitemap = Sitemap::new();
    assert_eq!(sitemap.page(&page_at("/guide", None)), Ok(()));
    assert_eq!(
        sitemap.write(),
        Err("it crashed earlier in the build, and lost the pages it had noted".to_string())
    );
}
