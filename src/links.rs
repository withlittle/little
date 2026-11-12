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
//! An anchor in one of the site's own files, like the `#usage` of
//! `guide/#usage`, little looks for itself, among the file's `id`s: lychee
//! checks that the file's there, and little reads each file once, where
//! lychee would read it once for each link into it.
//!
//! [`check`] checks a build from nothing, as `little build` does. A
//! [`Checker`] checks build after build, as `little serve` does, keeping
//! what it read from each file, so it reads only those a rebuild wrote.
//!
//! A broken link in a page's Markdown is shown as it's written there, so an
//! author can find it, and any other, like one in the theme, as a reader
//! follows it.
//!
//! A link to the site's repository, or to anything in it, isn't checked: a
//! theme can link to each page's file there, from every page, and a new
//! page's file isn't there until it's pushed, nor any in a private
//! repository to a checker that hasn't signed in.
//!
//! Nor is a link to another site from a version of the site built from a
//! git ref: the web changes, and a tag can't be fixed. Its links within the
//! site are checked.

use anyhow::{Context, Result, anyhow};
use futures::StreamExt;
use html5gum::{Token, Tokenizer};
use lychee_lib::remap::Remaps;
use lychee_lib::{
    BaseInfo, Client, ClientBuilder, Collector, ErrorKind, FileType, FragmentCheckerOptions, Input,
    InputSource, Request, RequestError, ResolvedInputSource, Status, Uri,
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
    Checker::default().check(built, scope)
}

/// A site's links, checked build after build, as `little serve` checks
/// them. It keeps the links it finds in each file, and each file's anchors,
/// so after a [`site::rebuild`](crate::site::rebuild) it reads only the
/// files that wrote. Every link is still checked each time, as one in a
/// file that hasn't changed can lead to one that has.
#[derive(Default)]
pub struct Checker {
    /// The destination of the builds it's read, as an absolute path, or
    /// `None` if it's read none, or its last check failed.
    destination: Option<PathBuf>,
    /// Each link it's found, once, and where each is in `links`, by its URL.
    links: Vec<Request>,
    ids: HashMap<String, usize>,
    /// The links in each HTML file, by its path in the destination.
    files: HashMap<PathBuf, Links>,
    /// The anchors in each HTML file a link leads into, by its path, or
    /// `None` for one that can't be read.
    anchors: HashMap<PathBuf, Option<HashSet<String>>>,
}

/// The links in a file.
#[derive(Default)]
struct Links {
    /// Where each is in [`Checker::links`].
    found: Vec<u32>,
    /// Each that isn't a URL at all, like `https://exa mple.com`, as it's
    /// written, and why it isn't.
    not_urls: Vec<(String, String)>,
}

impl Checker {
    /// The broken links in `built`, sorted by the files they're in. After
    /// the first, `built` must be the build after the one it last checked.
    pub fn check(&mut self, built: &Built, scope: Scope) -> Result<Vec<Broken>> {
        let broken = tokio::runtime::Runtime::new()
            .context("starting the link checker")
            .and_then(|runtime| runtime.block_on(self.check_site(&Site::new(built)?, scope)));
        // What it's read may be half of a build, so the next reads it all.
        if broken.is_err() {
            *self = Self::default();
        }
        broken
    }

