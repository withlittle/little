//! The files of a package's code that the plugin reads, the module each
//! relative import leads to, and the page that shows each module.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use little_api_reference::model::Kind;

/// The extensions of the files the plugin reads, the one it prefers first,
/// when a folder has files with the same name but for it, like `index.ts`
/// and `index.js`.
const EXTENSIONS: [&str; 11] = [
    ".ts", ".tsx", ".mts", ".cts", ".d.ts", ".d.mts", ".d.cts", ".js", ".jsx", ".mjs", ".cjs",
];

/// A file of the package's code.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    /// Its path from the folder read, like `util/strings.ts`.
    pub path: String,
    /// Its path without its extension, like `util/strings`, which names its
    /// module.
    pub id: String,
}

/// `name` in `folder`, with `/` between them, or `name` alone if `folder`
/// is empty.
pub fn join(folder: &str, name: &str) -> String {
    match folder {
        "" => name.to_string(),
        _ => format!("{folder}/{name}"),
    }
}

/// `name` without its extension, and the extension's place in
/// [`EXTENSIONS`], if it's one the plugin reads.
fn split(name: &str) -> Option<(&str, usize)> {
    (EXTENSIONS.iter().enumerate())
        .filter(|(_, extension)| name.len() > extension.len() && name.ends_with(*extension))
        .max_by_key(|(_, extension)| extension.len())
        .map(|(rank, extension)| (&name[..name.len() - extension.len()], rank))
}

/// The files of JavaScript and TypeScript in `folder`, and in the folders
/// in it, but `node_modules` and those whose names start with `.` or `_`,
/// in order of their modules' ids; of a module's files, like `index.ts` and
/// `index.js`, the one whose extension comes first in [`EXTENSIONS`]. None,
/// warning why, if `folder` can't be read.
pub fn walk(folder: &str, warnings: &mut Vec<String>) -> Option<Vec<Source>> {
    let mut found = BTreeMap::new();
    visit(folder, "", &mut found, warnings)?;
    let sources = found.into_iter().map(|(id, (_, path))| Source { path, id });
    Some(sources.collect())
}

/// Add the files in `folder`, in `root`, and in the folders in it, to
/// `found`, by their modules' ids, with their extensions' ranks; or none,
/// warning why, if `folder` can't be read.
fn visit(
    root: &str,
    folder: &str,
    found: &mut BTreeMap<String, (usize, String)>,
    warnings: &mut Vec<String>,
) -> Option<()> {
    let path = match folder {
        "" => root.to_string(),
        folder => join(root, folder),
    };
    let names = std::fs::read_dir(&path).map(|entries| {
        let names = entries.filter_map(|entry| entry.ok()?.file_name().into_string().ok());
        names.collect::<Vec<_>>()
    });
    let mut names = match names {
        Ok(names) => names,
        Err(error) => {
            warnings.push(format!("can't read `{path}`: {error}"));
            return None;
        }
    };
    names.sort();
    for name in names {
        if name.starts_with(['.', '_']) || name == "node_modules" {
            continue;
        }
        let inner = join(folder, &name);
        let Ok(metadata) = std::fs::metadata(join(&path, &name)) else {
            continue;
        };
        if metadata.is_dir() {
            visit(root, &inner, found, warnings);
        } else if let Some((stem, rank)) = split(&name) {
            let id = join(folder, stem);
            if found.get(&id).is_none_or(|(best, _)| rank < *best) {
                found.insert(id, (rank, inner));
            }
        }
    }
    Some(())
}

/// The module that `specifier`, imported by the file at `from`, leads to,
/// by its index in `ids`, if it's relative and leads to one: `./config` to
/// `config.ts`, or else `config/index.ts`, and `./config.js` to
/// `config.ts`, as TypeScript reads it.
pub fn resolve(from: &str, specifier: &str, ids: &HashMap<String, usize>) -> Option<usize> {
    let relative = matches!(specifier, "." | "..")
        || specifier.starts_with("./")
        || specifier.starts_with("../");
    if !relative {
        return None;
    }
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    let pieces: Vec<&str> = specifier.split('/').collect();
    for piece in &pieces {
        match *piece {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            piece => parts.push(piece),
        }
    }
    let path = parts.join("/");
    let index = join(&path, "index");
    if specifier.ends_with('/') || matches!(pieces.last(), Some(&("." | ".."))) {
        return ids.get(&index).copied();
    }
    let found = match split(&path) {
        Some((stem, _)) => ids.get(stem),
        None => ids.get(&path).or_else(|| ids.get(&index)),
    };
    found.copied()
}

/// A page of the reference.
#[derive(Debug, PartialEq)]
pub struct Page {
    /// Its path, from the folder the pages go in, like `util/README.md`.
    pub path: String,
    /// Its title: the package's name, or its folder's, or its module's.
    pub title: String,
    /// `Package`, for the package's page, or `Module`.
    pub kind: Kind,
    /// The module it shows, by its index, if it shows one.
    pub module: Option<usize>,
    /// The pages of the folders and modules in its folder, if it's a
    /// folder's, by their indexes, in order of their titles.
    pub children: Vec<usize>,
}

/// The pages of a package's reference.
#[derive(Debug)]
pub struct Layout {
    /// Its pages, the package's first.
    pub pages: Vec<Page>,
    /// The page that shows each module, by its index, if one does.
    pub module_page: Vec<Option<usize>>,
}

