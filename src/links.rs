//! Checking a built site's links with lychee: that the page, file, or
//! `#fragment` each one leads to is there, in the site or on the web.
//!
//! lychee reads the built HTML, so the theme's links are checked as well as
//! the Markdown's. A site published under a path, like `/my-project`, is
//! checked as it's published: a link from the root of the domain is resolved
//! in a folder that's never there, and only one under the site's path is led
//! from it into the destination, so a link that leaves the path out is
//! broken here, as it would be on the web.
//!
//! A broken link in a page's Markdown is shown as it's written there, so an
//! author can find it, and any other, like one in the theme, as a reader
//! follows it.
//!
//! A link to the site's repository, or to anything in it, isn't checked: a
//! theme can link to each page's file there, from every page, and a new
//! page's file isn't there until it's pushed, nor any in a private
//! repository to a checker that hasn't signed in.

use anyhow::{Context, Result, anyhow};
use futures::StreamExt;
use lychee_lib::remap::Remaps;
use lychee_lib::{
    BaseInfo, Client, ClientBuilder, Collector, FileType, FragmentCheckerOptions, Input,
    InputSource, Request, RequestError, ResolvedInputSource, Uri,
};
use percent_encoding::percent_decode_str;
use regex::{Regex, RegexSet};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use url::Url;

use crate::config::Repo;
use crate::render::WrittenLink;
use crate::site::Built;

/// How many files a warning names before it counts the rest.
const FILES_NAMED: usize = 3;

/// How many links are checked at once; lychee also limits how often it asks
/// each host.
const AT_ONCE: usize = 32;

/// Which links to check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Every link, including those that leave the site.
    All,
    /// Only the links within the site, which need no network.
    Internal,
}

/// A broken link, and the files it's in.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Broken {
    /// What each file it's in was built from, like `./docs/guide.md`, sorted.
    pub files: Vec<String>,
    /// The link as the Markdown writes it, like `install.md#usage`, or, in
    /// the theme or a copied file, as a reader follows it, like
    /// `/my-project/guide/#usage`.
    pub url: String,
    /// Why it's broken, in lychee's words.
    pub reason: String,
}

impl fmt::Display for Broken {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // A link in the theme is on every page, so only a few are named.
        let named = self.files.len().min(FILES_NAMED);
        let files = self.files[..named].join(", ");
        write!(f, "broken link {} in {files}", self.url)?;
        if self.files.len() > named {
            write!(f, " and {} more", self.files.len() - named)?;
        }
        write!(f, ": {}", self.reason)
    }
}

/// The broken links in `built`, sorted by the files they're in.
pub fn check(built: &Built, scope: Scope) -> Result<Vec<Broken>> {
    tokio::runtime::Runtime::new()
        .context("starting the link checker")?
        .block_on(check_site(&Site::new(built)?, scope))
}

async fn check_site(site: &Site<'_>, scope: Scope) -> Result<Vec<Broken>> {
    // Each link is checked once, however many files it's in, each with how
    // its Markdown writes it, if it does.
    let mut links: BTreeMap<String, (Request, BTreeSet<Found>)> = BTreeMap::new();
    // The files each broken link is in, by the link as it's shown and why
    // it's broken: one written alike in many files is one warning, as are
    // two that look alike once shown, like `/guide/#usage` and
    // `/guide/index.html#usage`, as a reader looks for one.
    let mut broken: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    let input = Input {
        source: InputSource::FsPath(site.destination.clone()),
        file_type_hint: None,
    };
    let requests = Collector::new(Some(site.root.clone()), BaseInfo::none())?
        // A destination git ignores, as `dist/` usually is, is still checked.
        .skip_ignored(false)
        .collect_links_from_file_types(HashSet::from([input]), FileType::html_extensions());
    let mut requests = std::pin::pin!(requests);
    while let Some(request) = requests.next().await {
        match request {
            Ok(request) => {
                let key = request.uri.as_str().to_string();
                let found = site.found(&request.source, &key);
                let (_, files) = links
                    .entry(key)
                    .or_insert_with(|| (request, BTreeSet::new()));
                files.extend(found);
            }
            // A link that isn't a URL at all, like `https://exa mple.com`.
            Err(RequestError::CreateRequestItem(link, source, error)) => {
                let reason = error.to_string();
                for (file, written) in site.found(&source, &link.text) {
                    let url = written.unwrap_or_else(|| link.text.clone());
                    broken
                        .entry((url, reason.clone()))
                        .or_default()
                        .insert(file);
                }
            }
            Err(error) => return Err(error).context("reading the built site's links"),
        }
    }

    let client = site.client(scope)?;
    let checked: Vec<_> = futures::stream::iter(links.into_values())
        .map(|(request, files)| {
            let client = &client;
            async move {
                let link = request.uri.clone();
                let (checked, reason) = match client.check(request).await {
                    Ok(response) => {
                        let status = response.status();
                        let reason = status.is_error().then(|| status.to_string());
                        (response.body().uri.clone(), reason)
                    }
                    Err(error) => (link.clone(), Some(error.to_string())),
                };
                Some((site.shown(&link, &checked), reason?, files))
            }
        })
        .buffer_unordered(AT_ONCE)
        .collect()
        .await;
    for (shown, reason, files) in checked.into_iter().flatten() {
        for (file, written) in files {
            let url = written.unwrap_or_else(|| shown.clone());
            broken
                .entry((url, reason.clone()))
                .or_default()
                .insert(file);
        }
    }

    let mut broken: Vec<_> = broken
        .into_iter()
        .map(|((url, reason), files)| Broken {
            files: files.into_iter().collect(),
            url,
            reason,
        })
        .collect();
    broken.sort();
    Ok(broken)
}