    /// Read the links in the files of `site` that its build wrote, or in
    /// all of them, the first time, and forget the files it didn't leave.
    async fn read(&mut self, site: &Site<'_>) -> Result<()> {
        let built = site.built;
        let file = |path: PathBuf| Input {
            source: InputSource::FsPath(path),
            file_type_hint: None,
        };
        let inputs: HashSet<Input> = if self.destination.as_ref() == Some(&site.destination) {
            let kept =
                |file: &Path| built.files.contains_key(file) && !built.written.contains(file);
            self.files.retain(|file, _| kept(file));
            self.anchors
                .retain(|file, _| file.strip_prefix(&site.destination).is_ok_and(kept));
            // Only the files lychee would read in the folder: HTML, and not
            // hidden.
            (built.written.iter())
                .filter(|path| FileType::from(path) == FileType::Html)
                .filter(|path| !crate::site::is_hidden(path))
                .map(|path| file(site.destination.join(path)))
                .collect()
        } else {
            *self = Self {
                destination: Some(site.destination.clone()),
                ..Self::default()
            };
            HashSet::from([file(site.destination.clone())])
        };
        if inputs.is_empty() {
            return Ok(());
        }
        let requests = Collector::new(Some(site.root.clone()), BaseInfo::none())?
            // A destination git ignores, as `dist/` usually is, is still checked.
            .skip_ignored(false)
            .collect_links_from_file_types(inputs, FileType::html_extensions());
        let mut requests = std::pin::pin!(requests);
        while let Some(request) = requests.next().await {
            match request {
                Ok(request) => {
                    let Some(file) = site.in_destination(&request.source) else {
                        continue;
                    };
                    let id = match self.ids.get(request.uri.as_str()) {
                        Some(id) => *id,
                        None => {
                            let id = self.links.len();
                            self.ids.insert(request.uri.as_str().to_string(), id);
                            self.links.push(request.clone());
                            id
                        }
                    };
                    let links = self.files.entry(file.to_path_buf()).or_default();
                    links
                        .found
                        .push(u32::try_from(id).context("counting the site's links")?);
                }
                Err(RequestError::CreateRequestItem(link, source, error)) => {
                    if let Some(file) = site.in_destination(&source) {
                        let links = self.files.entry(file.to_path_buf()).or_default();
                        links.not_urls.push((link.text, error.to_string()));
                    }
                }
                Err(error) => return Err(error).context("reading the built site's links"),
            }
        }
        Ok(())
    }

    async fn check_site(&mut self, site: &Site<'_>, scope: Scope) -> Result<Vec<Broken>> {
        self.read(site).await?;
        // Each link is checked once, however many files it's in, but for
        // one to another site from a version of the site from a git ref.
        let mut elsewhere: Vec<Option<bool>> = vec![None; self.links.len()];
        let links = &self.links;
        let mut checked = |from_ref: bool, id: u32| {
            let id = id as usize;
            !(from_ref && *elsewhere[id].get_or_insert_with(|| site.elsewhere(&links[id].uri)))
        };
        let mut found = vec![false; self.links.len()];
        for (file, links) in &self.files {
            let from_ref = site.built.from_refs.contains(file);
            for &id in &links.found {
                if checked(from_ref, id) {
                    found[id as usize] = true;
                }
            }
        }
        let found = (found.iter().enumerate()).filter_map(|(id, found)| found.then_some(id));

        let client = site.client(scope)?;
        // A link to somewhere in one of the site's pages, like `guide/#usage`,
        // is checked in two steps: lychee checks that the page is there, once
        // for all the links to it, and little that the page has the anchor,
        // reading each page once, as lychee would read it once for each link.
        let mut plain = Vec::new();
        let mut anchored = Vec::new();
        let mut pages: BTreeMap<String, Request> = BTreeMap::new();
        for id in found {
            let request = self.links[id].clone();
            match Anchor::of(&client, &request) {
                Some(anchor) => {
                    let mut page = request.clone();
                    page.uri = Uri::from(anchor.page.clone());
                    pages.entry(anchor.page.to_string()).or_insert(page);
                    anchored.push((id, request.uri, anchor));
                }
                None => plain.push((id, request)),
            }
        }
        let checked_plain: Vec<_> = futures::stream::iter(plain)
            .map(|(id, request)| {
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
                    Some((id, (site.shown(&link, &checked), reason?)))
                }
            })
            .buffer_unordered(AT_ONCE)
            .collect()
            .await;
        // Why each page a link leads into can't be reached, or else its file,
        // if it's one with anchors to find.
        let pages: HashMap<String, Result<Option<PathBuf>, String>> = futures::stream::iter(pages)
            .map(|(url, request)| {
                let client = &client;
                async move {
                    let file = match client.check(request).await {
                        Ok(response) if response.status().is_error() => {
                            Err(response.status().to_string())
                        }
                        Ok(response) if response.status().is_success() => Ok(html_file(&url)),
                        Ok(_) => Ok(None),
                        Err(error) => Err(error.to_string()),
                    };
                    (url, file)
                }
            })
            .buffer_unordered(AT_ONCE)
            .collect()
            .await;
        let files: BTreeSet<PathBuf> = (pages.values().flatten().flatten())
            .filter(|file| !self.anchors.contains_key(*file))
            .cloned()
            .collect();
        let threads = std::thread::available_parallelism().map_or(1, usize::from);
        let read: Vec<_> = futures::stream::iter(files)
            .map(|file| async move {
                let read = file.clone();
                let anchors = tokio::task::spawn_blocking(move || anchors_in(&read)).await;
                (file, anchors.ok().flatten())
            })
            .buffer_unordered(threads)
            .collect()
            .await;
        self.anchors.extend(read);
        let anchors = &self.anchors;
        let anchored = anchored.into_iter().filter_map(|(id, link, anchor)| {
            let reason = match pages.get(anchor.page.as_str())? {
                Err(reason) => reason.clone(),
                Ok(file) => {
                    // A file that can't be read is passed over, as lychee does.
                    let anchors = anchors.get(file.as_ref()?)?.as_ref()?;
                    if anchor.is_in(anchors) {
                        return None;
                    }
                    Status::from(ErrorKind::InvalidFragment(anchor.link.clone())).to_string()
                }
            };
            Some((id, (site.shown(&link, &anchor.link), reason)))
        });
        // Each broken link, as it's shown, and why it's broken.
        let reasons: HashMap<usize, (String, String)> =
            (checked_plain.into_iter().flatten().chain(anchored)).collect();

