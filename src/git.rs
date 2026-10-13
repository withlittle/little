//! The files of a version of a site from a git ref (see `config::Version`),
//! checked out of the site's repository with the `git` command, into a
//! folder of their own, apart from the site's files as they are.
//!
//! Each file is checked out as `git checkout` would check it out, with the
//! `.gitattributes` of its commit, and their filters, like Git LFS's, not as
//! `git archive` would, which leaves out the files marked `export-ignore`,
//! as many projects mark their docs. Only the files a build of a version
//! reads are checked out: the site's `little.yml`, its source, and the
//! folders its plugins read.

use anyhow::{Context, Result, anyhow, bail};
use std::ffi::OsString;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// The git repository a site is in.
#[derive(Debug)]
pub struct Repository {
    /// Its top folder.
    top: PathBuf,
    /// The site's folder, from the top, like `docs/`, or empty for the top.
    prefix: String,
}

impl Repository {
    /// The repository the site at `site_dir` is in.
    pub fn find(site_dir: &Path) -> Result<Repository> {
        let output = run(git(site_dir).args(["rev-parse", "--show-toplevel", "--show-prefix"]))
            .with_context(|| {
                // In full, as the site's folder is `.` by default, and its
                // `source` can be in a repository when it isn't.
                let folder = std::path::absolute(site_dir)
                    .map_or_else(|_| site_dir.to_path_buf(), |dir| normalize(&dir));
                format!(
                    "the site's `versions` are built from git refs, so its folder, {}, must be in \
                     a git repository",
                    folder.display()
                )
            })?;
        let output = String::from_utf8_lossy(&output);
        let mut lines = output.lines();
        let top = lines.next().unwrap_or_default();
        let prefix = lines.next().unwrap_or_default();
        Ok(Repository {
            top: PathBuf::from(top),
            prefix: prefix.to_string(),
        })
    }

    /// Ready the files of `git_ref`, a tag, branch, or commit, to be checked
    /// out, as `Checkout::extract` asks for them.
    pub fn checkout(&self, git_ref: &str) -> Result<Checkout> {
        let commit = self.commit(git_ref)?;
        let mut hasher = DefaultHasher::new();
        (&self.top, git_ref).hash(&mut hasher);
        let name = format!(
            "little-version-{}-{:x}",
            std::process::id(),
            hasher.finish()
        );
        // git checks the files out from the top of the repository, so the
        // folder they go into can't be relative.
        let dir = std::path::absolute(std::env::temp_dir().join(name))
            .context("finding the temporary folder")?;
        // One a build that stopped before it was done left, or the last
        // build, from the same ref, which may have moved since.
        let _ = std::fs::remove_dir_all(&dir);
        let tree = dir.join("tree");
        let site_dir = (self.prefix.split('/'))
            .filter(|name| !name.is_empty())
            .fold(tree.clone(), |dir, name| dir.join(name));
        let checkout = Checkout {
            top: self.top.clone(),
            dir,
            tree,
            site_dir,
            git_ref: git_ref.to_string(),
            commit,
        };
        // The site's folder, even if it has nothing at the ref, so a path
        // from it, like `../lib`, leads where it would.
        std::fs::create_dir_all(&checkout.site_dir)
            .with_context(|| format!("creating {}", checkout.site_dir.display()))?;
        run(checkout
            .git()
            .args(["read-tree", "--no-sparse-checkout", &checkout.commit]))?;
        Ok(checkout)
    }

    /// The full name of the commit `git_ref` is.
    fn commit(&self, git_ref: &str) -> Result<String> {
        let spec = format!("{git_ref}^{{commit}}");
        if let Ok(commit) = run(git(&self.top).args(["rev-parse", "--verify", "--quiet", &spec])) {
            return Ok(String::from_utf8_lossy(&commit).trim().to_string());
        }
        // A CI service, like GitHub Actions, often clones a repository with
        // only its last commit, and none of its tags.
        let shallow = run(git(&self.top).args(["rev-parse", "--is-shallow-repository"]))
            .is_ok_and(|output| String::from_utf8_lossy(&output).trim() == "true");
        if shallow {
            bail!(
                "there's no tag, branch, or commit `{git_ref}` in {}, a shallow clone, with only \
                 some of the repository's history: clone all of it, as `actions/checkout` does \
                 with `fetch-depth: 0`",
                self.top.display()
            );
        }
        bail!(
            "there's no tag, branch, or commit `{git_ref}` in {}",
            self.top.display()
        )
    }
}