/// A file a link is in, by what it was built from, and the link as the
/// file's Markdown writes it: `None` for one that isn't in its Markdown, like
/// one in the theme.
type Found = (String, Option<String>);

/// A built site, as lychee finds its files and links.
struct Site<'a> {
    built: &'a Built,
    /// The destination, as an absolute path, as lychee names its files.
    destination: PathBuf,
    /// The folder a link from the root of the domain is resolved in. It's
    /// never there, so only a link remapped out of it into the destination
    /// can be found.
    root: PathBuf,
    /// How each page's Markdown writes its links, by the page's path in the
    /// destination, then by the URL lychee makes of each, or, for one that
    /// isn't a URL, by its text.
    written: HashMap<PathBuf, HashMap<String, BTreeSet<String>>>,
}

impl<'a> Site<'a> {
    fn new(built: &'a Built) -> Result<Self> {
        let destination = std::path::absolute(&built.destination)
            .with_context(|| format!("finding {}", built.destination.display()))?;
        let root = destination.join(".little-root");
        let root_url = dir_url(&root)?;
        let written = built
            .links
            .iter()
            .map(|(rel, links)| {
                let source = ResolvedInputSource::FsPath(destination.join(rel));
                (rel.clone(), written_in(&source, links, &root_url))
            })
            .collect();
        Ok(Self {
            built,
            destination,
            root,
            written,
        })
    }

    fn client(&self, scope: Scope) -> Result<Client> {
        let schemes = match scope {
            Scope::All => HashSet::new(),
            // A link to the site's own `url` is still checked: it's remapped
            // to a `file` link first.
            Scope::Internal => HashSet::from(["file".to_string()]),
        };
        let client = ClientBuilder::builder()
            .index_files(Some(vec!["index.html".to_string()]))
            .fragment_checker_options(FragmentCheckerOptions {
                check_anchor_fragments: true,
                check_text_fragments: false,
            })
            .remaps(Some(self.remaps()?))
            .schemes(schemes)
            // Links that only work on the author's machine, like localhost.
            .exclude_all_private(true)
            .excludes(self.built.repo.as_ref().map(into_repo).transpose()?)
            .build()
            .client()?;
        Ok(client)
    }

    /// Lead into the destination a link from the root of the domain that's
    /// under the site's path, and a link to the site's own `url`.
    fn remaps(&self) -> Result<Remaps> {
        let site = &self.built.url;
        // Made URLs as lychee makes them, so they're encoded alike.
        let mut under = vec![dir_url(&self.root)?.join(site.path.trim_start_matches('/'))?];
        if !site.origin.is_empty() {
            under.push(Url::parse(&format!("{}{}", site.origin, site.path))?);
        }
        // `$` would start a capture group in a replacement.
        let into = format!(
            "{}${{1}}",
            trimmed(&dir_url(&self.destination)?).replace('$', "$$")
        );
        let remaps = under
            .iter()
            .map(|url| {
                let pattern = format!("^{}([/?#]|$)", regex::escape(trimmed(url)));
                Ok((Regex::new(&pattern)?, into.clone()))
            })
            .collect::<Result<_>>()?;
        Ok(Remaps::new(remaps))
    }

