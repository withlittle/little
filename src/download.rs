//! Themes downloaded from a URL. The first build that needs one downloads its
//! archive over HTTPS and unpacks it into the site's `.little/` folder; every
//! build after that uses it from there, offline too.
//!
//! ```text
//! my-docs/
//! └── .little/          made by the first download
//!     ├── .gitignore    `*`, so git ignores the folder
//!     └── themes/
//!         └── 3f9c…e1/  a theme, in a folder named for its URL
//! ```
//!
//! A theme's archive is a .zip or a .tar.gz that holds one folder, as GitHub's
//! archives of a tag or a branch do. little never downloads a URL again: to
//! pick up changes to a branch's archive, delete `.little/`.

use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

/// The folder in a site that little keeps its downloads in.
const CACHE_DIR: &str = ".little";
/// What `.little/.gitignore` holds: it ignores everything, itself included.
const GITIGNORE: &str = "# little's downloads. Delete this folder to download them again.\n*\n";

/// The folder `url`'s theme is kept in, in the site at `site_dir`.
pub fn folder(site_dir: &Path, url: &str) -> PathBuf {
    site_dir
        .join(CACHE_DIR)
        .join("themes")
        .join(cache_name(url))
}

/// The folder of `url`'s theme, downloaded first unless it's already there.
/// An error finishes the sentence "…, but": "little couldn't download it: the
/// server answered 404 Not Found".
pub fn theme(site_dir: &Path, url: &str) -> Result<PathBuf> {
    theme_with(site_dir, url, fetch)
}

/// `theme`, downloading with `fetch`, so that tests needn't use the network.
fn theme_with(
    site_dir: &Path,
    url: &str,
    fetch: impl FnOnce(&str) -> Result<Vec<u8>>,
) -> Result<PathBuf> {
    let dir = folder(site_dir, url);
    if dir.is_dir() {
        return Ok(dir);
    }
    eprintln!("downloading theme {url}");
    let archive = fetch(url).context("little couldn't download it")?;

    // Unpacked beside its folder, then moved in, in one step, so a download
    // that fails, or a build that's stopped, never leaves half a theme to use
    // next time. The process ID keeps apart two builds that download the same
    // theme at once.
    let unpacking = dir.with_extension(format!("{}.download", std::process::id()));
    let saved = save(&archive, site_dir, &unpacking, &dir);
    let _ = fs::remove_dir_all(&unpacking);
    saved.context("little couldn't unpack it")?;
    Ok(dir)
}

/// Unpack `archive` into `unpacking`, then move the folder it holds to `dir`.
fn save(archive: &[u8], site_dir: &Path, unpacking: &Path, dir: &Path) -> Result<()> {
    // Told apart by how they start, as a URL needn't end in .zip or .tar.gz.
    let is_zip = archive.starts_with(b"PK");
    if !is_zip && !archive.starts_with(&[0x1f, 0x8b]) {
        bail!(
            "it isn't a .zip or .tar.gz archive (on GitHub, a tag's archive is at \
             https://github.com/OWNER/REPO/archive/refs/tags/TAG.tar.gz)"
        );
    }
    make_cache(site_dir)?;
    let _ = fs::remove_dir_all(unpacking); // left by a build that was stopped
    create_dir(unpacking)?;
    if is_zip {
        unpack_zip(archive, unpacking)?;
    } else {
        unpack_tar_gz(archive, unpacking)?;
    }
    let top = single_folder(unpacking)?;
    match fs::rename(&top, dir) {
        Ok(()) => Ok(()),
        // Another build downloaded it first.
        Err(_) if dir.is_dir() => Ok(()),
        Err(e) => Err(e).with_context(|| format!("moving {} to {}", top.display(), dir.display())),
    }
}

/// Make the site's `.little/` folder, unless it's there, with a `.gitignore`
/// that keeps it out of git.
fn make_cache(site_dir: &Path) -> Result<()> {
    let cache = site_dir.join(CACHE_DIR);
    create_dir(&cache.join("themes"))?;
    let gitignore = cache.join(".gitignore");
    if !gitignore.exists() {
        fs::write(&gitignore, GITIGNORE)
            .with_context(|| format!("writing {}", gitignore.display()))?;
    }
    Ok(())
}

// Only files and folders are unpacked, and only inside the folder the archive
// is unpacked into. A link could point anywhere, like `~/.ssh`, and a build
// would copy what it points at into the site.

