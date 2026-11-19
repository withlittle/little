//! What little reads from a site's git repository, with the `git` command:
//! the files of a version of the site from a git ref (see
//! `config::Version`), and when the site, and each of its pages' files,
//! changed.
//!
//! A version's files are checked out into a folder of their own, apart from
//! the site's files as they are. Each file is checked out as `git checkout`
//! would check it out, with the `.gitattributes` of its commit, and their
//! filters, like Git LFS's, not as `git archive` would, which leaves out the
//! files marked `export-ignore`, as many projects mark their docs. Only the
//! files a build of a version reads are checked out: the site's
//! `little.yml`, its source, and the folders its plugins read.
//!
//! The dates are commits': a site's is that of the commit it's built from
//! (see [`Repository::head`]), and a file's are those of the commit that
//! added it, and of the newest to change it (see [`Repository::dates`]).

use anyhow::{Context, Result, anyhow, bail};
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};
use time::OffsetDateTime;

/// The git repository a site is in.
#[derive(Debug)]
pub struct Repository {
    /// Its top folder.
    top: PathBuf,
    /// The site's folder, from the top, like `docs/`, or empty for the top.
    prefix: String,
    /// Whether it's a shallow clone, with only some of its history.
    shallow: bool,
    /// The full name of the commit `HEAD` is at, if it has any commits.
    head: Option<String>,
}

/// A commit, and when it was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    /// Its full name.
    pub id: String,
    /// Its committer's date: when it joined the history it's in.
    pub date: OffsetDateTime,
}

/// When a file was added to a repository, and when it last changed there:
/// the dates of the commits that did so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dates {
    /// When the commit that added it was made.
    pub created: OffsetDateTime,
    /// When the newest commit to change it was made.
    pub modified: OffsetDateTime,
}

impl Repository {
    /// The repository the site at `site_dir` is in.
    pub fn find(site_dir: &Path) -> Result<Repository> {
        // In one command, as git can take a while to start. In a repository
        // with no commits yet, it writes all but the commit, and fails,
        // quietly.
        let mut command = git(site_dir);
        command.args(["rev-parse", "--show-toplevel", "--show-prefix"]);
        command.args(["--is-shallow-repository", "--verify", "--quiet"]);
        command.arg("HEAD^{commit}");
        let found = output(&mut command).and_then(|output| {
            match output.status.success() || !output.stdout.is_empty() {
                true => Ok(output.stdout),
                false => Err(failure(&command, &output)),
            }
        });
        let output = found.with_context(|| {
            // In full, as the site's folder is `.` by default, and its
            // `source` can be in a repository when it isn't.
            let folder = std::path::absolute(site_dir)
                .map_or_else(|_| site_dir.to_path_buf(), |dir| normalize(&dir));
            format!(
                "the site's `site.versions` are built from git refs, so its folder, {}, must be \
                 in a git repository",
                folder.display()
            )
        })?;
        let output = String::from_utf8_lossy(&output);
        let mut lines = output.lines();
        let top = lines.next().unwrap_or_default();
        let prefix = lines.next().unwrap_or_default();
        let shallow = lines.next() == Some("true");
        let head = lines.next().filter(|commit| !commit.is_empty());
        Ok(Repository {
            // With the system's separators, as little names other paths: git
            // writes `/`, even on Windows.
            top: Path::new(top).components().collect(),
            prefix: prefix.to_string(),
            shallow,
            head: head.map(str::to_string),
        })
    }

    /// Whether it's a shallow clone, with only some of its history, whose
    /// oldest commit seems to add every file.
    pub fn is_shallow(&self) -> bool {
        self.shallow
    }

    /// The full name of the commit `HEAD` is at, which the site's files, as
    /// they are, are built from, or `None` in a repository with no commits
    /// yet.
    pub fn head(&self) -> Option<&str> {
        self.head.as_deref()
    }

    /// When `commit` was made.
    pub fn date(&self, commit: &str) -> Result<OffsetDateTime> {
        let mut log = git(&self.top);
        log.args(["log", "-1", "--no-show-signature", "--format=%ct", commit]);
        let output = run(&mut log)?;
        time_of(String::from_utf8_lossy(&output).trim())
            .with_context(|| format!("git log gave no date for the commit {commit}"))
    }

