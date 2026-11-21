//! Input validation and output sanitisation.
//!
//! Tool arguments come from an LLM, which may itself be steered by prompt-injected Reddit
//! content, so every value that ends up in a request URL is checked against a strict
//! allow-list before it is used. Every string that comes back from Reddit is
//! user-generated and untrusted, so invisible / control characters that are commonly used
//! to hide instructions from a human reviewer are stripped before it reaches the model.

/// Maximum length of a search query, in characters.
pub const MAX_QUERY_CHARS: usize = 512;

/// Listing sorts accepted by `/r/{sub}/{sort}`.
pub const LISTING_SORTS: &[&str] = &["hot", "new", "top", "rising", "controversial"];
/// Sorts accepted by `/search` and `/r/{sub}/search`.
pub const SEARCH_SORTS: &[&str] = &["relevance", "new", "top", "comments", "hot"];
/// Time windows accepted by Reddit's `t` parameter.
pub const TIME_FILTERS: &[&str] = &["hour", "day", "week", "month", "year", "all"];

/// Validate a subreddit name (with or without an `r/` or `/r/` prefix) and return the bare
/// name. Reddit names are 2-21 ASCII letters, digits or underscores (`all`/`popular` pass).
pub fn subreddit(raw: &str) -> Result<&str, String> {
    let s = raw.trim();
    let s = s
        .strip_prefix("/r/")
        .or_else(|| s.strip_prefix("r/"))
        .unwrap_or(s);
    let ok =
        (2..=21).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if ok {
        Ok(s)
    } else {
        Err(format!(
            "invalid subreddit {raw:?}: expected 2-21 letters, digits or underscores"
        ))
    }
}

/// Validate a post id (bare base36, or a `t3_` fullname) and return the bare id.
pub fn post_id(raw: &str) -> Result<&str, String> {
    let s = raw.trim();
    let s = s.strip_prefix("t3_").unwrap_or(s);
    let ok = (1..=13).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase());
    if ok {
        Ok(s)
    } else {
        Err(format!(
            "invalid post_id {raw:?}: expected a base36 id such as \"1abcdef\" or \"t3_1abcdef\""
        ))
    }
}

/// Check `value` against a fixed allow-list.
pub fn choice<'a>(field: &str, value: &'a str, allowed: &[&str]) -> Result<&'a str, String> {
    if allowed.contains(&value) {
        Ok(value)
    } else {
        Err(format!(
            "invalid {field} {value:?}: expected one of {}",
            allowed.join(", ")
        ))
    }
}

/// Validate a free-text search query. It is sent as a URL-encoded query parameter, so it
/// cannot alter the request path; this only bounds its size and rejects control characters.
pub fn query(raw: &str) -> Result<&str, String> {
    let n = raw.chars().count();
    if n == 0 || n > MAX_QUERY_CHARS {
        return Err(format!(
            "invalid query: must be 1-{MAX_QUERY_CHARS} characters"
        ));
    }
    if raw.chars().any(char::is_control) {
        return Err("invalid query: control characters are not allowed".into());
    }
    Ok(raw)
}

/// True for characters that render as nothing (or reorder text) and are used to smuggle
/// hidden instructions: zero-width and joiner characters, bidi controls, invisible math
/// operators, the BOM, interlinear annotation, Unicode tag characters and variation
/// selectors (the "emoji smuggling" carrier).
fn is_invisible(c: char) -> bool {
    matches!(c,
        '\u{00AD}'                    // soft hyphen
        | '\u{034F}'                  // combining grapheme joiner
        | '\u{061C}'                  // Arabic letter mark
        | '\u{115F}' | '\u{1160}'     // Hangul fillers
        | '\u{17B4}' | '\u{17B5}'     // Khmer inherent vowels
        | '\u{180B}'..='\u{180F}'     // Mongolian variation selectors / vowel separator
        | '\u{200B}'..='\u{200F}'     // zero-width space/joiners, LRM/RLM
        | '\u{202A}'..='\u{202E}'     // bidi embeddings / overrides
        | '\u{2060}'..='\u{206F}'     // word joiner, invisible operators, bidi isolates
        | '\u{3164}'                  // Hangul filler
        | '\u{FE00}'..='\u{FE0F}'     // variation selectors
        | '\u{FEFF}'                  // BOM / zero-width no-break space
        | '\u{FFA0}'                  // halfwidth Hangul filler
        | '\u{FFF9}'..='\u{FFFB}'     // interlinear annotation
        | '\u{1D173}'..='\u{1D17A}'   // musical symbol format controls
        | '\u{E0000}'..='\u{E007F}'   // tag characters
        | '\u{E0100}'..='\u{E01EF}' // variation selectors supplement
    )
}