    /// `link`, which lychee checked as `checked`, as a reader follows it: a
    /// link within the site from the root of the domain, like
    /// `/my-project/guide/#usage`, and any other link as it is.
    fn shown(&self, link: &Uri, checked: &Uri) -> String {
        let shown = || -> Option<String> {
            if !link.is_file() {
                return None;
            }
            let url = Url::parse(checked.as_str()).ok()?;
            let path = url.to_file_path().ok()?;
            let (mut shown, rel, page) = match path.strip_prefix(&self.root) {
                Ok(rel) => (String::new(), rel, None),
                Err(_) => {
                    let rel = path.strip_prefix(&self.destination).ok()?;
                    let urls = &self.built.urls;
                    let page = urls.get(rel).or_else(|| urls.get(&rel.join("index.html")));
                    (self.built.url.path.clone(), rel, page)
                }
            };
            // lychee drops the `/` from the end of a link to a file, so a
            // page is shown at its URL, and a folder gets its `/` back.
            if let Some(page) = page {
                shown.push_str(page);
            } else {
                for part in rel {
                    shown.push('/');
                    shown.push_str(&part.to_string_lossy());
                }
                if shown.is_empty() || path.is_dir() {
                    shown.push('/');
                }
            }
            if let Some(query) = url.query() {
                shown.push('?');
                shown.push_str(query);
            }
            if let Some(fragment) = url.fragment() {
                shown.push('#');
                shown.push_str(&percent_decode_str(fragment).decode_utf8_lossy());
            }
            Some(shown)
        };
        shown().unwrap_or_else(|| link.to_string())
    }

    /// What `source`, a file lychee read, was built from.
    fn origin(&self, source: &ResolvedInputSource) -> String {
        self.in_destination(source)
            .and_then(|rel| self.built.origins.get(rel))
            .cloned()
            .unwrap_or_else(|| source.to_string())
    }

    /// The file `link` is in, which lychee read as `source`, once for each
    /// way its Markdown writes the link, or once if it doesn't. `link` is
    /// the URL lychee made of it, or, if it isn't a URL, its text.
    fn found(&self, source: &ResolvedInputSource, link: &str) -> Vec<Found> {
        let file = self.origin(source);
        let written = self
            .in_destination(source)
            .and_then(|rel| self.written.get(rel)?.get(link));
        match written {
            Some(written) => written
                .iter()
                .map(|written| (file.clone(), Some(written.clone())))
                .collect(),
            None => vec![(file, None)],
        }
    }

    /// Where `source`, a file lychee read, is in the destination.
    fn in_destination<'s>(&self, source: &'s ResolvedInputSource) -> Option<&'s Path> {
        match source {
            ResolvedInputSource::FsPath(path) => path.strip_prefix(&self.destination).ok(),
            _ => None,
        }
    }
}

/// How the Markdown of the file lychee reads as `source` writes each of its
/// `links`, by the URL lychee makes of it, or, for one that isn't a URL, by
/// its text. The URL is made as lychee makes it, from the text it reads: from
/// the file's folder, or, for a link from the root of the domain, from
/// `root`.
fn written_in(
    source: &ResolvedInputSource,
    links: &[WrittenLink],
    root: &Url,
) -> HashMap<String, BTreeSet<String>> {
    let base = source.to_base_info().ok();
    let mut written: HashMap<String, BTreeSet<String>> = HashMap::new();
    for link in links {
        let url = base.as_ref().and_then(|base| {
            base.parse_url_text_with_root_dir(&link.href, Some(root))
                .ok()
        });
        let key = match url {
            Some(mut url) => {
                // As lychee drops the `/` from the end of a link to a file.
                if url.scheme() == "file" {
                    let _ = url.path_segments_mut().map(|mut path| {
                        path.pop_if_empty();
                    });
                }
                url.into()
            }
            None => link.href.clone(),
        };
        written.entry(key).or_default().insert(link.written.clone());
    }
    written
}

/// The `file` URL of the folder `dir`, which must be an absolute path.
fn dir_url(dir: &Path) -> Result<Url> {
    Url::from_directory_path(dir).map_err(|()| anyhow!("{} has no file URL", dir.display()))
}