/// Unpack the .zip `archive` into `dest`.
fn unpack_zip(archive: &[u8], dest: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_string();
        // `None` for a path that climbs out of the archive with `..`.
        let Some(path) = entry.enclosed_name() else {
            bail!("`{name}` leads outside the archive");
        };
        if entry.is_symlink() {
            bail!("`{name}` is a link, and little only unpacks files and folders");
        }
        if entry.is_dir() {
            create_dir(&dest.join(path))?;
        } else {
            write_file(&dest.join(path), &mut entry)?;
        }
    }
    Ok(())
}

/// Unpack the .tar.gz `archive` into `dest`.
fn unpack_tar_gz(archive: &[u8], dest: &Path) -> Result<()> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries()? {
        let mut entry = entry?;
        let kind = entry.header().entry_type();
        // GitHub's archives start with one, naming the commit they're of.
        if kind.is_pax_global_extensions() {
            continue;
        }
        let path = entry.path()?.into_owned();
        let name = path.display();
        let inside = path
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
        if !inside {
            bail!("`{name}` leads outside the archive");
        }
        if kind.is_symlink() || kind.is_hard_link() {
            bail!("`{name}` is a link, and little only unpacks files and folders");
        } else if kind.is_dir() {
            create_dir(&dest.join(&path))?;
        } else if kind.is_file() {
            write_file(&dest.join(&path), &mut entry)?;
        } else {
            bail!("`{name}` isn't a file or a folder");
        }
    }
    Ok(())
}

/// The one folder in `dir`: GitHub's archives keep a theme in a folder named
/// for its repository and tag, like `theme-bootstrap-0.0.1/`.
fn single_folder(dir: &Path) -> Result<PathBuf> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        paths.push(entry?.path());
    }
    match paths.as_slice() {
        [only] if only.is_dir() => Ok(only.clone()),
        [] => bail!("it's empty"),
        _ => {
            let mut names: Vec<String> = paths
                .iter()
                .map(|path| {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    let slash = if path.is_dir() { "/" } else { "" };
                    format!("`{name}{slash}`")
                })
                .collect();
            names.sort();
            bail!("it should hold a single folder, not {}", names.join(", "))
        }
    }
}

fn create_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("creating {}", path.display()))
}

/// Write everything `contents` reads to a new file at `path`.
fn write_file(path: &Path, contents: &mut impl Read) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir(parent)?;
    }
    let mut file =
        fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    std::io::copy(contents, &mut file).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Download `url`, following redirects (GitHub serves archives from another
/// host), but only over HTTPS. As with other tools, a proxy named by
/// `HTTPS_PROXY`, `ALL_PROXY`, or `HTTP_PROXY` is used, except for the hosts
/// in `NO_PROXY`.
fn fetch(url: &str) -> Result<Vec<u8>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .build()
        .into();
    let mut response = agent.get(url).call().map_err(|e| match e {
        ureq::Error::StatusCode(code) => {
            let status = ureq::http::StatusCode::from_u16(code)
                .map_or_else(|_| code.to_string(), |status| status.to_string());
            anyhow!("the server answered {status}")
        }
        ureq::Error::Io(e) => e.into(),
        e => e.into(),
    })?;
    let mut archive = Vec::new();
    response.body_mut().as_reader().read_to_end(&mut archive)?;
    Ok(archive)
}

