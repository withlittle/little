//! The pages a plugin adds, their paths, and the links between them.

/// What a plugin adds: each page's path and Markdown, and what it warns
/// of.
#[derive(Debug, Default)]
pub struct Added {
    /// Each page's path, from the source's folder, and its Markdown.
    pub pages: Vec<(String, String)>,
    /// What it warns of.
    pub warnings: Vec<String>,
}

impl Added {
    /// Each page's path and contents, as the plugin's `files` hook adds it,
    /// once `warn` has warned of what it warns of.
    pub fn files(self, warn: impl Fn(&str)) -> Vec<(String, Vec<u8>)> {
        self.warnings.iter().for_each(|warning| warn(warning));
        let pages = self.pages.into_iter();
        pages
            .map(|(path, markdown)| (path, markdown.into_bytes()))
            .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_in_the_sources_folder_is_alone() {
        assert_eq!(join("api", "config.md"), "api/config.md");
        assert_eq!(join("", "config.md"), "config.md");
        assert_eq!(join(".", "index.ts"), "index.ts");
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
}
