//! A tiny dev server with live reload.
//!
//! Strategy: serve the site's destination over HTTP, expose a
//! `/__little/version` counter, and inject a small polling script into every
//! HTML page. A file watcher rebuilds the site and bumps the counter, so open
//! browsers reload themselves.
//!
//! A site whose `url` puts it under a path, like `/my-project`, is served
//! under it, as it's published, so a link that leaves the path out is broken
//! here too. The root of the domain sends the browser to the site.

use anyhow::{Result, anyhow};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use percent_encoding::percent_decode_str;
use std::ffi::OsStr;
use std::io;
use std::net::Ipv6Addr;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tiny_http::{Header, Response, Server};

use crate::site::Built;
use crate::theme::Location;
use crate::{config, site};

/// Serve the site at `host` and `port`, or where its config says for either
/// that's `None`.
pub fn serve(site_dir: &Path, host: Option<String>, port: Option<u16>) -> Result<()> {
    let site_dir = site_dir.to_path_buf();

    let cfg = config::load(&site_dir)?;
    let host = host.unwrap_or(cfg.host);
    let port = port.unwrap_or(cfg.port);

    // The last build that worked. A rebuild can move its folder, or the path
    // it's served under.
    let built = site::build(&site_dir)?;
    let home = match built.url.path.as_str() {
        "" => String::new(),
        path => format!("{path}/"),
    };
    let built = Arc::new(Mutex::new(built));

    let version = Arc::new(AtomicU64::new(1));
    spawn_watcher(site_dir.clone(), built.clone(), version.clone());

    let addr = address(&host, port);
    let server =
        Server::http((host.as_str(), port)).map_err(|e| anyhow!("failed to bind {addr}: {e}"))?;
    println!("little serving http://{addr}{home}  (live reload on — edit your docs or the theme)");

    for request in server.incoming_requests() {
        if request.url().starts_with("/__little/version") {
            let body = version.load(Ordering::SeqCst).to_string();
            let _ = request.respond(Response::from_string(body));
            continue;
        }
        let built = built.lock().unwrap().clone();
        serve_file(&built, request);
    }
    Ok(())
}

/// `host` and `port` as a URL writes them. An IPv6 address goes in brackets,
/// like `[::1]:8080`, so its colons aren't taken for the port's.
fn address(host: &str, port: u16) -> String {
    if host.parse::<Ipv6Addr>().is_ok() {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn spawn_watcher(site_dir: PathBuf, last: Arc<Mutex<Built>>, version: Arc<AtomicU64>) {
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut sources = match Sources::watch(&site_dir, tx) {
            Ok(sources) => sources,
            Err(e) => {
                eprintln!("watcher error: {e}");
                return;
            }
        };

        // Debounce: wake on the first event, then swallow the burst.
        while rx.recv().is_ok() {
            std::thread::sleep(Duration::from_millis(150));
            while rx.try_recv().is_ok() {}

            // The edit may have moved the site's folders or switched its theme.
            if let Err(e) = sources.follow(&site_dir) {
                eprintln!("watcher error: {e}");
            }

            match site::build(&site_dir) {
                Ok(built) => {
                    *last.lock().unwrap() = built;
                    version.fetch_add(1, Ordering::SeqCst);
                    println!("rebuilt");
                }
                Err(e) => eprintln!("build error: {e:#}"),
            }
        }
    });
}

/// A folder a build reads, and the destination, which can be inside it.
type Watched = (PathBuf, PathBuf);

/// Watchers on everything a build reads, sending events as it changes.
struct Sources {
    tx: Sender<notify::Result<Event>>,
    /// The site's config, which chooses the folders below.
    _config: RecommendedWatcher,
    /// The source folder, and the folder of each theme in the chain that
    /// isn't built in or downloaded.
    source: Option<(Watched, RecommendedWatcher)>,
    themes: Vec<(Watched, RecommendedWatcher)>,
}

impl Sources {
    fn watch(site_dir: &Path, tx: Sender<notify::Result<Event>>) -> notify::Result<Self> {
        let mut sources = Self {
            _config: watch_config(site_dir, tx.clone())?,
            tx,
            source: None,
            themes: Vec::new(),
        };
        sources.follow(site_dir)?;
        Ok(sources)
    }