/// The files of a commit, checked out as they're needed, into a folder of
/// their own, which is removed once it's dropped.
#[derive(Debug)]
pub struct Checkout {
    /// The top folder of the repository.
    top: PathBuf,
    /// The folder of the checkout: git's `index` of the commit's files, and
    /// the `tree` they're checked out into.
    dir: PathBuf,
    tree: PathBuf,
    /// The site's folder, in the tree.
    site_dir: PathBuf,
    /// The tag, branch, or commit the files are from, as it's written.
    pub git_ref: String,
    /// The full name of their commit.
    pub commit: String,
}

impl Checkout {
    /// The site's folder, as it was at the commit, holding only the files
    /// checked out of it so far.
    pub fn site_dir(&self) -> &Path {
        &self.site_dir
    }

    /// The path of `path`, in the checkout, from the top of the repository,
    /// with each `/`, like `docs/guide.md`, or `None` for one outside it.
    pub fn in_repo(&self, path: &Path) -> Option<String> {
        let path = normalize(path);
        let path = path.strip_prefix(&self.tree).ok()?;
        let names: Vec<_> = path.iter().map(|name| name.to_string_lossy()).collect();
        Some(names.join("/"))
    }

    /// `path`, in the checkout, named as git names it, like
    /// `v1.4:docs/guide.md`.
    pub fn describe(&self, path: &Path) -> String {
        match self.in_repo(path) {
            Some(path) => format!("{}:{path}", self.git_ref),
            None => path.display().to_string(),
        }
    }

    /// Check out each file of the commit at one of `paths`, or in a folder
    /// at one, but those outside the repository.
    pub fn extract(&self, paths: &[&Path]) -> Result<()> {
        let paths: Vec<String> = paths.iter().filter_map(|path| self.in_repo(path)).collect();
        if paths.is_empty() {
            return Ok(());
        }
        // Each path as it's written, not as a pattern, like `*.md`.
        let mut ls_files = self.git();
        ls_files.args(["--literal-pathspecs", "ls-files", "-z"]);
        // The top of the repository is every file.
        if !paths.iter().any(String::is_empty) {
            ls_files.arg("--").args(&paths);
        }
        let files = run(&mut ls_files)?;
        if files.is_empty() {
            return Ok(());
        }
        let list = self.dir.join("files");
        std::fs::write(&list, files).with_context(|| format!("writing {}", list.display()))?;
        let list =
            std::fs::File::open(&list).with_context(|| format!("reading {}", list.display()))?;
        let mut prefix = OsString::from("--prefix=");
        prefix.push(&self.tree);
        prefix.push("/");
        run(self
            .git()
            .args(["checkout-index", "--force", "-z", "--stdin"])
            .arg(prefix)
            .stdin(list))?;
        Ok(())
    }

    /// A git command, run at the top of the repository, with the commit's
    /// index, not the repository's.
    fn git(&self) -> Command {
        let mut command = git(&self.top);
        command.env("GIT_INDEX_FILE", self.dir.join("index"));
        command
    }
}

impl Drop for Checkout {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A git command, run in `dir`.
fn git(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    command
}

/// Run `command`, a git command, and give what it writes.
fn run(command: &mut Command) -> Result<Vec<u8>> {
    // The git command, like `ls-files`: the first name after `-C <dir>` that
    // isn't an option.
    let name = (command.get_args().skip(2))
        .map(|arg| arg.to_string_lossy().into_owned())
        .find(|arg| !arg.starts_with('-'))
        .unwrap_or_default();
    let output = command.output().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => anyhow!("git isn't installed, or isn't on the PATH"),
        _ => anyhow!(e).context(format!("running git {name}")),
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        match stderr.trim() {
            "" => bail!("git {name} failed, with {}", output.status),
            stderr => bail!("git {name} failed: {stderr}"),
        }
    }
    Ok(output.stdout)
}

