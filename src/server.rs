//! A tiny dev server with live reload.
//!
//! Strategy: serve dist/ over HTTP, expose a `/__little/version` counter, and
//! inject a small polling script into every HTML page. A file watcher rebuilds
//! the site and bumps the counter, so open browsers reload themselves.

use anyhow::{Result, anyhow};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;
use tiny_http::{Header, Response, Server};

use crate::{config, site};

pub fn serve(project_dir: &Path, addr: &str) -> Result<()> {
    let project_dir = project_dir.to_path_buf();
    let dist_dir = project_dir.join("dist");

    site::build(&project_dir)?;

    let version = Arc::new(AtomicU64::new(1));
    spawn_watcher(project_dir.clone(), version.clone());

    let server = Server::http(addr).map_err(|e| anyhow!("failed to bind {addr}: {e}"))?;
    println!("little serving http://{addr}  (live reload on — edit docs/ or theme/)");

    for request in server.incoming_requests() {
        if request.url().starts_with("/__little/version") {
            let body = version.load(Ordering::SeqCst).to_string();
            let _ = request.respond(Response::from_string(body));
            continue;
        }
        serve_file(&dist_dir, request);
    }
    Ok(())
}

fn spawn_watcher(project_dir: PathBuf, version: Arc<AtomicU64>) {
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        // Bound to a name (not `_`) so the watchers live as long as this loop.
        let _watchers = match watch_sources(&project_dir, tx) {
            Ok(watchers) => watchers,
            Err(e) => {
                eprintln!("watcher error: {e}");
                return;
            }
        };

        // Debounce: wake on the first event, then swallow the burst.
        while rx.recv().is_ok() {
            std::thread::sleep(Duration::from_millis(150));
            while rx.try_recv().is_ok() {}

            match site::build(&project_dir) {
                Ok(()) => {
                    version.fetch_add(1, Ordering::SeqCst);
                    println!("rebuilt");
                }
                Err(e) => eprintln!("build error: {e}"),
            }
        }
    });
}

/// Watch everything a build reads, sending events to `tx` as it changes.
fn watch_sources(
    project_dir: &Path,
    tx: Sender<notify::Result<Event>>,
) -> notify::Result<[RecommendedWatcher; 2]> {
    // docs/ and theme/ — the latter includes the theme's optional config.
    let content_tx = tx.clone();
    let mut content = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if !res.as_ref().is_ok_and(is_read) {
            let _ = content_tx.send(res);
        }
    })?;
    for sub in ["docs", "theme"] {
        let _ = content.watch(&project_dir.join(sub), RecursiveMode::Recursive);
    }

    // The site's config is optional and may not exist yet, so watch the project
    // root rather than the file, forwarding only changes to the config (the
    // root also holds dist/, which every build recreates).
    let mut root = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if res.as_ref().is_ok_and(changes_site_config) {
            let _ = tx.send(res);
        }
    })?;
    let _ = root.watch(project_dir, RecursiveMode::NonRecursive);

    Ok([content, root])
}

/// Is this event just a read? Every build opens the files it uses, and some
/// platforms report opens (Linux's inotify does), so a watcher that counted
/// reads as changes would trigger endless rebuilds.
fn is_read(event: &Event) -> bool {
    event.kind.is_access()
}

/// Does this project-root event change the site's config file?
fn changes_site_config(event: &Event) -> bool {
    !is_read(event)
        && event
            .paths
            .iter()
            .any(|path| path.file_name() == Some(OsStr::new(config::CONFIG_FILE)))
}

fn serve_file(dist_dir: &Path, request: tiny_http::Request) {
    let full = match resolve_route(dist_dir, request.url()) {
        Some(path) => path,
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
                let _ = request.respond(Response::from_string(html).with_header(header));
            } else {
                let _ = request.respond(Response::from_data(bytes).with_header(header));
            }
        }
        Err(_) => {
            let _ = request.respond(Response::from_string("404 Not Found").with_status_code(404));
        }
    }
}

/// Resolve a request URL to a file under `dist/`, understanding the pretty,
/// extensionless routes the build emits (`/getting-started` ->
/// `getting-started/index.html`). Returns `None` for anything that doesn't
/// resolve to a real file, and rejects `..` traversal.
fn resolve_route(dist_dir: &Path, url: &str) -> Option<PathBuf> {
    let path = url.split(['?', '#']).next().unwrap_or("");
    let trimmed = path.trim_start_matches('/');

    if trimmed.split('/').any(|segment| segment == "..") {
        return None;
    }

    let base = if trimmed.is_empty() {
        dist_dir.to_path_buf()
    } else {
        dist_dir.join(trimmed)
    };

    // Directory index first (covers "/", "/guide/", and extensionless routes),
    // then an exact file (assets), then a sibling ".html" as a fallback.
    let candidates = [
        base.join("index.html"),
        base.clone(),
        dist_dir.join(format!("{trimmed}.html")),
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

    /// A throwaway project containing every kind of file a build reads.
    fn scratch_project(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("little-server-test-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (file, contents) in [
            ("little.yml", "name: Test\n"),
            ("docs/README.md", "# Home\n"),
            ("docs/guide/install.md", "# Install\n"),
            ("theme/little.yml", "accent: \"#7c3aed\"\n"),
            ("theme/templates/page.html", "{{ page.content | safe }}"),
            ("theme/assets/style.css", "body {}\n"),
        ] {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, contents).unwrap();
        }
        root
    }

    #[test]
    fn a_build_does_not_retrigger_the_watcher() {
        let project = scratch_project("watch");
        site::build(&project).unwrap(); // `serve` builds once before watching

        let (tx, rx) = std::sync::mpsc::channel();
        let _watchers = watch_sources(&project, tx).unwrap();
        // Let any late events from creating the project arrive first.
        while rx.recv_timeout(Duration::from_millis(300)).is_ok() {}

        // A rebuild must not wake the watcher, or each one would trigger the next.
        site::build(&project).unwrap();
        let event = rx.recv_timeout(Duration::from_millis(500));
        assert!(event.is_err(), "a build woke the watcher: {event:?}");

        std::fs::write(
            project.join("theme/assets/style.css"),
            "body { margin: 0 }\n",
        )
        .unwrap();
        let event = rx.recv_timeout(Duration::from_secs(5));
        assert!(event.is_ok(), "editing an asset didn't wake the watcher");

        let _ = std::fs::remove_dir_all(&project);
    }
}