    /// Watch the folders the site's config chooses, unless they already are.
    fn follow(&mut self, site_dir: &Path) -> notify::Result<()> {
        // A config that doesn't work, or a source folder that isn't there,
        // fails the build, which says why; keep watching the last folders
        // until it's fixed.
        let Ok(cfg) = config::load(site_dir) else {
            return Ok(());
        };
        let source = (cfg.source_dir, cfg.destination_dir.clone());
        let moved = self
            .source
            .as_ref()
            .is_none_or(|(watched, _)| *watched != source);
        if moved && source.0.is_dir() {
            let watcher = watch_folder(&source.0, &source.1, true, self.tx.clone())?;
            self.source = Some((source, watcher));
        }
        // The built-in theme is in the binary, and a downloaded theme never
        // changes, so only the folders of the other themes are watched.
        let themes: Vec<Watched> = cfg
            .themes
            .iter()
            .filter_map(|theme| match theme {
                Location::Folder(dir) => Some((dir.clone(), cfg.destination_dir.clone())),
                Location::BuiltIn | Location::Downloaded { .. } => None,
            })
            .collect();
        if !themes
            .iter()
            .eq(self.themes.iter().map(|(watched, _)| watched))
        {
            let mut watchers = Vec::new();
            for theme in themes {
                let watcher = watch_folder(&theme.0, &theme.1, false, self.tx.clone())?;
                watchers.push((theme, watcher));
            }
            self.themes = watchers;
        }
        Ok(())
    }
}

/// Watch the site's config. It's optional and may not exist yet, so watch the
/// site's folder rather than the file, forwarding only changes to the config
/// (the folder may hold the destination too, which every build recreates).
fn watch_config(
    site_dir: &Path,
    tx: Sender<notify::Result<Event>>,
) -> notify::Result<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if res.as_ref().is_ok_and(changes_site_config) {
            let _ = tx.send(res);
        }
    })?;
    watcher.watch(site_dir, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

/// Watch a folder a build reads: the source, or a theme's folder, its
/// optional config included. Either can hold the destination (`docs/_build`,
/// or a theme's demo site that sets `theme: ..`), so the build's own output is
/// ignored, or each build would trigger the next. With `skip_hidden`, as for
/// the source, hidden files are ignored too, as a build leaves them out; a
/// theme copies its hidden static files, so they count.
fn watch_folder(
    dir: &Path,
    destination: &Path,
    skip_hidden: bool,
    tx: Sender<notify::Result<Event>>,
) -> notify::Result<RecommendedWatcher> {
    // Real paths on both sides: some platforms (macOS) report events by them.
    let dir = dir.canonicalize()?;
    let destination = real_path(destination)?;
    let root = dir.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if !res.as_ref().is_ok_and(|event| {
            is_read(event)
                || is_within(event, &destination)
                || (skip_hidden && is_hidden_in(event, &root))
        }) {
            let _ = tx.send(res);
        }
    })?;
    watcher.watch(&dir, RecursiveMode::Recursive)?;
    Ok(watcher)
}

/// The absolute path `path` really is, even if it doesn't exist yet: what
/// exists is resolved as the system would (following `..` and symlinks, and
/// spelled as on disk), and the rest is taken as written.
fn real_path(path: &Path) -> io::Result<PathBuf> {
    if let Ok(real) = path.canonicalize() {
        return Ok(real);
    }
    let mut real = PathBuf::new();
    for component in std::path::absolute(path)?.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                real.pop();
            }
            Component::Normal(name) => {
                real.push(name);
                if let Ok(canonical) = real.canonicalize() {
                    real = canonical;
                }
            }
            root => real.push(root),
        }
    }
    Ok(real)
}

/// Is this event just a read? Every build opens the files it uses, and some
/// platforms report opens (Linux's inotify does), so a watcher that counted
/// reads as changes would trigger endless rebuilds.
fn is_read(event: &Event) -> bool {
    event.kind.is_access()
}

/// Does this event only touch paths inside `dir`?
fn is_within(event: &Event, dir: &Path) -> bool {
    !event.paths.is_empty() && event.paths.iter().all(|path| path.starts_with(dir))
}

/// Does this event only touch hidden files in the source folder `source`?
/// A file saved by renaming a hidden one over it, as some editors do, isn't
/// hidden, so that event still counts.
fn is_hidden_in(event: &Event, source: &Path) -> bool {
    !event.paths.is_empty()
        && event
            .paths
            .iter()
            .all(|path| path.strip_prefix(source).is_ok_and(site::is_hidden))
}