/// Strip invisible and control characters from untrusted Reddit text. Newlines and tabs are
/// kept; `\r` is dropped and the Unicode line/paragraph separators become `\n`.
pub fn sanitize(s: &str) -> String {
    s.chars()
        .filter_map(|c| match c {
            '\n' | '\t' => Some(c),
            '\u{2028}' | '\u{2029}' => Some('\n'),
            c if c.is_control() || is_invisible(c) => None,
            c => Some(c),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subreddit_accepts_names_and_prefixes() {
        assert_eq!(subreddit("rust"), Ok("rust"));
        assert_eq!(subreddit("r/rust"), Ok("rust"));
        assert_eq!(subreddit("/r/Rust_Lang"), Ok("Rust_Lang"));
        assert_eq!(subreddit(" all "), Ok("all"));
    }

    #[test]
    fn subreddit_rejects_path_and_query_injection() {
        for bad in [
            "",
            "a",
            "rust/../../api/v1/me",
            "../message/inbox",
            "rust?x=1",
            "rust#frag",
            "rust%2F..",
            "rust/hot",
            "a+b",
            "abcdefghijklmnopqrstuv", // 22 chars
            "rüst",
        ] {
            assert!(subreddit(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn post_id_accepts_base36_and_fullname() {
        assert_eq!(post_id("1abcdef"), Ok("1abcdef"));
        assert_eq!(post_id("t3_1abcdef"), Ok("1abcdef"));
    }

    #[test]
    fn post_id_rejects_injection() {
        for bad in [
            "",
            "t3_",
            "1ABC",
            "1abc/../../api",
            "1abc?limit=1",
            "../x",
            "t1_abc",
        ] {
            assert!(post_id(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn choice_enforces_allow_list() {
        assert_eq!(choice("sort", "top", LISTING_SORTS), Ok("top"));
        assert!(choice("sort", "top/../../x", LISTING_SORTS).is_err());
        assert!(choice("sort", "relevance", LISTING_SORTS).is_err());
        assert!(choice("time", "forever", TIME_FILTERS).is_err());
    }

    #[test]
    fn query_bounds_length_and_controls() {
        assert_eq!(query("rust async"), Ok("rust async"));
        assert!(query("").is_err());
        assert!(query(&"x".repeat(MAX_QUERY_CHARS + 1)).is_err());
        assert!(query("a\u{0000}b").is_err());
        assert!(query("a\nb").is_err());
    }

    #[test]
    fn sanitize_strips_hidden_text_but_keeps_visible_text() {
        assert_eq!(sanitize("plain text\nline 2\tx"), "plain text\nline 2\tx");
        assert_eq!(sanitize("a\u{200B}b\u{202E}c\u{FEFF}d"), "abcd");
        // Unicode tag characters spelling "hi" are removed entirely.
        assert_eq!(sanitize("ok\u{E0068}\u{E0069}"), "ok");
        // Variation-selector smuggling.
        assert_eq!(sanitize("x\u{E0100}\u{FE01}y"), "xy");
        assert_eq!(sanitize("a\r\nb\u{0007}\u{0085}c\u{2028}d"), "a\nbc\nd");
        assert_eq!(sanitize("héllo 世界 🦀"), "héllo 世界 🦀");
    }
}
