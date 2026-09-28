//! A tiny dev server with live reload.
//!
//! Strategy: serve the site's destination over HTTP, expose a
//! `/__little/version` counter, and inject a small polling script into every
//! HTML page. A file watcher rebuilds the site and bumps the counter, so open
//! browsers reload themselves.

use anyhow::{Result, anyhow};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::ffi::OsStr;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tiny_http::{Header, Response, Server};

use crate::theme::Location;
use crate::{config, site};

pub fn serve(site_dir: &Path, addr: &str) -> Result<()> {
    let site_dir = site_dir.to_path_buf();

    // The folder of the last build that worked, which a rebuild can move.
    let destination = Arc::new(Mutex::new(site::build(&site_dir)?));

    let version = Arc::new(AtomicU64::new(1));
    spawn_watcher(site_dir.clone(), destination.clone(), version.clone());

    let server = Server::http(addr).map_err(|e| anyhow!("failed to bind {addr}: {e}"))?;
    println!("little serving http://{addr}  (live reload on — edit your docs or the theme)");

    for request in server.incoming_requests() {
        if request.url().starts_with("/__little/version") {
            let body = version.load(Ordering::SeqCst).to_string();
            let _ = request.respond(Response::from_string(body));
            continue;
        }
        let destination = destination.lock().unwrap().clone();
        serve_file(&destination, request);
    }
    Ok(())
}

fn spawn_watcher(site_dir: PathBuf, destination: Arc<Mutex<PathBuf>>, version: Arc<AtomicU64>) {
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
                    *destination.lock().unwrap() = built;
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
            let watcher = watch_folder(&source.0, &source.1, self.tx.clone())?;
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
                let watcher = watch_folder(&theme.0, &theme.1, self.tx.clone())?;
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
/// ignored, or each build would trigger the next.
fn watch_folder(
    dir: &Path,
    destination: &Path,
    tx: Sender<notify::Result<Event>>,
) -> notify::Result<RecommendedWatcher> {
    // Real paths on both sides: some platforms (macOS) report events by them.
    let destination = real_path(destination)?;
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if !res
            .as_ref()
            .is_ok_and(|event| is_read(event) || is_within(event, &destination))
        {
            let _ = tx.send(res);
        }
    })?;
    watcher.watch(&dir.canonicalize()?, RecursiveMode::Recursive)?;
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

/// Does this root-folder event change the site's config file?
fn changes_site_config(event: &Event) -> bool {
    !is_read(event)
        && event
            .paths
            .iter()
            .any(|path| path.file_name() == Some(OsStr::new(config::CONFIG_FILE)))
}

fn serve_file(destination: &Path, request: tiny_http::Request) {
    let (full, status) = match respond_with(destination, request.url()) {
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

/// The file to answer a request for `url` with, and the status to send: the
/// file at that URL, or else the site's 404 page, as a static host would.
/// `None` if there's neither.
fn respond_with(destination: &Path, url: &str) -> Option<(PathBuf, u16)> {
    if let Some(path) = resolve_route(destination, url) {
        return Some((path, 200));
    }
    let not_found = destination.join(site::NOT_FOUND_FILE);
    not_found.is_file().then_some((not_found, 404))
}

/// Resolve a request URL to a file in the destination, understanding the
/// pretty, extensionless routes the build emits (`/getting-started` ->
/// `getting-started/index.html`). Returns `None` for anything that doesn't
/// resolve to a real file, and rejects `..` traversal.
fn resolve_route(destination: &Path, url: &str) -> Option<PathBuf> {
    let path = url.split(['?', '#']).next().unwrap_or("");
    let trimmed = path.trim_start_matches('/');

    if trimmed.split('/').any(|segment| segment == "..") {
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
    fn unknown_urls_get_the_sites_404_page() {
        let dist = scratch("not-found", &[("index.html", "Home")]);
        let not_found = dist.join(site::NOT_FOUND_FILE);

        // Without a 404 page, there's nothing to send...
        assert_eq!(respond_with(&dist, "/nowhere"), None);

        // ...and with one, it's sent for any URL that has no file.
        std::fs::write(&not_found, "Not found").unwrap();
        assert_eq!(
            respond_with(&dist, "/nowhere"),
            Some((not_found.clone(), 404))
        );
        assert_eq!(
            respond_with(&dist, "/../secret"),
            Some((not_found.clone(), 404))
        );
        assert_eq!(
            respond_with(&dist, "/"),
            Some((dist.join("index.html"), 200))
        );
        assert_eq!(respond_with(&dist, "/404.html"), Some((not_found, 200)));

        let _ = std::fs::remove_dir_all(&dist);
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
}