/// The pages of the package called `package`, whose modules are in
/// `sources`, of which those `documented` are shown: the package's own,
/// `README.md`, which shows its `index` module; one for each folder a
/// module shown is in, its `README.md`, which shows the folder's `index`
/// module, or else the module beside it with its name; and one for each
/// other module shown, named as its file is, but with `.md`.
pub fn layout(package: &str, sources: &[Source], documented: &[bool]) -> Layout {
    let ids: BTreeMap<&str, usize> = (sources.iter().enumerate())
        .filter(|(index, _)| documented[*index])
        .map(|(index, source)| (source.id.as_str(), index))
        .collect();
    let folders: BTreeSet<&str> = (ids.keys())
        .flat_map(|id| id.match_indices('/').map(|(slash, _)| &id[..slash]))
        .collect();
    let mut pages = vec![Page {
        path: "README.md".to_string(),
        title: package.to_string(),
        kind: Kind::Package,
        module: ids.get("index").copied(),
        children: Vec::new(),
    }];
    // The page of each folder, by its path, and the folder each page is in.
    let mut folder_pages = HashMap::from([("", 0)]);
    let mut parents = vec![None];
    for &folder in &folders {
        let index = ids.get(join(folder, "index").as_str());
        folder_pages.insert(folder, pages.len());
        parents.push(Some(parent(folder)));
        pages.push(Page {
            path: format!("{folder}/README.md"),
            title: last(folder).to_string(),
            kind: Kind::Module,
            module: index.or_else(|| ids.get(folder)).copied(),
            children: Vec::new(),
        });
    }
    let mut module_page = vec![None; sources.len()];
    for (page, shown) in pages.iter().enumerate() {
        if let Some(module) = shown.module {
            module_page[module] = Some(page);
        }
    }
    for (&id, &module) in &ids {
        if module_page[module].is_none() {
            module_page[module] = Some(pages.len());
            parents.push(Some(parent(id)));
            pages.push(Page {
                path: format!("{id}.md"),
                title: last(id).to_string(),
                kind: Kind::Module,
                module: Some(module),
                children: Vec::new(),
            });
        }
    }
    for (page, folder) in parents.into_iter().enumerate() {
        if let Some(folder) = folder {
            pages[folder_pages[folder]].children.push(page);
        }
    }
    for index in 0..pages.len() {
        let mut children = std::mem::take(&mut pages[index].children);
        children.sort_by(|&a, &b| {
            (&pages[a].title, &pages[a].path).cmp(&(&pages[b].title, &pages[b].path))
        });
        pages[index].children = children;
    }
    Layout { pages, module_page }
}

/// The folder `path` is in, or empty if it's in none.
fn parent(path: &str) -> &str {
    path.rfind('/').map_or("", |slash| &path[..slash])
}

/// The last part of `path`, like `strings` for `util/strings`.
fn last(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources(ids: &[&str]) -> Vec<Source> {
        let sources = ids.iter().map(|id| Source {
            path: format!("{id}.ts"),
            id: id.to_string(),
        });
        sources.collect()
    }

    #[test]
    fn files_of_typescript_come_before_javascript_and_its_declarations() {
        assert_eq!(split("index.ts"), Some(("index", 0)));
        assert_eq!(split("index.d.ts"), Some(("index", 4)));
        assert_eq!(split("types.d.mts"), Some(("types", 5)));
        assert_eq!(split("app.jsx"), Some(("app", 8)));
        assert_eq!(split("data.json"), None);
        assert_eq!(split(".ts"), None);
    }

    #[test]
    fn relative_imports_lead_to_files_or_their_folders_index() {
        let ids: HashMap<String, usize> = [("index", 0), ("util/strings", 1), ("util/index", 2)]
            .map(|(id, index)| (id.to_string(), index))
            .into();
        assert_eq!(resolve("index.ts", "./util/strings", &ids), Some(1));
        assert_eq!(resolve("index.ts", "./util/strings.js", &ids), Some(1));
        assert_eq!(resolve("index.ts", "./util", &ids), Some(2));
        assert_eq!(resolve("index.ts", "./util/", &ids), Some(2));
        assert_eq!(resolve("util/strings.ts", "..", &ids), Some(0));
        assert_eq!(resolve("util/strings.ts", "../index.js", &ids), Some(0));
        assert_eq!(resolve("util/strings.ts", ".", &ids), Some(2));
        assert_eq!(resolve("index.ts", "../index", &ids), None);
        assert_eq!(resolve("index.ts", "util", &ids), None);
        assert_eq!(resolve("index.ts", "./missing", &ids), None);
    }

    #[test]
    fn each_folder_with_a_module_shown_has_a_page_that_lists_whats_in_it() {
        let sources = sources(&["index", "a", "a/x", "b/index", "b/c/y", "z"]);
        let layout = layout("demo", &sources, &[true, true, true, true, true, false]);
        let pages: Vec<(&str, &str, Option<usize>)> = (layout.pages.iter())
            .map(|page| (page.path.as_str(), page.title.as_str(), page.module))
            .collect();
        assert_eq!(
            pages,
            [
                ("README.md", "demo", Some(0)),
                ("a/README.md", "a", Some(1)),
                ("b/README.md", "b", Some(3)),
                ("b/c/README.md", "c", None),
                ("a/x.md", "x", Some(2)),
                ("b/c/y.md", "y", Some(4)),
            ]
        );
        assert_eq!(layout.pages[0].kind, Kind::Package);
        assert_eq!(layout.pages[0].children, [1, 2]);
        assert_eq!(layout.pages[1].children, [4]);
        assert_eq!(layout.pages[2].children, [3]);
        assert_eq!(layout.pages[3].children, [5]);
        let module_page = [Some(0), Some(1), Some(4), Some(2), Some(5), None];
        assert_eq!(layout.module_page, module_page);
    }
}