    /// When each file in `folder`, from the top of the repository, like
    /// `docs`, was added, and when it last changed, in the history of
    /// `commit`: each by its path from the top, like `docs/guide.md`. A file
    /// git finds was renamed there keeps the history it had by its old name,
    /// and one that was removed and added again has only its history since.
    pub fn dates(&self, commit: &str, folder: &str) -> Result<HashMap<String, Dates>> {
        let mut log = git(&self.top);
        // As git writes it whatever the config of whoever builds the site: a
        // name's letters as they are, not escaped, the first commit's files
        // as added, renames found, and no merges, colours, or signatures.
        log.args(["-c", "core.quotePath=false", "-c", "log.showRoot=true"]);
        log.args(["--literal-pathspecs", "log", "--date-order", "--no-merges"]);
        log.args(["--no-color", "--no-show-signature", "-M", "--name-status"]);
        log.args(["--format=%ct", commit, "--"]);
        // The top of the repository is every file.
        if !folder.is_empty() {
            log.arg(folder);
        }
        let log = run(&mut log)?;
        Ok(dates_in(&String::from_utf8_lossy(&log)))
    }

    /// The path of `path`, in the site at `site_dir`, from the top of the
    /// repository, with each `/`, like `docs`, and empty for the top itself,
    /// or `None` for one outside the repository, or the site's folder.
    pub fn in_repo(&self, site_dir: &Path, path: &Path) -> Option<String> {
        let mut names: Vec<_> = (self.prefix.split('/'))
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect();
        for component in path.strip_prefix(site_dir).ok()?.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => drop(names.pop()?),
                Component::Normal(name) => names.push(name.to_string_lossy().into_owned()),
                _ => return None,
            }
        }
        Some(names.join("/"))
    }

    /// Ready the files of `git_ref`, a tag, branch, or commit, to be checked
    /// out, as `Checkout::extract` asks for them.
    pub fn checkout(&self, git_ref: &str) -> Result<Checkout> {
        let commit = self.commit(git_ref)?;
        // Named at random, and made only if it's new, so no other program can
        // make it first and read or change the files. Its path is absolute,
        // as git, which checks them out from the top of the repository, needs.
        let dir = tempfile::Builder::new()
            .prefix("little-version-")
            .tempdir()
            .context("creating a temporary folder")?;
        let tree = dir.path().join("tree");
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
        if self.shallow {
            bail!(
                "there's no tag, branch, or commit `{git_ref}` in {}, a shallow clone, with only \
                 some of the repository's history: fetch the rest with `git fetch --unshallow`",
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
    /// the `tree` they're checked out into. Dropping it removes it.
    dir: tempfile::TempDir,
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
        let list = self.dir.path().join("files");
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
        command.env("GIT_INDEX_FILE", self.dir.path().join("index"));
        command
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
    let output = output(command)?;
    if !output.status.success() {
        return Err(failure(command, &output));
    }
    Ok(output.stdout)
}

/// Run `command`, a git command, to its end, whether or not it fails.
fn output(command: &mut Command) -> Result<Output> {
    command.output().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => anyhow!("git isn't installed, or isn't on the PATH"),
        _ => anyhow!(e).context(format!("running git {}", name(command))),
    })
}

/// Why `command`, a git command that ended with `output`, failed.
fn failure(command: &Command, output: &Output) -> anyhow::Error {
    let name = name(command);
    let stderr = String::from_utf8_lossy(&output.stderr);
    match stderr.trim() {
        "" => anyhow!("git {name} failed, with {}", output.status),
        stderr => anyhow!("git {name} failed: {stderr}"),
    }
}

/// The git command `command` runs, like `ls-files`: the first name after
/// `-C <dir>` that isn't an option, or the setting a `-c` gives.
fn name(command: &Command) -> String {
    let mut args = command.get_args().skip(2);
    while let Some(arg) = args.next() {
        let arg = arg.to_string_lossy();
        if arg == "-c" {
            args.next();
        } else if !arg.starts_with('-') {
            return arg.into_owned();
        }
    }
    String::new()
}

/// The time that's `seconds` since 1970, as git writes a commit's date.
fn time_of(seconds: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::from_unix_timestamp(seconds.parse().ok()?).ok()
}

