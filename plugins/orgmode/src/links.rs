//! What Org's links lead to, and the plain links and angle links in text,
//! like `https://orgmode.org` and `<https://orgmode.org>`, which orgize
//! leaves as text.

/// The types of the plain and angle links the plugin reads in text: those
/// of the web, of a DOI, and of a file.
const PLAIN_TYPES: [&str; 7] = ["https", "http", "ftp", "mailto", "news", "doi", "file"];

/// The types of link Org knows that little can't follow, as they lead into
/// Emacs, or out of the site.
const OTHER_TYPES: [&str; 17] = [
    "elisp",
    "shell",
    "help",
    "info",
    "attachment",
    "bbdb",
    "bibtex",
    "docview",
    "eww",
    "gnus",
    "irc",
    "mhe",
    "rmail",
    "w3m",
    "man",
    "woman",
    "rss",
];

/// The extensions of the images Org's HTML export shows in place of a link
/// to them.
const IMAGES: [&str; 6] = ["png", "jpg", "jpeg", "gif", "svg", "webp"];

/// What a link leads to, by its path, as Org reads it.
#[derive(Debug, PartialEq)]
pub enum Link {
    /// A page on the web, at its URL, or a DOI's, at `https://doi.org/`.
    Web(String),
    /// A file, by its path, and what to search for in it, after `::`.
    File(String, Option<String>),
    /// The headline with a `CUSTOM_ID`, `[[#id]]`.
    CustomId(String),
    /// The headline with a title, `[[*Title]]`.
    Headline(String),
    /// The headline with an `ID` property, `[[id:…]]`.
    Id(String),
    /// What little can't follow: a line of code, `[[(ref)]]`, or a link
    /// into Emacs, like `elisp:`.
    Other,
    /// A target, `<<text>>`, an element's `#+NAME:`, or else a headline's
    /// title.
    Fuzzy(String),
}

impl Link {
    /// What the link with the path `path` leads to, as Org's
    /// `org-element-link-parser` reads it.
    pub fn of(path: &str) -> Link {
        if path.starts_with('/')
            || path.starts_with('~')
            || path.starts_with("./")
            || path.starts_with("../")
        {
            return file(path);
        }
        if let Some((kind, rest)) = path.split_once(':') {
            match kind {
                "http" | "https" | "ftp" | "mailto" | "news" => return Link::Web(path.to_string()),
                "doi" => return Link::Web(format!("https://doi.org/{rest}")),
                "file" | "file+sys" | "file+emacs" => return file(rest),
                "id" => return Link::Id(rest.to_string()),
                _ if OTHER_TYPES.contains(&kind) => return Link::Other,
                _ => {}
            }
        }
        if path.starts_with('(') && path.ends_with(')') {
            Link::Other
        } else if let Some(id) = path.strip_prefix('#') {
            Link::CustomId(id.to_string())
        } else if let Some(title) = path.strip_prefix('*') {
            Link::Headline(title.to_string())
        } else {
            Link::Fuzzy(path.to_string())
        }
    }
}

/// A link to the file `path`, with what to search for in it, after its
/// first `::`.
fn file(path: &str) -> Link {
    match path.split_once("::") {
        Some((path, search)) => Link::File(path.to_string(), Some(search.to_string())),
        None => Link::File(path.to_string(), None),
    }
}

/// Whether `path` is an image's, which Org's HTML export shows, by its
/// extension, in any case.
pub fn is_image(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        IMAGES
            .iter()
            .any(|image| image.eq_ignore_ascii_case(extension))
    })
}

/// The name of the file at `path`, without its folder, as Org's HTML export
/// gives an image as its `alt`.
pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The path of a link in brackets, as it's written in `raw`, without the
/// backslashes that escape its brackets, as `org-link-unescape` does, and
/// with each run of whitespace a space.
pub fn unescape(raw: &str) -> String {
    let mut path = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            path.push(c);
            continue;
        }
        let mut run = 1;
        while chars.peek() == Some(&'\\') {
            chars.next();
            run += 1;
        }
        let escapes = matches!(chars.peek(), None | Some('[' | ']'));
        let kept = if escapes { run / 2 } else { run };
        path.extend(std::iter::repeat_n('\\', kept));
    }
    path.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The text a headline's title is matched by, as `org-link--normalize-string`