/// `url` with no `/` at the end, so a path can follow it.
fn trimmed(url: &Url) -> &str {
    url.as_str().trim_end_matches('/')
}

/// What matches a link to `repo`, or to anything in it, as lychee writes the
/// link: with its host in lowercase, say.
fn into_repo(repo: &Repo) -> Result<RegexSet> {
    let url = Url::parse(&repo.url).with_context(|| format!("reading `repo`, {}", repo.url))?;
    let pattern = format!("^{}([/?#]|$)", regex::escape(trimmed(&url)));
    Ok(RegexSet::new([pattern])?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site;
    use std::fs;

    /// A theme that shows only each page's content, so a test sees only the
    /// links it writes.
    const BARE_THEME: [(&str, &str); 2] = [
        ("little.yml", "theme: ./theme\n"),
        ("theme/_templates/default.html", "{{ page.content | safe }}"),
    ];

    const NOT_FOUND: &str = "File not found. Check if file exists and path is correct";

    /// A throwaway site holding `files`, each a (path, contents) pair; a
    /// later file overwrites an earlier one at the same path.
    fn scratch_site(label: &str, files: &[(&str, &str)]) -> PathBuf {
        let site =
            std::env::temp_dir().join(format!("little-links-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&site);
        for (file, contents) in files {
            let path = site.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
        }
        site
    }

    /// Build the site holding `files`, and warn of each broken link in
    /// `scope`, naming each file by its path in the site.
    fn broken_links(label: &str, scope: Scope, files: &[(&str, &str)]) -> Vec<String> {
        let site = scratch_site(label, files);
        let built = site::build(&site).unwrap();
        let broken = check(&built, scope).unwrap();
        fs::remove_dir_all(&site).unwrap();
        let prefix = format!("{}/", site.display());
        broken
            .into_iter()
            .map(|mut link| {
                for file in &mut link.files {
                    if let Some(rel) = file.strip_prefix(&prefix) {
                        *file = rel.to_string();
                    }
                }
                link.to_string()
            })
            .collect()
    }

    #[test]
    fn finds_pages_files_and_fragments_that_arent_there() {
        let broken = broken_links(
            "missing",
            Scope::Internal,
            &[
                BARE_THEME[0],
                BARE_THEME[1],
                (
                    "docs/README.md",
                    "# Home\n\n## Usage\n\n\
                     [Guide](guide/) [Install](/guide/install#from-source) [Usage](#usage) \
                     ![Diagram](guide/diagram.png)\n\n\
                     [Uninstall](/guide/uninstall) ![Logo](images/logo.png) \
                     [Usage](#usge) [Install](guide/install#from-sorce)\n",
                ),
                ("docs/guide/README.md", "# Guide\n"),
                ("docs/guide/install.md", "# Install\n\n## From source\n"),
                ("docs/guide/diagram.png", "png"),
                // Copied as it is, so its links are led from its folder.
                (
                    "docs/guide/old.html",
                    "<a href=\"install\">Install</a> <a href=\"setup.html\">Setup</a>",
                ),
            ],
        );
        assert_eq!(
            broken,
            [
                "broken link #usge in docs/README.md: Cannot find fragment".to_string(),
                format!("broken link /guide/uninstall in docs/README.md: {NOT_FOUND}"),
                "broken link guide/install#from-sorce in docs/README.md: Cannot find fragment"
                    .to_string(),
                format!("broken link images/logo.png in docs/README.md: {NOT_FOUND}"),
                format!("broken link /guide/setup.html in docs/guide/old.html: {NOT_FOUND}"),
            ]
        );
    }

    #[test]
    fn follows_a_link_to_a_pages_file_to_the_page() {
        let broken = broken_links(
            "md",
            Scope::Internal,
            &[
                (
                    "little.yml",
                    "theme: ./theme\nurl: https://withlittle.github.io/little\n",
                ),
                BARE_THEME[1],
                (
                    "docs/README.md",
                    "# Home\n\n[Guide](guide/README.md) [Install](guide/install.md#from-source) \
                     [Home](README.md) [Home](/readme.md)\n\n\
                     [Uninstall](guide/uninstall.md) [Install](guide/install.md#from-sorce)\n",
                ),
                (
                    "docs/guide/README.md",
                    "# Guide\n\n## Usage\n\n[Home](../README.md) [Install](./install.md) \
                     [Usage](Readme.md#usage)\n",
                ),
                (
                    "docs/guide/install.md",
                    "# Install\n\n## From source\n\n[Usage](README.md#usage) [Home](/README.md)\n",
                ),
            ],
        );
        assert_eq!(
            broken,
            [
                "broken link guide/install.md#from-sorce in docs/README.md: Cannot find fragment"
                    .to_string(),
                format!("broken link guide/uninstall.md in docs/README.md: {NOT_FOUND}"),
            ]
        );
    }

    #[test]
    fn shows_a_link_in_markdown_as_its_written() {
        let broken = broken_links(
            "written",
            Scope::Internal,
            &[
                (
                    "little.yml",
                    "theme: ./theme\nurl: https://withlittle.github.io/little\n",
                ),
                BARE_THEME[1],
                // Each way a link is written is a warning, which names each
                // file it's written in that way.
                (
                    "docs/README.md",
                    "# Home\n\n[Usage](guide/#usge) [Usage](/guide/#usge)\n",
                ),
                ("docs/about.md", "# About\n\n[Usage](/guide/#usge)\n"),
                (
                    "docs/guide/README.md",
                    "# Guide\n\n## Usage\n\n[Usage](#usge)\n",
                ),
            ],
        );
        assert_eq!(
            broken,
            [
                "broken link guide/#usge in docs/README.md: Cannot find fragment",
                "broken link /guide/#usge in docs/README.md, docs/about.md: Cannot find fragment",
                "broken link #usge in docs/guide/README.md: Cannot find fragment",
            ]
        );
    }

    #[test]
    fn shows_a_page_at_its_url() {
        let broken = broken_links(
            "shown",
            Scope::Internal,
            &[
                BARE_THEME[0],
                BARE_THEME[1],
                // Copied as it is, so its links are shown as a reader follows
                // them. Both `/guide#nope` and `guide/index.html#nope` show as
                // `/guide/#nope`, so they're one warning.
                (
                    "docs/old.html",
                    "<a href=\"/guide/install/#nope\">Install</a> \
                     <a href=\"/guide#nope\">Guide</a> \
                     <a href=\"guide/index.html#nope\">Guide</a> \
                     <a href=\"/images/\">Images</a> <a href=\"#café\">Café</a>",
                ),
                ("docs/README.md", "# Home\n"),
                ("docs/guide/README.md", "# Guide\n"),
                ("docs/guide/install.md", "# Install\n"),
                ("docs/images/logo.png", "png"),
            ],
        );
        assert_eq!(
            broken,
            [
                "broken link /guide/#nope in docs/old.html: Cannot find fragment",
                "broken link /guide/install#nope in docs/old.html: Cannot find fragment",
                "broken link /images/ in docs/old.html: Cannot find index file within directory",
                "broken link /old.html#café in docs/old.html: Cannot find fragment",
            ]
        );
    }

    #[test]
    fn checks_a_site_under_its_path_as_its_published() {
        let broken = broken_links(
            "path",
            Scope::Internal,
            &[
                (
                    "little.yml",
                    "theme: ./theme\nurl: https://withlittle.github.io/little\n",
                ),
                // The second stylesheet leaves out the site's path.
                (
                    "theme/_templates/default.html",
                    "<link rel=\"stylesheet\" href=\"{{ '/style.css' | url }}\">\
                     <link rel=\"stylesheet\" href=\"/style.css\">\
                     {{ page.content | safe }}",
                ),
                ("theme/_static/style.css", "body {}\n"),
                (
                    "docs/README.md",
                    "# Home\n\n[Usage](/guide/#usage) \
                     [Usage](https://withlittle.github.io/little/guide/#usage) \
                     [Usage](https://withlittle.github.io/little/guide/#usge)\n",
                ),
                ("docs/guide/README.md", "# Guide\n\n## Usage\n"),
            ],
        );
        assert_eq!(
            broken,
            [
                "broken link https://withlittle.github.io/little/guide/#usge in docs/README.md: \
                 Cannot find fragment"
                    .to_string(),
                format!(
                    "broken link /style.css in docs/README.md, docs/guide/README.md: {NOT_FOUND}"
                ),
            ]
        );
    }

    #[test]
    fn finds_a_link_thats_not_a_url() {
        let broken = broken_links(
            "not-a-url",
            Scope::Internal,
            &[
                BARE_THEME[0],
                BARE_THEME[1],
                ("docs/README.md", "[Example](<https://exa mple.com>)\n"),
            ],
        );
        assert_eq!(broken.len(), 1, "{broken:?}");
        assert!(
            broken[0].starts_with("broken link https://exa mple.com in docs/README.md: "),
            "{broken:?}"
        );
    }

    #[test]
    fn leaves_links_out_of_the_site_to_a_build() {
        let broken = broken_links(
            "internal",
            Scope::Internal,
            &[
                BARE_THEME[0],
                BARE_THEME[1],
                (
                    "docs/README.md",
                    "[Gone](https://github.com/withlittle/no-such-repo) \
                     [Mail](mailto:hello@withlittle.dev)\n",
                ),
            ],
        );
        assert_eq!(broken, Vec::<String>::new());
    }

    #[test]
    fn leaves_out_links_that_work_only_on_the_authors_machine() {
        let broken = broken_links(
            "private",
            Scope::All,
            &[
                BARE_THEME[0],
                BARE_THEME[1],
                (
                    "docs/README.md",
                    "[Preview](http://127.0.0.1:1/) [Preview](http://localhost:1/) \
                     [Router](http://192.168.0.1:1/)\n",
                ),
            ],
        );
        assert_eq!(broken, Vec::<String>::new());
    }

    #[test]
    fn leaves_out_links_into_the_sites_repo() {
        // The built-in theme links to the repository, and each page to its
        // file there, which no repository has, so none can be checked.
        let repo = "https://github.com/withlittle/no-such-repo";
        let broken = broken_links(
            "repo",
            Scope::All,
            &[
                ("little.yml", &format!("repo: {repo}/\n")),
                (
                    "docs/README.md",
                    &format!(
                        "# Home\n\n[Open issues]({repo}/issues?q=is%3Aopen) [Read me]({repo}#readme)\n"
                    ),
                ),
                ("docs/guide.md", "# Guide\n"),
            ],
        );
        assert_eq!(broken, Vec::<String>::new());

        // A link is in the repository if it's to it, or under it, as lychee
        // writes it.
        let into = into_repo(&Repo {
            url: "https://GitHub.com/withlittle/little".into(),
            branch: "main".into(),
        })
        .unwrap();
        let matches = |url: &str| into.is_match(Url::parse(url).unwrap().as_str());
        for url in [
            "https://github.com/withlittle/little",
            "https://github.com/withlittle/little/",
            "https://GITHUB.COM/withlittle/little/blob/main/docs/README.md",
            "https://github.com/withlittle/little?tab=readme-ov-file",
            "https://github.com/withlittle/little#readme",
        ] {
            assert!(matches(url), "{url}");
        }
        for url in [
            "https://github.com/withlittle/little-other",
            "https://github.com/withlittle",
            "http://github.com/withlittle/little",
            "https://github.com/WithLittle/Little",
        ] {
            assert!(!matches(url), "{url}");
        }
    }

    #[test]
    fn finds_no_broken_links_in_the_built_in_theme() {
        for config in ["", "url: https://withlittle.github.io/little\n"] {
            let broken = broken_links(
                "built-in",
                Scope::Internal,
                &[
                    ("little.yml", config),
                    ("docs/README.md", "# Home\n\n## Usage\n\n### Options\n"),
                    ("docs/guide/README.md", "# Guide\n\n## Install\n"),
                    ("docs/guide/install.md", "# Install\n\n## From source\n"),
                    ("docs/about.md", "# About\n"),
                ],
            );
            assert_eq!(broken, Vec::<String>::new(), "with little.yml {config:?}");
        }
    }

    #[test]
    fn names_a_few_of_the_files_a_link_is_in() {
        let broken = |files: usize| Broken {
            files: (1..=files).map(|n| format!("docs/{n}.md")).collect(),
            url: "/guide/".to_string(),
            reason: "Cannot find fragment".to_string(),
        };
        assert_eq!(
            broken(1).to_string(),
            "broken link /guide/ in docs/1.md: Cannot find fragment"
        );
        assert_eq!(
            broken(3).to_string(),
            "broken link /guide/ in docs/1.md, docs/2.md, docs/3.md: Cannot find fragment"
        );
        assert_eq!(
            broken(5).to_string(),
            "broken link /guide/ in docs/1.md, docs/2.md, docs/3.md and 2 more: \
             Cannot find fragment"
        );
    }
}