/// Does this root-folder event change the site's config file?
fn changes_site_config(event: &Event) -> bool {
    !is_read(event)
        && event
            .paths
            .iter()
            .any(|path| path.file_name() == Some(OsStr::new(config::CONFIG_FILE)))
}

fn serve_file(built: &Built, request: tiny_http::Request) {
    let base = &built.url.path;
    if let Some(location) = redirect(base, request.url())
        && let Ok(header) = Header::from_bytes(&b"Location"[..], location.as_bytes())
    {
        // Not 301, which a browser would remember after the path changes.
        let _ = request.respond(Response::empty(302).with_header(header));
        return;
    }
    let (full, status) = match respond_with(&built.destination, base, request.url()) {
        Some(found) => found,
        None => {
            let _ = request.respond(Response::from_string("404 Not Found").with_status_code(404));
            return;
        }
    };

    match std::fs::read(&full) {
        Ok(bytes) => {
            let content_type = content_type(&full);
            let header = Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()).unwrap();
            if content_type == "text/html" {
                let html = inject_livereload(String::from_utf8_lossy(&bytes).into_owned());
                let response = Response::from_string(html).with_header(header);
                let _ = request.respond(response.with_status_code(status));
            } else {
                let response = Response::from_data(bytes).with_header(header);
                let _ = request.respond(response.with_status_code(status));
            }
        }
        Err(_) => {
            let _ = request.respond(Response::from_string("404 Not Found").with_status_code(404));
        }
    }
}

/// Where to send the browser instead of `url`, for a site under `base`, like
/// `/my-project`: to the site's home page, from the root of the domain or
/// from `base` without its `/`.
fn redirect(base: &str, url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or("");
    (!base.is_empty() && (path == "/" || path == base)).then(|| format!("{base}/"))
}

/// The file to answer a request for `url` with, and the status to send: the
/// file at that URL under `base`, the site's path, or else the site's 404
/// page, as a static host would. `None` if there's neither.
fn respond_with(destination: &Path, base: &str, url: &str) -> Option<(PathBuf, u16)> {
    let path = url.split(['?', '#']).next().unwrap_or("");
    let within = path.strip_prefix(base).filter(|rest| rest.starts_with('/'));
    if let Some(path) = within.and_then(|rest| resolve_route(destination, rest)) {
        return Some((path, 200));
    }
    let not_found = destination.join(site::NOT_FOUND_FILE);
    not_found.is_file().then_some((not_found, 404))
}

