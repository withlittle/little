//! The site's `markdown` settings, which say what Markdown a page can be
//! written in.

use serde_json::Value;

/// Which of Markdown's extensions the site turns on, under little's names,
/// so a page is written only in Markdown the site reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// GitHub's tables.
    pub tables: bool,
    /// `~~struck through~~`.
    pub strikethrough: bool,
    /// Links from bare URLs and email addresses.
    pub autolinks: bool,
    /// `- [x]` items.
    pub task_lists: bool,
    /// `[^1]` footnotes.
    pub footnotes: bool,
    /// GitHub's alerts, `> [!NOTE]`.
    pub alerts: bool,
    /// `:emoji:` shortcodes.
    pub emoji: bool,
    /// HTML, kept as it's written.
    pub raw_html: bool,
    /// `$math$`, rendered in any way.
    pub math: bool,
    /// Curly quotes, dashes, and ellipses from straight ones.
    pub smart_punctuation: bool,
    /// `^superscript^`.
    pub superscript: bool,
    /// `~subscript~`.
    pub subscript: bool,
    /// `==highlighted==`.
    pub mark: bool,
    /// `++inserted++`.
    pub insert: bool,
    /// Terms, each with `: what it means` below it.
    pub description_lists: bool,
    /// `^[inline footnotes]`, with `footnotes` on.
    pub inline_footnotes: bool,
    /// Emphasis beside Chinese, Japanese, and Korean text.
    pub cjk_friendly_emphasis: bool,
}

impl Default for Settings {
    /// little's own: GitHub's extensions on, and the rest off.
    fn default() -> Self {
        Settings {
            tables: true,
            strikethrough: true,
            autolinks: true,
            task_lists: true,
            footnotes: true,
            alerts: true,
            emoji: true,
            raw_html: true,
            math: true,
            smart_punctuation: false,
            superscript: false,
            subscript: false,
            mark: false,
            insert: false,
            description_lists: false,
            inline_footnotes: false,
            cjk_friendly_emphasis: false,
        }
    }
}

impl Settings {
    /// Reads the settings from the JSON the `formats` hook is given, where a
    /// setting that's missing keeps little's default, and `math` is on unless
    /// it's `false`.
    pub fn from_json(json: &str) -> Result<Settings, String> {
        let value: Value = serde_json::from_str(json)
            .map_err(|error| format!("can't read `markdown`: {error}"))?;
        let Value::Object(map) = value else {
            return Err("`markdown` isn't a map".to_string());
        };
        let mut settings = Settings::default();
        for (key, value) in &map {
            if key == "math" {
                settings.math = value != &Value::Bool(false);
                continue;
            }
            let Some(on) = value.as_bool() else { continue };
            let setting = match key.as_str() {
                "tables" => &mut settings.tables,
                "strikethrough" => &mut settings.strikethrough,
                "autolinks" => &mut settings.autolinks,
                "task_lists" => &mut settings.task_lists,
                "footnotes" => &mut settings.footnotes,
                "alerts" => &mut settings.alerts,
                "emoji" => &mut settings.emoji,
                "raw_html" => &mut settings.raw_html,
                "smart_punctuation" => &mut settings.smart_punctuation,
                "superscript" => &mut settings.superscript,
                "subscript" => &mut settings.subscript,
                "mark" => &mut settings.mark,
                "insert" => &mut settings.insert,
                "description_lists" => &mut settings.description_lists,
                "inline_footnotes" => &mut settings.inline_footnotes,
                "cjk_friendly_emphasis" => &mut settings.cjk_friendly_emphasis,
                _ => continue,
            };
            *setting = on;
        }
        Ok(settings)
    }
}