/// gives it: without statistics cookies, like `[1/3]`, and with each run of
/// whitespace a space, trimmed.
pub fn normalize(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('[') {
        out.push_str(&rest[..start]);
        match cookie(&rest[start..]) {
            Some(length) => {
                out.push(' ');
                rest = &rest[start + length..];
            }
            None => {
                out.push('[');
                rest = &rest[start + 1..];
            }
        }
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The length of the statistics cookie, like `[1/3]` or `[50%]`, at the
/// start of `text`, if one is.
fn cookie(text: &str) -> Option<usize> {
    let inner = text.strip_prefix('[')?;
    let digits = inner.len() - inner.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let rest = &inner[digits..];
    let length = if rest.starts_with("%]") {
        digits + 2
    } else {
        let after = rest.strip_prefix('/')?;
        let more = after.len() - after.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        after[more..].strip_prefix(']')?;
        digits + more + 2
    };
    Some(1 + length)
}

/// `text` with each run of whitespace a space, trimmed, as a fuzzy link
/// and the target, or `#+NAME:`, it leads to are matched, and as a warning
/// shows what it's of.
pub fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The id of the target or `#+NAME:` `text`, which a fuzzy link to it
/// leads to: its [`words`], joined with `-`.
pub fn id_of(text: &str) -> String {
    words(text).replace(' ', "-")
}

/// The length of the plain link at the start of `text`, like
/// `https://orgmode.org`, as `org-link-plain-re` matches it: a type the
/// plugin reads, a `:`, and a path of at least two characters, with no
/// whitespace, `[]` or `<>`, and `()` only in pairs, that ends in a letter,
/// a digit, `-`, `/` or a pair of `()`. Whether it starts a word is for its
/// caller to check.
pub fn plain_link(text: &str) -> Option<usize> {
    let path_start = link_type(text)? + 1;
    let mut position = path_start;
    let mut units = 0;
    let mut end = None;
    while position < text.len() {
        let rest = &text[position..];
        let (length, ends) = if rest.starts_with('(') {
            match parenthesis(rest) {
                Some(length) => (length, true),
                None => break,
            }
        } else {
            let c = rest.chars().next()?;
            if is_outside_path(c) {
                break;
            }
            (c.len_utf8(), c == '-' || c == '/' || c.is_alphanumeric())
        };
        position += length;
        units += 1;
        if ends && units >= 2 {
            end = Some(position);
        }
    }
    end
}

/// The length of the angle link at the start of `text`, like
/// `<https://orgmode.org>`, as `org-link-angle-re` matches it, and its
/// link, without the line breaks in it, and the spaces around them.
pub fn angle_link(text: &str) -> Option<(usize, String)> {
    let inner = text.strip_prefix('<')?;
    link_type(inner)?;
    let close = inner.find('>')?;
    let mut link = String::new();
    for (index, line) in inner[..close].split('\n').enumerate() {
        if index == 0 {
            link.push_str(line);
            continue;
        }
        let line = line.trim_start_matches([' ', '\t']);
        if line.is_empty() {
            return None;
        }
        link.truncate(link.trim_end_matches([' ', '\t']).len());
        link.push_str(line);
    }
    Some((close + 2, link))
}

/// The length of the type of link at the start of `text`, one the plugin
/// reads in text, if it's followed by a `:`.
fn link_type(text: &str) -> Option<usize> {
    PLAIN_TYPES.iter().find_map(|kind| {
        text.strip_prefix(kind)
            .filter(|rest| rest.starts_with(':'))
            .map(|_| kind.len())
    })
}

/// Whether `c` can't be in a plain link's path, but in a pair of `()`.
fn is_outside_path(c: char) -> bool {
    matches!(
        c,
        '[' | ']' | ' ' | '\t' | '\n' | '\r' | '(' | ')' | '<' | '>'
    )
}

/// The length of the pair of `()` at the start of `text`, which can hold
/// what a path can, and pairs of `()` that hold only that.
fn parenthesis(text: &str) -> Option<usize> {
    let mut chars = text.char_indices().skip(1);
    while let Some((index, c)) = chars.next() {
        match c {
            ')' => return Some(index + 1),
            '(' => loop {
                let (_, inner) = chars.next()?;
                if inner == ')' {
                    break;
                }
                if is_outside_path(inner) {
                    return None;
                }
            },
            _ if is_outside_path(c) => return None,
            _ => {}
        }
    }
    None
}