/// `path`, without each `.` in it, and with each `..` taking away the name
/// before it, as the file system would if none were a link.
fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normal.pop();
            }
            component => normal.push(component),
        }
    }
    normal
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Run git in `dir`, for a test, as no one in particular, whatever the
    /// git config of whoever runs it, and give what it writes, trimmed.
    pub(crate) fn git_in(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=little",
                "-c",
                "user.email=little@example.com",
            ])
            .args(["-c", "commit.gpgSign=false", "-c", "tag.gpgSign=false"])
            .args(["-c", "init.defaultBranch=main", "-c", "core.autocrlf=false"])
            .args(args)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "git {args:?}: {stderr}");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Commit every file in the repository at `dir`, as it is.
    pub(crate) fn commit_all(dir: &Path) {
        git_in(dir, &["add", "--all"]);
        let commit = [
            "commit",
            "--quiet",
            "--no-verify",
            "--allow-empty",
            "-m",
            "Change",
        ];
        git_in(dir, &commit);
    }

    /// A fresh git repository, with `files`, each a (path, contents) pair,
    /// committed.
    fn repository(label: &str, files: &[(&str, &str)]) -> PathBuf {
        let root = scratch(label, files);
        git_in(&root, &["init", "--quiet"]);
        commit_all(&root);
        root
    }

    /// A fresh scratch folder holding `files`, each a (path, contents) pair.
    fn scratch(label: &str, files: &[(&str, &str)]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("little-git-test-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        write(&root, files);
        // The folder git names, which, on macOS, isn't the one the temporary
        // folder's link leads to.
        root.canonicalize().unwrap()
    }

    fn write(root: &Path, files: &[(&str, &str)]) {
        for (file, contents) in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
    }

    /// The files under `dir`, from it, with each `/`.
    fn files_in(dir: &Path) -> Vec<String> {
        let mut files: Vec<String> = walkdir::WalkDir::new(dir)
            .into_iter()
            .map(Result::unwrap)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| {
                let path = entry.path().strip_prefix(dir).unwrap();
                let names: Vec<_> = path.iter().map(|name| name.to_string_lossy()).collect();
                names.join("/")
            })
            .collect();
        files.sort();
        files
    }

    #[test]
    fn finds_where_a_site_is_in_its_repository() {
        let root = repository("find", &[("docs/little.yml", ""), ("README.md", "")]);
        let repo = Repository::find(&root.join("docs")).unwrap();
        assert_eq!(
            (repo.top.as_path(), repo.prefix.as_str()),
            (&*root, "docs/")
        );
        let repo = Repository::find(&root).unwrap();
        assert_eq!((repo.top.as_path(), repo.prefix.as_str()), (&*root, ""));

        // A site that isn't in one can't have versions from git refs, and
        // its folder is named, tidied.
        let lone = scratch("find-none", &[("little.yml", ""), ("sub/little.yml", "")]);
        let not_in_one = format!(
            "the site's `versions` are built from git refs, so its folder, {}, must be in a git \
             repository: git rev-parse failed: fatal: not a git repository",
            lone.display()
        );
        for site_dir in [lone.clone(), lone.join("sub").join("..")] {
            let message = format!("{:#}", Repository::find(&site_dir).unwrap_err());
            assert!(message.starts_with(&not_in_one), "{message}");
        }
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&lone);
    }

    #[test]
    fn checks_out_only_the_files_asked_for_as_they_were() {
        // Many projects keep their docs out of the archives git makes of
        // them, but not out of their checkouts.
        let root = repository(
            "extract",
            &[
                ("docs/little.yml", "name: Old\n"),
                ("docs/guide.md", "# Old\n"),
                ("docs/a b/[c].md", "# C\n"),
                ("src/lib.py", "x = 1\n"),
                ("README.md", "# Read me\n"),
                (".gitattributes", "docs export-ignore\n"),
            ],
        );
        git_in(&root, &["tag", "--annotate", "-m", "One", "v1"]);
        let head = |root: &Path| {
            let output = git(root).args(["rev-parse", "HEAD"]).output().unwrap();
            String::from_utf8(output.stdout).unwrap().trim().to_string()
        };
        let v1 = head(&root);
        // What's changed since isn't checked out, committed or not.
        write(&root, &[("docs/guide.md", "# New\n"), ("docs/new.md", "")]);
        commit_all(&root);
        std::fs::remove_file(root.join("src/lib.py")).unwrap();

        let repo = Repository::find(&root.join("docs")).unwrap();
        for git_ref in ["v1", "main~1", &v1[..7]] {
            let checkout = repo.checkout(git_ref).unwrap();
            assert_eq!(checkout.commit, v1);
            let site = checkout.site_dir().to_path_buf();
            assert!(site.is_dir());
            assert_eq!(files_in(&checkout.tree), [] as [&str; 0]);

            checkout.extract(&[&site.join("little.yml")]).unwrap();
            assert_eq!(files_in(&checkout.tree), ["docs/little.yml"]);
            checkout.extract(&[&site]).unwrap();
            let docs = ["docs/a b/[c].md", "docs/guide.md", "docs/little.yml"];
            assert_eq!(files_in(&checkout.tree), docs);
            assert_eq!(
                std::fs::read_to_string(site.join("guide.md")).unwrap(),
                "# Old\n"
            );

            // A path can lead out of the site's folder, but not out of the
            // repository, and one to nothing checks out nothing.
            let outside = checkout.dir.parent().unwrap().join("src");
            let lib = site.join("../src");
            checkout
                .extract(&[&lib, &site.join("missing"), &outside])
                .unwrap();
            assert_eq!(
                files_in(&checkout.tree),
                [&docs[..], &["src/lib.py"]].concat()
            );
            assert_eq!(
                checkout.describe(&lib.join("lib.py")),
                format!("{git_ref}:src/lib.py")
            );
            assert_eq!(checkout.in_repo(&outside), None);

            // The top of the repository is every file.
            checkout.extract(&[&checkout.tree]).unwrap();
            let all = [".gitattributes", "README.md"];
            assert_eq!(
                files_in(&checkout.tree),
                [&all[..], &docs, &["src/lib.py"]].concat()
            );

            // None of it's left once it's done with.
            let dir = checkout.dir.clone();
            drop(checkout);
            assert!(!dir.exists());
        }
        // The repository's own files are as they were.
        assert_eq!(
            std::fs::read_to_string(root.join("docs/guide.md")).unwrap(),
            "# New\n"
        );
        assert!(!root.join("src/lib.py").exists());
        let status = git(&root).args(["status", "--porcelain"]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&status.stdout), " D src/lib.py\n");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_ref_that_isnt_there_is_explained() {
        let root = repository("missing-ref", &[("docs/guide.md", "# Guide\n")]);
        git_in(&root, &["tag", "v1"]);
        commit_all(&root);
        let repo = Repository::find(&root).unwrap();
        let message = format!("{:#}", repo.checkout("v2").unwrap_err());
        let missing = format!(
            "there's no tag, branch, or commit `v2` in {}",
            root.display()
        );
        assert_eq!(message, missing);

        // A shallow clone has only some of the history, and its tags.
        let clone = root.with_file_name(format!("{}-clone", root.file_name().unwrap().display()));
        let _ = std::fs::remove_dir_all(&clone);
        let url = format!("file://{}", root.display());
        let parent = clone.parent().unwrap();
        let name = clone.file_name().unwrap().to_str().unwrap();
        git_in(parent, &["clone", "--quiet", "--depth", "1", &url, name]);
        let repo = Repository::find(&clone).unwrap();
        let message = format!("{:#}", repo.checkout("v1").unwrap_err());
        assert_eq!(
            message,
            format!(
                "there's no tag, branch, or commit `v1` in {}, a shallow clone, with only some \
                 of the repository's history: clone all of it, as `actions/checkout` does with \
                 `fetch-depth: 0`",
                clone.display()
            )
        );
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&clone);
    }
}