        // The files each broken link is in, by the link as it's shown and why
        // it's broken: one written alike in many files is one warning, as are
        // two that look alike once shown, like `/guide/#usage` and
        // `/guide/index.html#usage`, as a reader looks for one.
        let mut broken: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
        for (file, links) in &self.files {
            for (link, reason) in &links.not_urls {
                for (origin, written) in site.found(file, link) {
                    let url = written.unwrap_or_else(|| link.clone());
                    (broken.entry((url, reason.clone())).or_default()).insert(origin);
                }
            }
            if reasons.is_empty() {
                continue;
            }
            let from_ref = site.built.from_refs.contains(file);
            for &id in &links.found {
                let Some((shown, reason)) = reasons.get(&(id as usize)) else {
                    continue;
                };
                if !checked(from_ref, id) {
                    continue;
                }
                for (origin, written) in site.found(file, self.links[id as usize].uri.as_str()) {
                    let url = written.unwrap_or_else(|| shown.clone());
                    (broken.entry((url, reason.clone())).or_default()).insert(origin);
                }
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
}

/// A link to an anchor in a file, like `guide/#usage`, which little looks
/// for itself.
struct Anchor {
    /// The link, as lychee would check it: led into the destination, if
    /// it's to the site's own `url`, or from the root of the domain.
    link: Uri,
    /// The link without its fragment: the file, or its folder.
    page: Url,
    /// The fragment, as the link writes it, and, if it's encoded, like
    /// `caf%C3%A9`, decoded too: a page's `id` can be either.
    names: Vec<String>,
}

impl Anchor {
    /// The anchor `request` links to, if little's to look for it: one in a
    /// file, that isn't `#top`, which every page has. A link to text in a
    /// page, like `#:~:text=words`, or into a Markdown file, which has no
    /// `id`s, is left to lychee, as is any that `client` leaves out.
    fn of(client: &Client, request: &Request) -> Option<Self> {
        let mut link = request.uri.clone();
        client.remap(&mut link).ok()?;
        let url = Url::parse(link.as_str()).ok()?;
        let fragment = url.fragment()?;
        let checked = link.is_file()
            && !client.is_excluded(&link)
            && !fragment.is_empty()
            && !fragment.eq_ignore_ascii_case("top")
            && !fragment.contains(":~:")
            && FileType::from(url.path()) != FileType::Markdown;
        if !checked {
            return None;
        }
        let mut names = vec![fragment.to_string()];
        let decoded = percent_decode_str(fragment).decode_utf8().ok()?;
        if decoded != fragment {
            names.push(decoded.into_owned());
        }
        let mut page = url.clone();
        page.set_fragment(None);
        Some(Self { link, page, names })
    }

    /// Whether it's one of a file's `anchors`.
    fn is_in(&self, anchors: &HashSet<String>) -> bool {
        self.names.iter().any(|name| anchors.contains(name))
    }
}

/// The HTML file a browser shows at `url`, a `file` URL of a file or folder
/// that's there: the file, or its folder's `index.html`. `None` for any
/// other file, like an image, which has no anchors to find.
fn html_file(url: &str) -> Option<PathBuf> {
    let mut path = Url::parse(url).ok()?.to_file_path().ok()?;
    if path.is_dir() {
        path.push("index.html");
    }
    (FileType::from(&path) == FileType::Html).then_some(path)
}

/// Each anchor in the HTML `file`: the `id` or `name` of any of its
/// elements, as lychee finds them. `None` if it can't be read.
fn anchors_in(file: &Path) -> Option<HashSet<String>> {
    let html = std::fs::read_to_string(file).ok()?;
    let mut anchors = HashSet::new();
    for token in Tokenizer::new(&html).flatten() {
        if let Token::StartTag(tag) = token {
            for name in ["id", "name"] {
                if let Some(value) = tag.attributes.get(name.as_bytes()) {
                    anchors.insert(String::from_utf8_lossy(value).into_owned());
                }
            }
        }
    }
    Some(anchors)
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
    /// What matches a link to the site's own `url`, or to anything under
    /// it, if it has one.
    own: Option<Regex>,
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
        // Made as lychee makes URLs, so they're encoded alike.
        let own = match built.url.origin.as_str() {
            "" => None,
            origin => Some(under(&Url::parse(&format!("{origin}{}", built.url.path))?)?),
        };
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
            own,
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
        let root = dir_url(&self.root)?.join(site.path.trim_start_matches('/'))?;
        let mut patterns = vec![under(&root)?];
        patterns.extend(self.own.clone());
        // `$` would start a capture group in a replacement.
        let into = format!(
            "{}${{1}}",
            trimmed(&dir_url(&self.destination)?).replace('$', "$$")
        );
        let remaps = (patterns.into_iter())
            .map(|pattern| (pattern, into.clone()))
            .collect();
        Ok(Remaps::new(remaps))
    }

    /// Whether `link` is to another site, which isn't checked from a file
    /// of a version of the site from a git ref.
    fn elsewhere(&self, link: &Uri) -> bool {
        let own = |own: &Regex| own.is_match(link.as_str());
        !link.is_file() && !self.own.as_ref().is_some_and(own)
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

    /// What the file at `rel` in the destination was built from.
    fn origin(&self, rel: &Path) -> String {
        (self.built.origins.get(rel).cloned())
            .unwrap_or_else(|| self.destination.join(rel).display().to_string())
    }

    /// The file `link` is in, at `rel` in the destination, once for each
    /// way its Markdown writes the link, or once if it doesn't. `link` is
    /// the URL lychee made of it, or, if it isn't a URL, its text.
    fn found(&self, rel: &Path, link: &str) -> Vec<Found> {
        let file = self.origin(rel);
        let written = self.written.get(rel).and_then(|written| written.get(link));
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

/// What matches a link to `url`, or to anything under it.
fn under(url: &Url) -> Result<Regex> {
    Ok(Regex::new(&format!(
        "^{}([/?#]|$)",
        regex::escape(trimmed(url))
    ))?)
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
    use crate::testing::scratch;
    use std::fs;

    /// A theme that shows only each page's content, so a test sees only the
    /// links it writes.
    const BARE_THEME: [(&str, &str); 2] = [
        ("little.yml", "theme: ./theme\n"),
        ("theme/_templates/default.html", "{{ page.content | safe }}"),
    ];

    const NOT_FOUND: &str = "File not found. Check if file exists and path is correct";

    /// Build the site holding `files`, and warn of each broken link in
    /// `scope`, naming each file by its path in the site, written with `/`
    /// on any system.
    fn broken_links(scope: Scope, files: &[(&str, &str)]) -> Vec<String> {
        let (_tmp, site) = scratch(files);
        let built = site::build(&site).unwrap();
        let broken = check(&built, scope).unwrap();
        let in_site = |file: &str| {
            let names: Vec<_> = Path::new(file)
                .strip_prefix(&site)
                .ok()?
                .iter()
                .map(|name| name.to_string_lossy())
                .collect();
            Some(names.join("/"))
        };
        broken
            .into_iter()
            .map(|mut link| {
                for file in &mut link.files {
                    if let Some(rel) = in_site(file) {
                        *file = rel;
                    }
                }
                link.to_string()
            })
            .collect()
    }

    #[test]
    fn a_checker_finds_what_a_check_would_build_after_build() {
        let (_tmp, site) = scratch(&[
            BARE_THEME[0],
            BARE_THEME[1],
            (
                "docs/README.md",
                "[Usage](guide.md#usage), and [about](about.md).\n",
            ),
            ("docs/guide.md", "# Guide\n\n## Usage\n"),
            ("docs/about.md", "# About\n"),
        ]);
        let mut checker = Checker::default();
        // The links the checker finds broken, which a check of the same
        // build, from nothing, must find too.
        let mut broken = |built: &Built| {
            let broken = checker.check(built, Scope::Internal).unwrap();
            assert_eq!(broken, check(built, Scope::Internal).unwrap());
            let shown = broken
                .iter()
                .map(|link| format!("{}: {}", link.url, link.reason));
            shown.collect::<Vec<_>>()
        };
        let write = |file: &str, text: &str| fs::write(site.join(file), text).unwrap();
        let built = site::build(&site).unwrap();
        assert_eq!(broken(&built), [""; 0]);

        // A heading goes from a page, which a page that's as it was links to.
        write("docs/guide.md", "# Guide\n\n## Using\n");
        let built = site::rebuild(&site, &built).unwrap();
        assert!(!built.written.contains(Path::new("index.html")));
        assert_eq!(
            broken(&built),
            ["guide.md#usage: Cannot find fragment".to_string()]
        );

        // It comes back, and a page goes, as another comes with links of
        // its own, one of them not a URL.
        write("docs/guide.md", "# Guide\n\n## Usage\n");
        fs::remove_file(site.join("docs/about.md")).unwrap();
        write(
            "docs/new.md",
            "[Nowhere](nowhere.md), and [not a URL](<https://exa mple.com>).\n",
        );
        let built = site::rebuild(&site, &built).unwrap();
        let found = broken(&built);
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found[0].starts_with("about.md: "), "{found:?}");
        assert!(found[1].starts_with("https://exa mple.com: "), "{found:?}");
        assert!(found[2].starts_with("nowhere.md: "), "{found:?}");

        // The new page goes, with its links, and the other comes back.
        fs::remove_file(site.join("docs/new.md")).unwrap();
        write("docs/about.md", "# About\n");
        let built = site::rebuild(&site, &built).unwrap();
        assert_eq!(broken(&built), [""; 0]);

        // A file the build didn't write isn't read again.
        let built = site::rebuild(&site, &built).unwrap();
        assert!(built.written.is_empty());
        let home = site.join("dist/index.html");
        fs::write(&home, "<a href=\"nowhere\">Nowhere</a>").unwrap();
        assert_eq!(checker.check(&built, Scope::Internal).unwrap(), []);
        assert_eq!(check(&built, Scope::Internal).unwrap().len(), 1);
    }

    #[test]
    fn finds_pages_files_and_fragments_that_arent_there() {
        let broken = broken_links(
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
    fn finds_an_anchor_by_its_id_or_name_encoded_or_not() {
        let broken = broken_links(
            Scope::Internal,
            &[
                BARE_THEME[0],
                BARE_THEME[1],
                // Copied as it is. Its links to anchors that are there, to
                // the top of a page, to text in one, and to a file with no
                // anchors, aren't broken.
                (
                    "docs/old.html",
                    "<h2 id=\"café\">Café</h2> <a name=\"named\"></a> \
                     <a href=\"#café\">Café</a> <a href=\"#caf%C3%A9\">Café</a> \
                     <a href=\"#named\">Named</a> <a href=\"#top\">Top</a> \
                     <a href=\"/guide/#TOP\">Top</a> <a href=\"/guide/#:~:text=Guide\">Text</a> \
                     <a href=\"/logo.png#here\">Logo</a> <a href=\"/guide/#Guide\">Guide</a> \
                     <a href=\"/images/#here\">Images</a> <a href=\"/nowhere#here\">Nowhere</a>",
                ),
                ("docs/README.md", "# Home\n"),
                ("docs/guide/README.md", "# Guide\n"),
                ("docs/logo.png", "png"),
                ("docs/images/logo.png", "png"),
            ],
        );
        assert_eq!(
            broken,
            [
                "broken link /guide/#Guide in docs/old.html: Cannot find fragment",
                "broken link /images/#here in docs/old.html: Cannot find index file within directory",
                format!("broken link /nowhere#here in docs/old.html: {NOT_FOUND}").as_str(),
            ]
        );
    }

    #[test]
    fn checks_a_site_under_its_path_as_its_published() {
        let broken = broken_links(
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
    fn checks_only_links_within_the_site_from_a_git_ref() {
        use crate::git::tests::{commit_all, git_in, init};
        // A link to another site from a version from a ref isn't checked, as
        // the ref can't be fixed, but one within the site is, even to the
        // site's own `url`.
        let (_tmp, site) = scratch(&[
            BARE_THEME[1],
            (".gitignore", "dist/\n"),
            (
                "docs/README.md",
                "[Elsewhere](https://example.invalid/) [Gone](gone.md) \
                     [Missing](https://example.com/proj/1.4/missing)\n",
            ),
        ]);
        init(&site);
        commit_all(&site);
        git_in(&site, &["tag", "v1.4"]);
        let site_cfg = "theme: ./theme\nurl: https://example.com/proj\nversions:\n  \
                        - { title: \"2.0\", url: / }\n  \
                        - { title: \"1.4\", url: /1.4, ref: v1.4 }\n";
        fs::write(site.join("little.yml"), site_cfg).unwrap();
        fs::write(site.join("docs/README.md"), "# Home\n").unwrap();

        let built = site::build(&site).unwrap();
        let broken = check(&built, Scope::All).unwrap();
        let broken: Vec<_> = broken.iter().map(ToString::to_string).collect();
        assert_eq!(
            broken,
            [
                format!("broken link gone.md in v1.4:docs/README.md: {NOT_FOUND}"),
                format!(
                    "broken link https://example.com/proj/1.4/missing in v1.4:docs/README.md: \
                     {NOT_FOUND}"
                ),
            ]
        );
    }

    #[test]
    fn finds_no_broken_links_in_the_built_in_theme() {
        for config in ["", "url: https://withlittle.github.io/little\n"] {
            let broken = broken_links(
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
