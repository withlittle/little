//! A tiny dev server with live reload.
//!
//! Strategy: serve dist/ over HTTP, expose a `/__little/version` counter, and
//! inject a small polling script into every HTML page. A file watcher rebuilds
//! the site and bumps the counter, so open browsers reload themselves.

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tiny_http::{Header, Response, Server};

use crate::site;

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
        use notify::{RecursiveMode, Watcher};

        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("watcher error: {e}");
                return;
            }
        };

        for sub in ["docs", "theme"] {
            let _ = watcher.watch(&project_dir.join(sub), RecursiveMode::Recursive);
        }
        let _ = watcher.watch(&project_dir.join("little.yml"), RecursiveMode::NonRecursive);

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

fn serve_file(dist_dir: &Path, request: tiny_http::Request) {
    let raw = request.url().split('?').next().unwrap_or("/");
    let mut rel = raw.trim_start_matches('/').to_string();
    if rel.is_empty() || rel.ends_with('/') {
        rel.push_str("index.html");
    }

    let mut full = dist_dir.join(&rel);
    if full.is_dir() {
        full = full.join("index.html");
    }

    // Basic traversal guard for a local dev server.
    if !full.starts_with(dist_dir) {
        let _ = request.respond(Response::from_string("forbidden").with_status_code(403));
        return;
    }

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
