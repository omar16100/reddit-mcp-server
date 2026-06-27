//! Tool parameter structs, compact output structs, and Reddit-JSON → compact mappers.
//!
//! Why compact: MCP tool results are injected into the model's context. Raw Reddit
//! listing/comments JSON is huge, so every tool returns a trimmed shape (selftext/body
//! truncated, only the fields an agent needs).

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const REDDIT: &str = "https://www.reddit.com";
const SELFTEXT_MAX: usize = 600;
const COMMENT_BODY_MAX: usize = 500;

// ── defaults for serde-defaulted params ─────────────────────────────────────────
fn default_hot() -> String { "hot".into() }
fn default_relevance() -> String { "relevance".into() }
fn default_month() -> String { "month".into() }
fn default_limit() -> u32 { 25 }
fn default_comment_limit() -> u32 { 50 }
fn default_true() -> bool { true }

fn truncate(s: &str, n: usize) -> String {
    let mut chars = s.chars();
    let head: String = chars.by_ref().take(n).collect();
    if chars.next().is_some() { format!("{head}…") } else { head }
}

// ── tool parameters (Deserialize + JsonSchema for the input schema) ─────────────
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListingParams {
    /// Subreddit name, with or without "r/" (e.g. "rust" or "r/rust").
    pub subreddit: String,
    /// Sort: hot | new | top | rising | controversial. Default: hot.
    #[serde(default = "default_hot")]
    pub sort: String,
    /// Max posts to return, 1-100. Default: 25.
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Only for top/controversial: hour | day | week | month | year | all. Default: month.
    #[serde(default = "default_month")]
    pub time_filter: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchParams {
    /// Full-text search query.
    pub query: String,
    /// Restrict to this subreddit (with or without "r/"). Omit for a site-wide search.
    #[serde(default)]
    pub subreddit: Option<String>,
    /// Sort: relevance | new | top | comments | hot. Default: relevance.
    #[serde(default = "default_relevance")]
    pub sort: String,
    /// Time window: hour | day | week | month | year | all. Default: month.
    #[serde(default = "default_month")]
    pub time: String,
    /// Max results, 1-100. Default: 25.
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// When a subreddit is given, restrict results to it only. Default: true.
    #[serde(default = "default_true")]
    pub restrict_sr: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CommentsParams {
    /// Post id (base36, e.g. "1abcdef"; a "t3_" prefix is also accepted).
    pub post_id: String,
    /// Optional subreddit (with or without "r/").
    #[serde(default)]
    pub subreddit: Option<String>,
    /// Max comments to return (thread is flattened, top-first). Default: 50.
    #[serde(default = "default_comment_limit")]
    pub comment_limit: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AboutParams {
    /// Subreddit name, with or without "r/".
    pub subreddit: String,
}

// ── compact outputs (Serialize + JsonSchema for the output schema) ──────────────
#[derive(Debug, Serialize, JsonSchema)]
pub struct PostSummary {
    pub id: String,
    pub fullname: String,
    pub title: String,
    pub author: String,
    pub subreddit: String,
    pub score: i64,
    pub num_comments: i64,
    pub created_utc: i64,
    pub permalink: String,
    pub url: String,
    pub flair: Option<String>,
    pub selftext: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommentSummary {
    pub author: String,
    pub score: i64,
    pub depth: i64,
    pub body: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommentsResult {
    pub post: Option<PostSummary>,
    pub comments: Vec<CommentSummary>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SubredditInfo {
    pub display_name: String,
    pub title: String,
    pub subscribers: i64,
    pub active_user_count: i64,
    pub public_description: String,
    pub over18: bool,
}

/// Object wrapper for list results — MCP requires tool outputSchema to have an object root
/// (a bare array root is rejected), so list-returning tools wrap their results here.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PostList {
    pub posts: Vec<PostSummary>,
}

/// Object wrapper for the about result (object root; `subreddit` is null if not found).
#[derive(Debug, Serialize, JsonSchema)]
pub struct AboutResult {
    pub subreddit: Option<SubredditInfo>,
}

// ── helpers ─────────────────────────────────────────────────────────────────────
fn str_field(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}
fn int_field(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64))).unwrap_or(0)
}

// ── mappers (raw Reddit JSON → compact) ─────────────────────────────────────────
pub fn post_summary(d: &Value) -> PostSummary {
    PostSummary {
        id: str_field(d, "id"),
        fullname: str_field(d, "name"),
        title: str_field(d, "title"),
        author: str_field(d, "author"),
        subreddit: str_field(d, "subreddit"),
        score: int_field(d, "score"),
        num_comments: int_field(d, "num_comments"),
        created_utc: int_field(d, "created_utc"),
        permalink: format!("{REDDIT}{}", str_field(d, "permalink")),
        url: str_field(d, "url"),
        flair: d.get("link_flair_text").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string),
        selftext: truncate(&str_field(d, "selftext"), SELFTEXT_MAX),
    }
}

/// A Listing response: `{ "data": { "children": [ { "data": {...} }, ... ] } }`.
pub fn summarize_listing(v: Option<&Value>) -> Vec<PostSummary> {
    let mut out = Vec::new();
    if let Some(children) = v.and_then(|v| v.pointer("/data/children")).and_then(Value::as_array) {
        for c in children {
            if let Some(d) = c.get("data") {
                out.push(post_summary(d));
            }
        }
    }
    out
}

/// An about response: `{ "kind": "t5", "data": {...} }`.
pub fn summarize_about(v: Option<&Value>) -> Option<SubredditInfo> {
    let d = v?.get("data")?;
    Some(SubredditInfo {
        display_name: str_field(d, "display_name"),
        title: str_field(d, "title"),
        subscribers: int_field(d, "subscribers"),
        active_user_count: int_field(d, "active_user_count"),
        public_description: str_field(d, "public_description"),
        over18: d.get("over18").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// A comments response: `[ <post listing>, <comments listing> ]`.
pub fn summarize_comments(v: Option<&Value>, limit: usize) -> CommentsResult {
    let mut post = None;
    let mut comments = Vec::new();
    if let Some(arr) = v.and_then(Value::as_array) {
        if let Some(p) = arr.first().and_then(|x| x.pointer("/data/children/0/data")) {
            post = Some(post_summary(p));
        }
        if let Some(children) = arr.get(1).and_then(|x| x.pointer("/data/children")).and_then(Value::as_array) {
            collect_comments(children, &mut comments, limit);
        }
    }
    CommentsResult { post, comments }
}

fn collect_comments(children: &[Value], out: &mut Vec<CommentSummary>, limit: usize) {
    for c in children {
        if out.len() >= limit {
            return;
        }
        if c.get("kind").and_then(Value::as_str) != Some("t1") {
            continue; // skip "more" placeholders
        }
        let Some(d) = c.get("data") else { continue };
        out.push(CommentSummary {
            author: str_field(d, "author"),
            score: int_field(d, "score"),
            depth: int_field(d, "depth"),
            body: truncate(&str_field(d, "body"), COMMENT_BODY_MAX),
        });
        if let Some(replies) = d.get("replies").and_then(|r| r.pointer("/data/children")).and_then(Value::as_array) {
            collect_comments(replies, out, limit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn truncate_adds_ellipsis_only_when_cut() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello", 3), "hel…");
    }

    #[test]
    fn listing_maps_children_to_summaries() {
        let v = json!({"data":{"children":[
            {"kind":"t3","data":{"id":"a1","name":"t3_a1","title":"Async in Rust","author":"u1",
                "subreddit":"rust","score":42,"num_comments":7,"created_utc":1_700_000_000.0,
                "permalink":"/r/rust/comments/a1/x/","url":"https://e.com","link_flair_text":"News",
                "selftext":"body"}}
        ]}});
        let posts = summarize_listing(Some(&v));
        assert_eq!(posts.len(), 1);
        let p = &posts[0];
        assert_eq!(p.id, "a1");
        assert_eq!(p.score, 42);
        assert_eq!(p.permalink, "https://www.reddit.com/r/rust/comments/a1/x/");
        assert_eq!(p.flair.as_deref(), Some("News"));
        assert_eq!(p.created_utc, 1_700_000_000); // float coerced to int
    }

    #[test]
    fn about_maps_data() {
        let v = json!({"kind":"t5","data":{"display_name":"rust","title":"The Rust Programming Language",
            "subscribers":300000,"active_user_count":1234,"public_description":"All things Rust","over18":false}});
        let info = summarize_about(Some(&v)).expect("some");
        assert_eq!(info.display_name, "rust");
        assert_eq!(info.subscribers, 300000);
        assert!(!info.over18);
    }

    #[test]
    fn comments_extracts_post_and_flattens_replies_with_limit() {
        let v = json!([
            {"data":{"children":[{"kind":"t3","data":{"id":"p1","name":"t3_p1","title":"T","author":"op",
                "subreddit":"rust","score":5,"num_comments":3,"created_utc":1,"permalink":"/r/rust/comments/p1/","url":"u","selftext":"s"}}]}},
            {"data":{"children":[
                {"kind":"t1","data":{"author":"c1","score":10,"depth":0,"body":"top",
                    "replies":{"data":{"children":[{"kind":"t1","data":{"author":"c2","score":2,"depth":1,"body":"reply"}}]}}}},
                {"kind":"more","data":{}}
            ]}}
        ]);
        let res = summarize_comments(Some(&v), 50);
        assert_eq!(res.post.as_ref().unwrap().id, "p1");
        assert_eq!(res.comments.len(), 2); // top + nested reply, "more" skipped
        assert_eq!(res.comments[0].author, "c1");
        assert_eq!(res.comments[1].depth, 1);

        let limited = summarize_comments(Some(&v), 1);
        assert_eq!(limited.comments.len(), 1);
    }
}