/// The name of the folder `url`'s theme is kept in: a hash of the URL, which
/// has characters a folder's name can't. It's 64-bit FNV-1a, written out here
/// because Rust's own hash can change with the version of Rust, and then every
/// theme would be downloaded again.
fn cache_name(url: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in url.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io::Write;
    use tar::EntryType;
    use walkdir::WalkDir;

    const URL: &str = "https://example.invalid/theme-1.0.tar.gz";

    /// A theme's files, text and not.
    const FILES: &[(&str, &[u8])] = &[
        ("little.yml", b"font: Inter\n"),
        ("_templates/default.html", b"{{ page.content }}"),
        (
            "_static/img/logo.png",
            &[0x89, b'P', b'N', b'G', 0x00, 0xff],
        ),
    ];

    /// A fresh scratch site.
    fn scratch(label: &str) -> PathBuf {
        let site = std::env::temp_dir().join(format!(
            "little-download-test-{}-{label}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&site);
        fs::create_dir_all(&site).unwrap();
        site
    }

    /// A .tar.gz of `entries`, each a type, a path, and contents, written as
    /// they are, even ones `tar` won't write. A link's contents are what it
    /// points at.
    fn tar_gz(entries: &[(EntryType, &str, &[u8])]) -> Vec<u8> {
        let mut tar = tar::Builder::new(Vec::new());
        for &(kind, path, contents) in entries {
            let mut header = tar::Header::new_gnu();
            header.as_old_mut().name[..path.len()].copy_from_slice(path.as_bytes());
            header.set_entry_type(kind);
            header.set_mode(0o644);
            let data = if kind.is_symlink() || kind.is_hard_link() {
                header.set_link_name_literal(contents).unwrap();
                &[][..]
            } else {
                contents
            };
            header.set_size(data.len() as u64);
            header.set_cksum();
            tar.append(&header, data).unwrap();
        }
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(&tar.into_inner().unwrap()).unwrap();
        gz.finish().unwrap()
    }

    /// A .zip of `entries`, each a path and contents. A path that ends in `/`
    /// is a folder.
    fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        for &(path, contents) in entries {
            if path.ends_with('/') {
                zip.add_directory(path, options).unwrap();
            } else {
                zip.start_file(path, options).unwrap();
                zip.write_all(contents).unwrap();
            }
        }
        zip.finish().unwrap().into_inner()
    }

    /// `FILES` in a folder, archived as GitHub archives a tag: a .tar.gz,
    /// which starts with a header naming the commit, and a .zip.
    fn github_archives() -> [Vec<u8>; 2] {
        let paths: Vec<String> = FILES
            .iter()
            .map(|(path, _)| format!("theme-1.0/{path}"))
            .collect();
        let files = paths
            .iter()
            .zip(FILES)
            .map(|(path, (_, contents))| (path.as_str(), *contents));
        let commit: &[u8] = b"52 comment=83bb4fdb7a01c9d20af28fc07364c45f38d3efba\n";
        let mut tar = vec![
            (EntryType::XGlobalHeader, "pax_global_header", commit),
            (EntryType::Directory, "theme-1.0/", &[][..]),
        ];
        tar.extend(
            files
                .clone()
                .map(|(path, contents)| (EntryType::Regular, path, contents)),
        );
        let mut zip_entries = vec![("theme-1.0/", &[][..])];
        zip_entries.extend(files);
        [tar_gz(&tar), zip(&zip_entries)]
    }

    /// Every file in `dir`, by its path from `dir`, and what it holds.
    fn files_in(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut files: Vec<_> = WalkDir::new(dir)
            .into_iter()
            .map(Result::unwrap)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| {
                let path = entry.path().strip_prefix(dir).unwrap();
                let path = path.to_string_lossy().replace('\\', "/");
                (path, fs::read(entry.path()).unwrap())
            })
            .collect();
        files.sort();
        files
    }

    /// Why `archive` can't be unpacked as a theme.
    fn error(site: &Path, archive: Vec<u8>) -> String {
        format!("{:#}", theme_with(site, URL, |_| Ok(archive)).unwrap_err())
    }

    #[test]
    fn a_theme_is_unpacked_from_the_folder_its_archive_holds() {
        let site = scratch("unpack");
        let mut expected: Vec<(String, Vec<u8>)> = FILES
            .iter()
            .map(|(path, contents)| (path.to_string(), contents.to_vec()))
            .collect();
        expected.sort();

        let urls = [
            "https://example.invalid/theme-1.0.tar.gz",
            "https://example.invalid/theme-1.0.zip",
        ];
        for (url, archive) in urls.into_iter().zip(github_archives()) {
            let dir = theme_with(&site, url, |_| Ok(archive)).unwrap();
            assert_eq!(dir, folder(&site, url));
            // Byte for byte, without the folder it was in or GitHub's header.
            assert_eq!(files_in(&dir), expected, "from {url}");
        }

        // Kept out of git, with nothing left over from unpacking.
        let cache = site.join(CACHE_DIR);
        let gitignore = fs::read_to_string(cache.join(".gitignore")).unwrap();
        assert_eq!(gitignore, GITIGNORE);
        let mut kept: Vec<PathBuf> = fs::read_dir(cache.join("themes"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        kept.sort();
        let mut folders = urls.map(|url| folder(&site, url));
        folders.sort();
        assert_eq!(kept, folders);

        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_theme_is_downloaded_once() {
        let site = scratch("once");
        let [archive, _] = github_archives();
        let dir = theme_with(&site, URL, |_| Ok(archive.clone())).unwrap();

        // Later builds use the theme that was downloaded, even offline...
        let again = theme_with(&site, URL, |_| panic!("it downloaded the theme again"));
        assert_eq!(again.unwrap(), dir);

        // ...until `.little/` is deleted.
        fs::remove_dir_all(site.join(CACHE_DIR)).unwrap();
        let downloaded = Cell::new(false);
        let again = theme_with(&site, URL, |_| {
            downloaded.set(true);
            Ok(archive)
        });
        assert_eq!(again.unwrap(), dir);
        assert!(downloaded.get());

        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_theme_that_fails_to_download_or_unpack_leaves_nothing() {
        let site = scratch("fails");
        let failed = theme_with(&site, URL, |_| bail!("the server answered 404 Not Found"));
        let message = format!("{:#}", failed.unwrap_err());
        let expected = "little couldn't download it: the server answered 404 Not Found";
        assert_eq!(message, expected);
        assert!(!site.join(CACHE_DIR).exists());

        // Half an archive: none of it is kept, not even the half.
        let [mut archive, _] = github_archives();
        archive.truncate(archive.len() / 2);
        let message = error(&site, archive);
        assert!(
            message.starts_with("little couldn't unpack it: "),
            "{message}"
        );
        let themes = site.join(CACHE_DIR).join("themes");
        assert_eq!(fs::read_dir(themes).unwrap().count(), 0);

        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn an_archive_holds_one_folder() {
        let site = scratch("shape");

        // A web page, say, from the URL of a repository, not of its archive.
        let message = error(&site, b"\n<!DOCTYPE html>\n<html>".to_vec());
        let expected = "little couldn't unpack it: it isn't a .zip or .tar.gz archive (on \
             GitHub, a tag's archive is at \
             https://github.com/OWNER/REPO/archive/refs/tags/TAG.tar.gz)";
        assert_eq!(message, expected);
        assert!(!site.join(CACHE_DIR).exists());

        // Files and folders side by side, or nothing at all.
        let loose = zip(&[("little.yml", &b"font: Inter\n"[..]), ("_templates/", &[])]);
        let expected = "little couldn't unpack it: it should hold a single folder, not `_templates/`, `little.yml`";
        assert_eq!(error(&site, loose), expected);
        let expected = "little couldn't unpack it: it's empty";
        assert_eq!(error(&site, zip(&[])), expected);
        assert!(!folder(&site, URL).exists());

        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn only_files_and_folders_inside_the_archive_are_unpacked() {
        let site = scratch("unsafe");
        // From where it's unpacked, `.little/themes/<name>.<pid>.download/`,
        // this is the site's own folder.
        let outside = "theme/../../../../outside";
        let link = "theme/_static/key";
        let key = "/Users/you/.ssh/id_rsa";
        let is_a_link = format!("`{link}` is a link, and little only unpacks files and folders");

        for (entry, expected) in [
            (
                (EntryType::Regular, outside, &b"x"[..]),
                format!("`{outside}` leads outside the archive"),
            ),
            (
                (EntryType::Regular, "/outside", &b"x"[..]),
                "`/outside` leads outside the archive".to_string(),
            ),
            (
                (EntryType::Symlink, link, key.as_bytes()),
                is_a_link.clone(),
            ),
            ((EntryType::Link, link, key.as_bytes()), is_a_link.clone()),
            (
                (EntryType::Fifo, "theme/pipe", &[][..]),
                "`theme/pipe` isn't a file or a folder".to_string(),
            ),
        ] {
            let archive = tar_gz(&[(EntryType::Directory, "theme/", &[]), entry]);
            let expected = format!("little couldn't unpack it: {expected}");
            assert_eq!(error(&site, archive), expected);
        }

        let archive = zip(&[("theme/", &[]), (outside, &b"x"[..])]);
        let expected = format!("little couldn't unpack it: `{outside}` leads outside the archive");
        assert_eq!(error(&site, archive), expected);
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        archive.add_symlink(link, key, options).unwrap();
        let archive = archive.finish().unwrap().into_inner();
        let expected = format!("little couldn't unpack it: {is_a_link}");
        assert_eq!(error(&site, archive), expected);

        assert!(!site.join("outside").exists());
        assert!(!folder(&site, URL).exists());
        let _ = fs::remove_dir_all(&site);
    }

    #[test]
    fn a_urls_folder_is_named_by_a_hash_that_never_changes() {
        // FNV-1a's own test values: if the names changed, every site would
        // download its themes again.
        assert_eq!(cache_name(""), "cbf29ce484222325");
        assert_eq!(cache_name("a"), "af63dc4c8601ec8c");
        assert_eq!(cache_name("foobar"), "85944171f73967e8");

        let url = |tag: &str| {
            format!("https://github.com/withlittle/theme-bootstrap/archive/refs/tags/{tag}.tar.gz")
        };
        assert_ne!(cache_name(&url("0.0.1")), cache_name(&url("0.0.2")));
    }
}
