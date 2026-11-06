//! Text: HTML's escapes, which asciidoc-parser writes some text in, an
//! image's default alt text and path, and the macros the plugin doesn't
//! read, which are left as text.

/// `text`, escaped as HTML.
pub(crate) fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The text of `html`, without its tags, and with its character references
/// decoded.
pub(crate) fn plain(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    decode(&text)
}

/// `html`, with each character reference decoded, but for a named one the
/// plugin doesn't know, which is left as it is.
pub(crate) fn decode(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('&') {
        text.push_str(&rest[..start]);
        rest = &rest[start..];
        let decoded = rest
            .find(';')
            .filter(|end| *end <= 32)
            .and_then(|end| Some((reference(&rest[1..end])?, end)));
        match decoded {
            Some((c, end)) => {
                text.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                text.push('&');
                rest = &rest[1..];
            }
        }
    }
    text.push_str(rest);
    text
}

/// The character a reference names, without its `&` and `;`: `#169`,
/// `#xA9` or `copy`.
pub(crate) fn reference(name: &str) -> Option<char> {
    if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16),
            None => number.parse(),
        };
        return code.ok().and_then(char::from_u32);
    }
    ENTITIES
        .iter()
        .find(|(entity, _)| *entity == name)
        .map(|(_, c)| *c)
}

/// HTML's named character references that authors write most, which an
/// AsciiDoc file can hold as they are.
const ENTITIES: &[(&str, char)] = &[
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("nbsp", '\u{a0}'),
    ("ensp", '\u{2002}'),
    ("emsp", '\u{2003}'),
    ("thinsp", '\u{2009}'),
    ("zwnj", '\u{200c}'),
    ("zwj", '\u{200d}'),
    ("shy", '\u{ad}'),
    ("copy", '©'),
    ("reg", '®'),
    ("trade", '™'),
    ("hellip", '…'),
    ("mdash", '—'),
    ("ndash", '–'),
    ("lsquo", '‘'),
    ("rsquo", '’'),
    ("sbquo", '‚'),
    ("ldquo", '“'),
    ("rdquo", '”'),
    ("bdquo", '„'),
    ("laquo", '«'),
    ("raquo", '»'),
    ("lsaquo", '‹'),
    ("rsaquo", '›'),
    ("bull", '•'),
    ("middot", '·'),
    ("prime", '′'),
    ("Prime", '″'),
    ("dagger", '†'),
    ("Dagger", '‡'),
    ("permil", '‰'),
    ("sect", '§'),
    ("para", '¶'),
    ("deg", '°'),
    ("plusmn", '±'),
    ("times", '×'),
    ("divide", '÷'),
    ("minus", '−'),
    ("frac12", '½'),
    ("frac14", '¼'),
    ("frac34", '¾'),
    ("sup1", '¹'),
    ("sup2", '²'),
    ("sup3", '³'),
    ("micro", 'µ'),
    ("euro", '€'),
    ("pound", '£'),
    ("yen", '¥'),
    ("cent", '¢'),
    ("curren", '¤'),
    ("iexcl", '¡'),
    ("iquest", '¿'),
    ("larr", '←'),
    ("rarr", '→'),
    ("uarr", '↑'),
    ("darr", '↓'),
    ("harr", '↔'),
    ("lArr", '⇐'),
    ("rArr", '⇒'),
    ("hArr", '⇔'),
    ("infin", '∞'),
    ("ne", '≠'),
    ("le", '≤'),
    ("ge", '≥'),
    ("asymp", '≈'),
    ("equiv", '≡'),
    ("sum", '∑'),
    ("prod", '∏'),
    ("radic", '√'),
    ("part", '∂'),
    ("nabla", '∇'),
    ("isin", '∈'),
    ("notin", '∉'),
    ("forall", '∀'),
    ("exist", '∃'),
    ("empty", '∅'),
    ("cap", '∩'),
    ("cup", '∪'),
    ("and", '∧'),
    ("or", '∨'),
    ("not", '¬'),
    ("alpha", 'α'),
    ("beta", 'β'),
    ("gamma", 'γ'),
    ("delta", 'δ'),
    ("epsilon", 'ε'),
    ("lambda", 'λ'),
    ("mu", 'μ'),
    ("pi", 'π'),
    ("sigma", 'σ'),
    ("omega", 'ω'),
    ("check", '✓'),
    ("loz", '◊'),
];

/// The alt text Asciidoctor gives an image without one: its file's name,
/// without its extension, with `-` and `_` as spaces.
pub(crate) fn default_alt(target: &str) -> String {
    let name = target.rsplit('/').next().unwrap_or(target);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    stem.replace(['-', '_'], " ")
}

/// The path of an image, `target`, in the folder `imagesdir` names, as
/// Asciidoctor gives it, but for a URL or an absolute path.
pub(crate) fn in_folder(folder: Option<&str>, target: &str) -> String {
    match folder.map(|folder| folder.trim_end_matches('/')) {
        Some(folder)
            if !folder.is_empty()
                && !target.contains("://")
                && !target.starts_with('/')
                && !target.starts_with("data:") =>
        {
            format!("{folder}/{target}")
        }
        _ => target.to_string(),
    }
}

/// The URL of a cross-reference to another document, `written`, as it's
/// written, but with `.adoc` after a path with no extension, as Asciidoctor
/// reads `other#id` as `other.adoc#id`; little leads it to its page.
pub(crate) fn document(written: &str) -> String {
    let (path, fragment) = written.split_once('#').unwrap_or((written, ""));
    let mut url = path.to_string();
    if !path.rsplit('/').next().unwrap_or(path).contains('.') {
        url.push_str(".adoc");
    }
    if !fragment.is_empty() {
        url.push('#');
        url.push_str(fragment);
    }
    url
}

/// A macro in text, like `name:target[attributes]`, which asciidoc-parser
/// leaves as text, as it doesn't read it: where it starts, in bytes, the
/// macro, and its name.
pub(crate) fn macros(text: &str) -> Vec<(usize, &str, &str)> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let preceded = start > 0 && {
            let before = bytes[start - 1];
            before.is_ascii_alphanumeric() || matches!(before, b'\\' | b'_' | b'-' | b':')
        };
        if bytes[start].is_ascii_lowercase()
            && !preceded
            && let Some(end) = macro_at(bytes, start)
        {
            let name_end = start + text[start..].find(':').unwrap_or(0);
            found.push((start, &text[start..end], &text[start..name_end]));
            start = end;
        } else {
            start += 1;
        }
    }
    found
}

/// Where the macro that starts at `start` ends, if one does.
fn macro_at(bytes: &[u8], start: usize) -> Option<usize> {
    let mut at = start;
    while at < bytes.len()
        && (bytes[at].is_ascii_alphanumeric() || matches!(bytes[at], b'_' | b'-'))
    {
        at += 1;
    }
    if bytes.get(at) != Some(&b':') {
        return None;
    }
    at += 1;
    if bytes.get(at) == Some(&b':') {
        at += 1;
    }
    if bytes.get(at) == Some(&b'/') {
        return None;
    }
    while at < bytes.len() && !bytes[at].is_ascii_whitespace() && bytes[at] != b'[' {
        at += 1;
    }
    if bytes.get(at) != Some(&b'[') {
        return None;
    }
    let mut escaped = false;
    for (offset, byte) in bytes[at + 1..].iter().enumerate() {
        match byte {
            b'\\' => escaped = !escaped,
            b']' if !escaped => return Some(at + 2 + offset),
            b'\n' => return None,
            _ => escaped = false,
        }
    }
    None
}