/// Resolve a request URL to a file in the destination, understanding the
/// pretty, extensionless routes the build emits (`/getting-started` ->
/// `getting-started/index.html`). The URL is decoded first, as a browser
/// encodes what it can't send as is: the page built from
/// `Incident Response/triage.md` is asked for as `/Incident%20Response/triage`.
/// Returns `None` for anything that doesn't resolve to a real file, and
/// rejects paths that would leave the destination.
fn resolve_route(destination: &Path, url: &str) -> Option<PathBuf> {
    let path = url.split(['?', '#']).next().unwrap_or("");
    let decoded = percent_decode_str(path).decode_utf8().ok()?;
    let trimmed = decoded.trim_start_matches('/');

    // Check the decoded path, where `%2e%2e` is `..`, and check it as the
    // system reads it, so on Windows a `\` (`%5C`) can't climb out either.
    if !Path::new(trimmed)
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }

    let base = if trimmed.is_empty() {
        destination.to_path_buf()
    } else {
        destination.join(trimmed)
    };

    // Directory index first (covers "/", "/guide/", and extensionless routes),
    // then an exact file (a static file), then a sibling ".html" as a fallback.
    let candidates = [
        base.join("index.html"),
        base.clone(),
        destination.join(format!("{trimmed}.html")),
    ];
    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()) {
        Some("html") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

fn inject_livereload(html: String) -> String {
    const SCRIPT: &str = r#"<script>
(function(){let v=null;async function poll(){try{const r=await fetch('/__little/version',{cache:'no-store'});const t=await r.text();if(v===null){v=t;}else if(t!==v){location.reload();}}catch(e){}}setInterval(poll,400);})();
</script>"#;

    match html.rfind("</body>") {
        Some(idx) => {
            let mut out = String::with_capacity(html.len() + SCRIPT.len());
            out.push_str(&html[..idx]);
            out.push_str(SCRIPT);
            out.push_str(&html[idx..]);
            out
        }
        None => html + SCRIPT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::Receiver;

    /// Build a throwaway `dist/` tree mirroring the pretty-URL output layout.
    fn scratch_dist() -> PathBuf {
        let root = std::env::temp_dir().join(format!("little-server-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for file in [
            "index.html",
            "getting-started/index.html",
            "guide/index.html",
            "guide/install/index.html",
            "assets/style.css",
        ] {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"x").unwrap();
        }
        root
    }

    #[test]
    fn resolves_pretty_and_asset_routes() {
        let dist = scratch_dist();
        let at = |url: &str| resolve_route(&dist, url);

        assert_eq!(at("/"), Some(dist.join("index.html")));
        assert_eq!(
            at("/getting-started"),
            Some(dist.join("getting-started/index.html"))
        );
        assert_eq!(
            at("/getting-started/"),
            Some(dist.join("getting-started/index.html"))
        );
        assert_eq!(at("/guide/"), Some(dist.join("guide/index.html")));
        assert_eq!(
            at("/guide/install"),
            Some(dist.join("guide/install/index.html"))
        );
        assert_eq!(at("/assets/style.css"), Some(dist.join("assets/style.css")));
        assert_eq!(
            at("/getting-started?v=2"),
            Some(dist.join("getting-started/index.html"))
        );

        assert_eq!(at("/does-not-exist"), None);
        assert_eq!(at("/../secret"), None);

        let _ = std::fs::remove_dir_all(&dist);
    }

    #[test]
    fn encoded_urls_find_the_files_they_name() {
        // A browser encodes the space in `Incident Response`, and the `é` in
        // `café`, when it asks for them.
        let root = scratch(
            "encoded",
            &[
                ("dist/index.html", "Home"),
                ("dist/Incident Response/Process/triage/index.html", "Triage"),
                ("dist/café/index.html", "Café"),
                ("secret/index.html", "Secret"),
            ],
        );
        let dist = root.join("dist");
        let at = |url: &str| resolve_route(&dist, url);
        let triage = dist.join("Incident Response/Process/triage/index.html");

        assert_eq!(
            at("/Incident%20Response/Process/triage"),
            Some(triage.clone())
        );
        assert_eq!(
            at("/Incident%20Response/Process/triage/?v=2"),
            Some(triage.clone())
        );
        assert_eq!(at("/caf%C3%A9/"), Some(dist.join("café/index.html")));
        // Not UTF-8 once decoded, so it can't be one of little's URLs.
        assert_eq!(at("/caf%E9/"), None);
        // And under a site's path, as `little serve` is asked for it.
        assert_eq!(
            respond_with(
                &dist,
                "/little",
                "/little/Incident%20Response/Process/triage"
            ),
            Some((triage, 200))
        );

        // Decoded, each of these would leave the destination for `secret/`.
        for url in [
            "/%2e%2e/secret/",
            "/%2E%2E%2Fsecret/",
            "/Incident%20Response/..%2F..%2Fsecret/",
        ] {
            assert_eq!(at(url), None, "{url}");
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_urls_get_the_sites_404_page() {
        let dist = scratch("not-found", &[("index.html", "Home")]);
        let not_found = dist.join(site::NOT_FOUND_FILE);

        // Without a 404 page, there's nothing to send...
        assert_eq!(respond_with(&dist, "", "/nowhere"), None);

        // ...and with one, it's sent for any URL that has no file.
        std::fs::write(&not_found, "Not found").unwrap();
        assert_eq!(
            respond_with(&dist, "", "/nowhere"),
            Some((not_found.clone(), 404))
        );
        assert_eq!(
            respond_with(&dist, "", "/../secret"),
            Some((not_found.clone(), 404))
        );
        assert_eq!(
            respond_with(&dist, "", "/"),
            Some((dist.join("index.html"), 200))
        );
        assert_eq!(respond_with(&dist, "", "/404.html"), Some((not_found, 200)));

        let _ = std::fs::remove_dir_all(&dist);
    }

    #[test]
    fn a_site_under_a_path_is_served_under_it() {
        let dist = scratch(
            "under-a-path",
            &[
                ("index.html", "Home"),
                ("guide/index.html", "Guide"),
                ("404.html", "Not found"),
            ],
        );
        let at = |url: &str| respond_with(&dist, "/little", url);
        let found = |file: &str| Some((dist.join(file), 200));
        let not_found = Some((dist.join(site::NOT_FOUND_FILE), 404));

        assert_eq!(at("/little/"), found("index.html"));
        assert_eq!(at("/little/guide/"), found("guide/index.html"));
        assert_eq!(at("/little/guide?v=2"), found("guide/index.html"));
        assert_eq!(at("/little/404.html"), found("404.html"));

        // Outside the path, nothing is the site's, as when it's published, so
        // a link that leaves the path out is broken here too.
        for url in ["/guide/", "/404.html", "/littler/", "/../little/"] {
            assert_eq!(at(url), not_found, "{url}");
        }

        // The root of the domain, and the path without its `/`, go to the
        // site's home page.
        assert_eq!(redirect("/little", "/"), Some("/little/".into()));
        assert_eq!(redirect("/little", "/?v=2"), Some("/little/".into()));
        assert_eq!(redirect("/little", "/little"), Some("/little/".into()));
        for url in ["/little/", "/guide/", "/littler"] {
            assert_eq!(redirect("/little", url), None, "{url}");
        }
        // A site at the root of its domain is already there.
        assert_eq!(redirect("", "/"), None);

        let _ = std::fs::remove_dir_all(&dist);
    }

    #[test]
    fn ipv6_addresses_go_in_brackets() {
        assert_eq!(address("127.0.0.1", 8080), "127.0.0.1:8080");
        assert_eq!(address("localhost", 3000), "localhost:3000");
        assert_eq!(address("::1", 8080), "[::1]:8080");
    }

    #[test]
    fn only_site_config_changes_trigger_root_rebuilds() {
        use notify::EventKind;
        use notify::event::{AccessKind, CreateKind, ModifyKind, RemoveKind};

        let event = |kind, name: &str| Event::new(kind).add_path(Path::new("/site").join(name));

        // Creating, editing, or deleting little.yml rebuilds — including a
        // config that didn't exist when `little serve` started.
        for kind in [
            EventKind::Create(CreateKind::File),
            EventKind::Modify(ModifyKind::Any),
            EventKind::Remove(RemoveKind::File),
        ] {
            assert!(changes_site_config(&event(kind, "little.yml")));
        }

        // Reading it (every build does) and other root churn don't.
        assert!(!changes_site_config(&event(
            EventKind::Access(AccessKind::Any),
            "little.yml"
        )));
        assert!(!changes_site_config(&event(
            EventKind::Remove(RemoveKind::Folder),
            "dist"
        )));
        assert!(!changes_site_config(&event(
            EventKind::Create(CreateKind::File),
            ".little.yml.swp"
        )));
    }

    #[test]
    fn only_hidden_files_in_the_source_are_ignored() {
        let source = Path::new("/site/.github");
        let event = |paths: &[&str]| {
            let mut event = Event::new(notify::EventKind::Any);
            for path in paths {
                event = event.add_path(source.join(path));
            }
            event
        };

        // A build leaves out hidden files and what's in hidden folders.
        assert!(is_hidden_in(&event(&[".install.md.swp"]), source));
        assert!(is_hidden_in(&event(&[".drafts/wip.md"]), source));
        // Only names inside the source count, so it can be hidden itself.
        assert!(!is_hidden_in(&event(&["README.md"]), source));
        // A hidden file renamed over a page changes the page.
        assert!(!is_hidden_in(
            &event(&[".README.md.tmp", "README.md"]),
            source
        ));
    }

    /// A throwaway folder holding `files`, each a (path, contents) pair.
    fn scratch(label: &str, files: &[(&str, &str)]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("little-server-test-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (file, contents) in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, contents).unwrap();
        }
        root
    }

    /// A throwaway site containing every kind of file a build reads.
    fn scratch_site(label: &str) -> PathBuf {
        scratch(
            label,
            &[
                ("little.yml", "theme: ./theme\nname: Test\n"),
                ("docs/README.md", "# Home\n"),
                ("docs/guide/install.md", "# Install\n"),
                ("docs/guide/images/diagram.png", "png"),
                ("theme/little.yml", "font: serif\n"),
                ("theme/_templates/default.html", "{{ page.content | safe }}"),
                ("theme/_static/style.css", "body {}\n"),
            ],
        )
    }

    /// Watch a built site as `serve` does, and wait out any late events from
    /// creating it.
    fn watch_built_site(site_dir: &Path) -> (Sources, Receiver<notify::Result<Event>>) {
        site::build(site_dir).unwrap(); // `serve` builds once before watching
        let (tx, rx) = std::sync::mpsc::channel();
        let sources = Sources::watch(site_dir, tx).unwrap();
        while rx.recv_timeout(Duration::from_millis(300)).is_ok() {}
        (sources, rx)
    }

    /// Check that rebuilding the site doesn't wake the watcher (or each build
    /// would trigger the next), but that editing `file` does.
    fn assert_only_edits_wake(site_dir: &Path, file: &Path) {
        let (_sources, rx) = watch_built_site(site_dir);

        site::build(site_dir).unwrap();
        let event = rx.recv_timeout(Duration::from_millis(500));
        assert!(event.is_err(), "a build woke the watcher: {event:?}");

        std::fs::write(file, "body { margin: 0 }\n").unwrap();
        let event = rx.recv_timeout(Duration::from_secs(5));
        assert!(
            event.is_ok(),
            "editing {} didn't wake the watcher",
            file.display()
        );
    }

    #[test]
    fn a_build_does_not_retrigger_the_watcher() {
        let site_dir = scratch_site("watch");
        assert_only_edits_wake(&site_dir, &site_dir.join("theme/_static/style.css"));
        let _ = std::fs::remove_dir_all(&site_dir);
    }

    #[test]
    fn a_source_can_hold_the_destination() {
        let site_dir = scratch_site("inside");
        let site_cfg = "theme: ./theme\ndestination: ./docs/_build\n";
        std::fs::write(site_dir.join("little.yml"), site_cfg).unwrap();
        assert_only_edits_wake(&site_dir, &site_dir.join("docs/guide/install.md"));
        let _ = std::fs::remove_dir_all(&site_dir);
    }

    #[test]
    fn the_watcher_watches_every_theme_in_the_chain() {
        let site_dir = scratch_site("chain");
        std::fs::write(site_dir.join("little.yml"), "theme: ./company\n").unwrap();
        let company = site_dir.join("company");
        std::fs::create_dir_all(&company).unwrap();
        let company_cfg = company.join("little.yml");
        std::fs::write(&company_cfg, "theme: ../theme\nfont: mono\n").unwrap();

        // The theme it builds on, and its own config. That edit breaks the
        // config, so it goes last.
        assert_only_edits_wake(&site_dir, &site_dir.join("theme/_static/style.css"));
        assert_only_edits_wake(&site_dir, &company_cfg);
        let _ = std::fs::remove_dir_all(&site_dir);
    }

    #[test]
    fn the_watcher_skips_downloaded_themes() {
        // A company's theme that builds on a downloaded one...
        let site_dir = scratch_site("downloaded");
        let url = "https://example.invalid/fancy-1.0.zip";
        std::fs::write(site_dir.join("little.yml"), "theme: ./company\n").unwrap();
        let company = site_dir.join("company");
        std::fs::create_dir_all(company.join("_static")).unwrap();
        std::fs::write(company.join("little.yml"), format!("theme: {url}\n")).unwrap();
        let company_css = company.join("_static/company.css");
        std::fs::write(&company_css, "body {}\n").unwrap();
        // ...kept where it would be once downloaded, so the test needn't
        // download it.
        let fancy = crate::download::folder(&site_dir, url);
        std::fs::create_dir_all(fancy.parent().unwrap()).unwrap();
        std::fs::rename(site_dir.join("theme"), &fancy).unwrap();

        {
            let (_sources, rx) = watch_built_site(&site_dir);
            std::fs::write(fancy.join("_static/style.css"), "body { margin: 0 }\n").unwrap();
            let event = rx.recv_timeout(Duration::from_millis(500));
            assert!(event.is_err(), "a downloaded theme is watched: {event:?}");
        }
        assert_only_edits_wake(&site_dir, &company_css);
        let _ = std::fs::remove_dir_all(&site_dir);
    }

    #[test]
    fn a_theme_can_hold_the_site_that_uses_it() {
        // A theme's repo with a demo site inside, which uses the theme around
        // it, so the site's destination is inside the watched theme: beside
        // the site's docs, or anywhere else in the theme.
        for destination in ["./dist", "../site"] {
            let demo_cfg = format!("theme: ..\ndestination: {destination}\n");
            let theme_dir = scratch(
                "demo",
                &[
                    ("little.yml", "font: serif\n"),
                    ("_templates/default.html", "{{ page.content | safe }}"),
                    ("_static/style.css", "body {}\n"),
                    ("demo/little.yml", &demo_cfg),
                    ("demo/docs/README.md", "# Home\n"),
                ],
            );
            assert_only_edits_wake(
                &theme_dir.join("demo"),
                &theme_dir.join("_static/style.css"),
            );
            let _ = std::fs::remove_dir_all(&theme_dir);
        }
    }

    /// Check that once the site's config is `site_cfg`, the watcher ignores
    /// edits to the `old` file but wakes for edits to the `new` one.
    fn assert_follows(site_dir: &Path, site_cfg: &str, old: &str, new: &str) {
        let (mut sources, rx) = watch_built_site(site_dir);

        std::fs::write(site_dir.join("little.yml"), site_cfg).unwrap();
        sources.follow(site_dir).unwrap();
        while rx.recv_timeout(Duration::from_millis(300)).is_ok() {}

        let edit = |file: &str| std::fs::write(site_dir.join(file), "edited\n");
        edit(old).unwrap();
        let event = rx.recv_timeout(Duration::from_millis(500));
        assert!(event.is_err(), "{old} is still watched: {event:?}");

        edit(new).unwrap();
        let event = rx.recv_timeout(Duration::from_secs(5));
        assert!(event.is_ok(), "{new} isn't watched");
    }

    #[test]
    fn the_watcher_follows_the_site_to_another_theme() {
        let site_dir = scratch_site("switch");
        for (file, contents) in [
            ("other/_templates/default.html", "{{ page.content | safe }}"),
            ("other/_static/style.css", "body {}\n"),
        ] {
            let path = site_dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, contents).unwrap();
        }
        assert_follows(
            &site_dir,
            "theme: ./other\n",
            "theme/_static/style.css",
            "other/_static/style.css",
        );
        let _ = std::fs::remove_dir_all(&site_dir);
    }

    #[test]
    fn the_watcher_follows_the_site_to_another_source() {
        let site_dir = scratch_site("move");
        std::fs::create_dir_all(site_dir.join("pages")).unwrap();
        std::fs::write(site_dir.join("pages/README.md"), "# Home\n").unwrap();
        assert_follows(
            &site_dir,
            "theme: ./theme\nsource: ./pages\n",
            "docs/README.md",
            "pages/README.md",
        );
        let _ = std::fs::remove_dir_all(&site_dir);
    }

    #[test]
    fn the_watcher_skips_hidden_files_in_the_source() {
        // Like an editor's swap files or drafts, which a build leaves out.
        let site_dir = scratch_site("hidden");
        {
            let (_sources, rx) = watch_built_site(&site_dir);
            let docs = site_dir.join("docs");
            std::fs::write(docs.join("guide/.install.md.swp"), "swap\n").unwrap();
            std::fs::create_dir_all(docs.join(".drafts")).unwrap();
            std::fs::write(docs.join(".drafts/wip.md"), "# WIP\n").unwrap();
            let event = rx.recv_timeout(Duration::from_millis(500));
            assert!(event.is_err(), "a hidden file woke the watcher: {event:?}");

            std::fs::write(docs.join("guide/install.md"), "# Installing\n").unwrap();
            let event = rx.recv_timeout(Duration::from_secs(5));
            assert!(event.is_ok(), "editing a page didn't wake the watcher");
        }

        // Only names inside the source count, so it can be hidden itself.
        std::fs::create_dir_all(site_dir.join(".docs")).unwrap();
        std::fs::write(site_dir.join(".docs/README.md"), "# Home\n").unwrap();
        assert_follows(
            &site_dir,
            "theme: ./theme\nsource: ./.docs\n",
            "docs/README.md",
            ".docs/README.md",
        );
        let _ = std::fs::remove_dir_all(&site_dir);
    }
}
