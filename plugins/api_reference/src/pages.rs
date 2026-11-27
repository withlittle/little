//! The pages a plugin adds, their paths, and the links between them.

/// What a plugin adds: each page's path and Markdown, any other file, like
/// the one a page is made from, and what it warns of.
#[derive(Debug, Default)]
pub struct Added {
    /// Each page's path, from the source's folder, and its Markdown.
    pub pages: Vec<(String, String)>,
    /// Each other file's path, from the source's folder, and what it holds.
    pub others: Vec<(String, Vec<u8>)>,
    /// What it warns of.
    pub warnings: Vec<String>,
}

impl Added {
    /// Each file's path and contents, as the plugin's `files` hook adds it
    /// to a source that has the files at `source`, once `warn` has warned
    /// of what it warns of. A file that isn't a page is added only where
    /// the source has none: there, the site's own is the one published.
    pub fn files(self, source: &[String], warn: impl Fn(&str)) -> Vec<(String, Vec<u8>)> {
        self.warnings.iter().for_each(|warning| warn(warning));
        let pages = self.pages.into_iter();
        let pages = pages.map(|(path, markdown)| (path, markdown.into_bytes()));
        let others = self.others.into_iter();
        let others = others.filter(|(path, _)| !source.contains(path));
        pages.chain(others).collect()
    }
}

/// `name` in `folder`, with `/` between them, or `name` alone in `.` or
/// the source's folder.
pub fn join(folder: &str, name: &str) -> String {
    match folder {
        "" | "." => name.to_string(),
        _ => format!("{folder}/{name}"),
    }
}

/// The names of what's in `folder`, in order, or the warning that it can't
/// be read.
pub fn names(folder: &str) -> Result<Vec<String>, String> {
    let entries =
        std::fs::read_dir(folder).map_err(|error| format!("can't read `{folder}`: {error}"))?;
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .collect();
    names.sort();
    Ok(names)
}

/// The folder `path` is in, or empty if it's in none.
pub fn parent(path: &str) -> &str {
    path.rfind('/').map_or("", |slash| &path[..slash])
}

/// The last part of `path`, like `config` for `internal/config`.
pub fn last(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The relative link from the page at `from` to the one at `to`, each a
/// path from the source's folder, like `../config.md`.
pub fn relative(from: &str, to: &str) -> String {
    let folders: Vec<&str> = from.split('/').collect();
    let folders = &folders[..folders.len() - 1];
    let parts: Vec<&str> = to.split('/').collect();
    let shared = (folders.iter().zip(&parts[..parts.len() - 1]))
        .take_while(|(folder, part)| folder == part)
        .count();
    let up = std::iter::repeat_n("..", folders.len() - shared);
    up.chain(parts[shared..].iter().copied())
        .collect::<Vec<_>>()
        .join("/")
}

/// The link from the page at `from` to the heading with `id` on the page at
/// `to`: only `#` and `id`, if it's the same page.
pub fn heading(from: &str, to: &str, id: &str) -> String {
    match to == from {
        true => format!("#{id}"),
        false => format!("{}#{id}", relative(from, to)),
    }
}

/// The link from the page at `from` to the one at `to`, at the heading with
/// `id`, if there is one.
pub fn link(from: &str, to: &str, id: Option<&str>) -> String {
    match id {
        Some(id) => heading(from, to, id),
        None => relative(from, to),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::folder_with;

    #[test]
    fn a_file_that_isnt_a_page_is_added_only_where_the_source_has_none() {
        let added = Added {
            pages: vec![("api/pets.md".into(), "# Pets\n".into())],
            others: vec![
                ("api/pets.yaml".into(), b"openapi: 3.1.0\n".to_vec()),
                ("api/events.yaml".into(), b"asyncapi: 3.0.0\n".to_vec()),
            ],
            warnings: vec!["a warning".into()],
        };
        // The source has one of the files, and a file where the page goes,
        // which is for little to stop the build over.
        let source = ["api/events.yaml".to_string(), "api/pets.md".to_string()];
        let warned = std::cell::RefCell::new(Vec::new());
        let warn = |warning: &str| warned.borrow_mut().push(warning.to_string());
        assert_eq!(
            added.files(&source, warn),
            [
                ("api/pets.md".to_string(), b"# Pets\n".to_vec()),
                ("api/pets.yaml".to_string(), b"openapi: 3.1.0\n".to_vec()),
            ]
        );
        assert_eq!(*warned.borrow(), ["a warning"]);
    }

    #[test]
    fn a_name_in_the_sources_folder_is_alone() {
        assert_eq!(join("api", "config.md"), "api/config.md");
        assert_eq!(join("", "config.md"), "config.md");
        assert_eq!(join(".", "index.ts"), "index.ts");
    }

    #[test]
    fn a_folders_names_are_in_order() {
        let files = [("b.py", ""), ("a/c.py", "")];
        let root = folder_with("little-api-reference", "names", &files);
        assert_eq!(names(&root), Ok(vec!["a".to_string(), "b.py".to_string()]));
        std::fs::remove_dir_all(&root).unwrap();
        let warning = names(&root).unwrap_err();
        assert!(warning.starts_with(&format!("can't read `{root}`: ")));
    }

    #[test]
    fn a_path_is_in_the_folder_before_its_last_part() {
        assert_eq!(parent("internal/config"), "internal");
        assert_eq!(parent("config"), "");
        assert_eq!(last("internal/config"), "config");
        assert_eq!(last("config"), "config");
    }

    #[test]
    fn relative_links_go_up_to_the_folder_two_pages_share() {
        assert_eq!(relative("api/README.md", "api/config.md"), "config.md");
        assert_eq!(
            relative("api/site/versions.md", "api/config.md"),
            "../config.md"
        );
        assert_eq!(
            relative("api/config.md", "api/site/README.md"),
            "site/README.md"
        );
        assert_eq!(relative("README.md", "a/b/c.md"), "a/b/c.md");
        assert_eq!(relative("a/b/c.md", "README.md"), "../../README.md");
        assert_eq!(relative("api/config.md", "api/config.md"), "config.md");
    }

    #[test]
    fn a_link_to_a_heading_on_the_same_page_is_its_fragment() {
        assert_eq!(heading("api/config.md", "api/config.md", "load"), "#load");
        assert_eq!(
            heading("api/README.md", "api/config.md", "load"),
            "config.md#load"
        );
        assert_eq!(
            link("api/README.md", "api/config.md", Some("load")),
            "config.md#load"
        );
        assert_eq!(link("api/README.md", "api/config.md", None), "config.md");
    }
}