/// When each file was added, and when it last changed, from `log`: what
/// `git log` writes of the commits that changed them, the newest first. Each
/// commit is its date, in seconds since 1970, on a line of its own, then a
/// line for each file it changed, with a tab before each name, like
/// `M<tab>docs/guide.md`, or, for one it renamed,
/// `R097<tab>docs/guide.md<tab>docs/start.md`.
fn dates_in(log: &str) -> HashMap<String, Dates> {
    // The file each name is, at the commits read so far, by the name it has
    // at the newest: its own, unless it's here, with `None` for a name that
    // is none of the newest commit's files, as before a file was added.
    let mut names: HashMap<&str, Option<&str>> = HashMap::new();
    fn named<'a>(names: &HashMap<&'a str, Option<&'a str>>, name: &'a str) -> Option<&'a str> {
        match names.get(name) {
            Some(file) => *file,
            None => Some(name),
        }
    }
    let mut dates: HashMap<&str, Dates> = HashMap::new();
    let mut date = None;
    for line in log.lines() {
        let mut fields = line.split('\t');
        let (Some(status), Some(name)) = (fields.next(), fields.next()) else {
            date = time_of(line).or(date);
            continue;
        };
        let changed = match status.as_bytes().first() {
            // Before this commit, the file had its old name, and its new
            // name was no file, or another.
            Some(b'R') => fields.next().and_then(|new| {
                let file = named(&names, new);
                names.insert(new, None);
                names.insert(name, file);
                file
            }),
            // A copy is a new file, as one that's added is, so anything
            // older by its name is another file.
            Some(b'C') => fields.next().and_then(|copy| {
                let file = named(&names, copy);
                names.insert(copy, None);
                file
            }),
            Some(b'A') => {
                let file = named(&names, name);
                names.insert(name, None);
                file
            }
            // What's removed is a file from before any that's there now.
            Some(b'D') => {
                names.insert(name, None);
                None
            }
            _ => named(&names, name),
        };
        if let (Some(file), Some(date)) = (changed, date) {
            (dates.entry(file))
                .and_modify(|dates| dates.created = date)
                .or_insert(Dates {
                    created: date,
                    modified: date,
                });
        }
    }
    (dates.into_iter())
        .map(|(file, dates)| (file.to_string(), dates))
        .collect()
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
    use crate::testing::{self, files_in, write};
    use tempfile::TempDir;

    /// Run git in `dir`, for a test, as no one in particular, whatever the
    /// git config of whoever runs it, and give what it writes, trimmed.
    pub(crate) fn git_in(dir: &Path, args: &[&str]) -> String {
        git_at(dir, None, args)
    }

    /// Run git in `dir`, as [`git_in`] does, with `date`, if there's one, as
    /// the committer's date of a commit it makes, like `2026-01-05T10:00:00Z`.
    fn git_at(dir: &Path, date: Option<&str>, args: &[&str]) -> String {
        let mut command = Command::new("git");
        command
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
            .args(args);
        if let Some(date) = date {
            command.env("GIT_COMMITTER_DATE", date);
        }
        let output = command.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "git {args:?}: {stderr}");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Make `dir` a fresh git repository, whose files are checked out as
    /// they're committed, by little too, whatever the system's git config:
    /// Git for Windows sets `core.autocrlf`, which checks lines out ending
    /// in `\r\n`.
    pub(crate) fn init(dir: &Path) {
        git_in(dir, &["init", "--quiet"]);
        git_in(dir, &["config", "core.autocrlf", "false"]);
    }

    /// Commit every file in the repository at `dir`, as it is.
    pub(crate) fn commit_all(dir: &Path) {
        commit(dir, None);
    }

    /// Commit every file in the repository at `dir`, as it is, at `date`, as
    /// its committer's date, like `2026-01-05T10:00:00Z`. Its author's date
    /// is still now.
    pub(crate) fn commit_all_at(dir: &Path, date: &str) {
        commit(dir, Some(date));
    }

    fn commit(dir: &Path, date: Option<&str>) {
        git_in(dir, &["add", "--all"]);
        let commit = [
            "commit",
            "--quiet",
            "--no-verify",
            "--allow-empty",
            "-m",
            "Change",
        ];
        git_at(dir, date, &commit);
    }

    /// The time `text` is, as RFC 3339 writes it, like `2026-01-05T10:00:00Z`.
    pub(crate) fn when(text: &str) -> OffsetDateTime {
        OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339).unwrap()
    }

    /// A fresh git repository, with `files`, each a (path, contents) pair,
    /// committed.
    fn repository(files: &[(&str, &str)]) -> (TempDir, PathBuf) {
        let (tmp, root) = scratch(files);
        init(&root);
        commit_all(&root);
        (tmp, root)
    }

    /// A fresh scratch folder holding `files`, each a (path, contents) pair.
    fn scratch(files: &[(&str, &str)]) -> (TempDir, PathBuf) {
        let (tmp, root) = testing::scratch(files);
        // The folder as git names it: on macOS, the one the temporary
        // folder's link leads to, and on Windows, by its long name, not one
        // like `RUNNER~1`, and without the `\\?\` that `canonicalize` adds.
        (tmp, dunce::canonicalize(root).unwrap())
    }

    #[test]
    fn finds_where_a_site_is_in_its_repository() {
        let (_tmp, root) = repository(&[("docs/little.yml", ""), ("README.md", "")]);
        let repo = Repository::find(&root.join("docs")).unwrap();
        assert_eq!(
            (repo.top.as_path(), repo.prefix.as_str()),
            (&*root, "docs/")
        );
        let repo = Repository::find(&root).unwrap();
        assert_eq!((repo.top.as_path(), repo.prefix.as_str()), (&*root, ""));

        // A site that isn't in one can't have versions from git refs, and
        // its folder is named, tidied.
        let (_lone, lone) = scratch(&[("little.yml", ""), ("sub/little.yml", "")]);
        let not_in_one = format!(
            "the site's `site.versions` are built from git refs, so its folder, {}, must be in a \
             git repository: git rev-parse failed: fatal: not a git repository",
            lone.display()
        );
        for site_dir in [lone.clone(), lone.join("sub").join("..")] {
            let message = format!("{:#}", Repository::find(&site_dir).unwrap_err());
            assert!(message.starts_with(&not_in_one), "{message}");
        }
    }

    #[test]
    fn checks_out_only_the_files_asked_for_as_they_were() {
        // Many projects keep their docs out of the archives git makes of
        // them, but not out of their checkouts.
        let (_tmp, root) = repository(&[
            ("docs/little.yml", "site: { name: Old }\n"),
            ("docs/guide.md", "# Old\n"),
            ("docs/a b/[c].md", "# C\n"),
            ("src/lib.py", "x = 1\n"),
            ("README.md", "# Read me\n"),
            (".gitattributes", "docs export-ignore\n"),
        ]);
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
            let outside = checkout.dir.path().parent().unwrap().join("src");
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
            let dir = checkout.dir.path().to_path_buf();
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
    }

    #[test]
    fn a_ref_that_isnt_there_is_explained() {
        let (_tmp, root) = repository(&[("docs/guide.md", "# Guide\n")]);
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
        let (_clone, clone) = scratch(&[]);
        let url = url::Url::from_file_path(&root).unwrap().to_string();
        git_in(&clone, &["clone", "--quiet", "--depth", "1", &url, "."]);
        let repo = Repository::find(&clone).unwrap();
        let message = format!("{:#}", repo.checkout("v1").unwrap_err());
        assert_eq!(
            message,
            format!(
                "there's no tag, branch, or commit `v1` in {}, a shallow clone, with only some \
                 of the repository's history: fetch the rest with `git fetch --unshallow`",
                clone.display()
            )
        );
    }

    #[test]
    fn a_repository_has_its_newest_commit_and_whether_its_shallow() {
        // Without a commit, it's still a repository.
        let (_tmp, root) = scratch(&[("docs/README.md", "# Home\n")]);
        init(&root);
        let repo = Repository::find(&root).unwrap();
        assert_eq!((repo.head(), repo.is_shallow()), (None, false));

        // A commit's date is its committer's, not its author's, which is now.
        commit_all_at(&root, "2026-01-05T10:00:00+11:00");
        let repo = Repository::find(&root.join("docs")).unwrap();
        let head = git_in(&root, &["rev-parse", "HEAD"]);
        assert_eq!((repo.head(), repo.is_shallow()), (Some(&*head), false));
        assert_eq!(repo.date(&head).unwrap(), when("2026-01-04T23:00:00Z"));
        let message = format!("{:#}", repo.date("0000000").unwrap_err());
        assert!(message.starts_with("git log failed: "), "{message}");

        // A shallow clone has it whole, but not the commits before it.
        commit_all_at(&root, "2026-02-01T00:00:00Z");
        let (_clone, clone) = scratch(&[]);
        let url = url::Url::from_file_path(&root).unwrap().to_string();
        git_in(&clone, &["clone", "--quiet", "--depth", "1", &url, "."]);
        let repo = Repository::find(&clone).unwrap();
        let head = git_in(&root, &["rev-parse", "HEAD"]);
        assert_eq!((repo.head(), repo.is_shallow()), (Some(&*head), true));
        assert_eq!(repo.date(&head).unwrap(), when("2026-02-01T00:00:00Z"));
    }

    #[test]
    fn a_path_in_a_site_is_named_from_the_top_of_its_repository() {
        let (_tmp, root) = repository(&[("website/little.yml", ""), ("docs/README.md", "")]);
        let site = root.join("website");
        let repo = Repository::find(&site).unwrap();
        let in_repo = |path: &Path| repo.in_repo(&site, path);
        assert_eq!(
            in_repo(&site.join("pages")).as_deref(),
            Some("website/pages")
        );
        assert_eq!(in_repo(&site.join(".")).as_deref(), Some("website"));
        assert_eq!(in_repo(&site.join("../docs")).as_deref(), Some("docs"));
        assert_eq!(in_repo(&site.join("..")).as_deref(), Some(""));
        // Outside the repository, or the site's folder.
        assert_eq!(in_repo(&site.join("../../docs")), None);
        assert_eq!(in_repo(&root.join("docs")), None);
    }

    #[test]
    fn each_files_dates_are_read_from_a_log() {
        // The newest commit first. `guide.md` was `start.md`, and `README.md`
        // was removed, then added again.
        let log = "300\n\nM\tdocs/guide.md\nA\tdocs/new.md\n\
                   250\n\nR090\tdocs/start.md\tdocs/guide.md\nA\tdocs/README.md\n\
                   200\n\nM\tdocs/start.md\nD\tdocs/README.md\n\
                   150\n\nM\tdocs/README.md\n\
                   100\n\nA\tdocs/start.md\nA\tdocs/README.md\n";
        let at = |seconds| OffsetDateTime::from_unix_timestamp(seconds).unwrap();
        let mut dates: Vec<_> = (dates_in(log).into_iter())
            .map(|(file, dates)| (file, dates.created, dates.modified))
            .collect();
        dates.sort();
        let expected = [
            ("docs/README.md".to_string(), at(250), at(250)),
            ("docs/guide.md".to_string(), at(100), at(300)),
            ("docs/new.md".to_string(), at(300), at(300)),
        ];
        assert_eq!(dates, expected);

        // Lines that are neither a date nor a file change nothing.
        let noisy = "gpg: Signature made Thu 19 Nov 2026\n300\n\nM\tdocs/guide.md\n\n";
        assert_eq!(dates_in(noisy).len(), 1);
        assert!(dates_in("").is_empty());
    }

    #[test]
    fn each_files_dates_are_its_commits() {
        let install = "# Install\n\nDownload little.\n\nUnpack it.\n\nRun it.\n";
        let (_tmp, root) = scratch(&[
            ("docs/README.md", "# Home\n"),
            ("docs/guide/install.md", install),
            ("docs/café.md", "# Café\n"),
            ("notes.md", "Notes\n"),
        ]);
        init(&root);
        commit_all_at(&root, "2026-01-05T10:00:00+11:00");
        write(&root, &[("docs/README.md", "# Home\n\nWelcome.\n")]);
        commit_all_at(&root, "2026-02-01T00:00:00Z");
        // Renamed, and changed a little, in one commit.
        git_in(&root, &["mv", "docs/guide/install.md", "docs/setup.md"]);
        let setup = format!("{install}\nThat's all.\n");
        write(&root, &[("docs/setup.md", &setup)]);
        commit_all_at(&root, "2026-03-01T00:00:00Z");
        // Not committed.
        write(&root, &[("docs/draft.md", "# Draft\n")]);
        write(&root, &[("docs/README.md", "# Home\n\nWelcome!\n")]);

        let repo = Repository::find(&root).unwrap();
        let head = repo.head().unwrap();
        let (jan, feb, mar) = (
            when("2026-01-04T23:00:00Z"),
            when("2026-02-01T00:00:00Z"),
            when("2026-03-01T00:00:00Z"),
        );
        let listed = |folder: &str| {
            let mut dates: Vec<_> = (repo.dates(head, folder).unwrap().into_iter())
                .map(|(file, dates)| (file, dates.created, dates.modified))
                .collect();
            dates.sort();
            dates
        };
        let docs = [
            ("docs/README.md".to_string(), jan, feb),
            ("docs/café.md".to_string(), jan, jan),
            ("docs/setup.md".to_string(), jan, mar),
        ];
        assert_eq!(listed("docs"), docs);
        // The top of the repository is every file.
        let mut every = docs.to_vec();
        every.push(("notes.md".to_string(), jan, jan));
        assert_eq!(listed(""), every);

        // A commit before the others has only its own history.
        let first = git_in(&root, &["rev-parse", "HEAD~2"]);
        let dates = repo.dates(&first, "docs/guide").unwrap();
        let install = Dates {
            created: jan,
            modified: jan,
        };
        assert_eq!(dates.get("docs/guide/install.md"), Some(&install));
        assert_eq!(dates.len(), 1);
    }
}
