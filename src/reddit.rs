//! Read-only Reddit HTTP client: app-only OAuth (client_credentials), in-memory token
//! cache, and the four read endpoints. The request-construction and auth/cache decisions
//! are factored into pure functions so they can be unit-tested without network (TDD).

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const TOKEN_URL: &str = "https://www.reddit.com/api/v1/access_token";
const API_BASE: &str = "https://oauth.reddit.com";
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);
const TOKEN_SKEW: Duration = Duration::from_secs(60); // refresh this long before expiry

type Query = Vec<(&'static str, String)>;

#[derive(Debug)]
pub enum RedditError {
    RateLimited { reset_secs: u64 },
    Http(String),
}

impl std::fmt::Display for RedditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RedditError::RateLimited { reset_secs } => write!(f, "rate limited; retry in ~{reset_secs}s"),
            RedditError::Http(m) => write!(f, "{m}"),
        }
    }
}

struct CachedToken {
    token: String,
    expiry: Instant,
}

// ── pure helpers (unit-tested; no network) ──────────────────────────────────────

/// Strip a leading `r/` if present.
pub fn norm_sub(sub: &str) -> &str {
    sub.strip_prefix("r/").unwrap_or(sub)
}

/// HTTP Basic credential value (without the "Basic " prefix).
fn basic_auth(client_id: &str, client_secret: &str) -> String {
    STANDARD.encode(format!("{client_id}:{client_secret}"))
}

/// Return the cached token if it is still valid at `now` (accounting for skew).
fn valid_cached(entry: &Option<CachedToken>, now: Instant) -> Option<String> {
    match entry {
        Some(c) if now < c.expiry => Some(c.token.clone()),
        _ => None,
    }
}

/// Build the (path, query) for a subreddit listing. `t` is only sent for time-windowed sorts.
pub fn listing_request(sub: &str, sort: &str, limit: u32, time_filter: &str) -> (String, Query) {
    let mut q: Query = vec![("limit", limit.to_string()), ("raw_json", "1".into())];
    if matches!(sort, "top" | "controversial") {
        q.push(("t", time_filter.to_string()));
    }
    (format!("/r/{}/{}", norm_sub(sub), sort), q)
}

/// Build the (path, query) for search (subreddit-scoped or site-wide).
pub fn search_request(query: &str, sub: Option<&str>, sort: &str, time: &str, limit: u32, restrict_sr: bool) -> (String, Query) {
    let mut q: Query = vec![
        ("q", query.to_string()),
        ("sort", sort.to_string()),
        ("t", time.to_string()),
        ("limit", limit.to_string()),
        ("raw_json", "1".into()),
    ];
    match sub {
        Some(s) => {
            q.push(("restrict_sr", if restrict_sr { "1" } else { "0" }.into()));
            (format!("/r/{}/search", norm_sub(s)), q)
        }
        None => ("/search".to_string(), q),
    }
}

/// Build the (path, query) for a post's comment thread. Accepts a bare id or `t3_` fullname.
pub fn comments_request(post_id: &str, sub: Option<&str>, limit: u32) -> (String, Query) {
    let pid = post_id.strip_prefix("t3_").unwrap_or(post_id);
    let q: Query = vec![("limit", limit.to_string()), ("raw_json", "1".into())];
    let path = match sub {
        Some(s) => format!("/r/{}/comments/{pid}", norm_sub(s)),
        None => format!("/comments/{pid}"),
    };
    (path, q)
}

/// Build the (path, query) for subreddit "about".
pub fn about_request(sub: &str) -> (String, Query) {
    (format!("/r/{}/about", norm_sub(sub)), vec![("raw_json", "1".into())])
}

// ── client ──────────────────────────────────────────────────────────────────────

pub struct RedditClient {
    http: Client,
    client_id: String,
    client_secret: String,
    token: Mutex<Option<CachedToken>>,
}

impl RedditClient {
    pub fn from_env() -> Result<Self> {
        let client_id = std::env::var("REDDIT_CLIENT_ID").map_err(|_| anyhow!("REDDIT_CLIENT_ID not set"))?;
        let client_secret = std::env::var("REDDIT_CLIENT_SECRET").map_err(|_| anyhow!("REDDIT_CLIENT_SECRET not set"))?;
        let user_agent = std::env::var("REDDIT_USER_AGENT").unwrap_or_else(|_| "rust:reddit-mcp:0.1 (read-only)".into());
        let http = Client::builder().user_agent(user_agent).timeout(HTTP_TIMEOUT).build()?;
        Ok(Self { http, client_id, client_secret, token: Mutex::new(None) })
    }

    async fn token(&self) -> Result<String, RedditError> {
        if let Some(tok) = valid_cached(&*self.token.lock().await, Instant::now()) {
            return Ok(tok);
        }
        self.refresh_token().await
    }

    async fn refresh_token(&self) -> Result<String, RedditError> {
        let resp = self
            .http
            .post(TOKEN_URL)
            .header("Authorization", format!("Basic {}", basic_auth(&self.client_id, &self.client_secret)))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body("grant_type=client_credentials")
            .send()
            .await
            .map_err(|e| RedditError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(RedditError::Http(format!("oauth token failed: HTTP {code}: {}", body.chars().take(200).collect::<String>())));
        }
        let v: Value = resp.json().await.map_err(|e| RedditError::Http(e.to_string()))?;
        let token = v["access_token"].as_str().ok_or_else(|| RedditError::Http("no access_token in response".into()))?.to_string();
        let expires_in = v["expires_in"].as_u64().unwrap_or(3600);
        let expiry = Instant::now() + Duration::from_secs(expires_in).saturating_sub(TOKEN_SKEW);
        *self.token.lock().await = Some(CachedToken { token: token.clone(), expiry });
        tracing::info!(expires_in, "fetched new app-only Reddit token");
        Ok(token)
    }

    /// Authenticated GET against oauth.reddit.com. 401 → refresh once; 404 → Ok(None);
    /// 429 → RateLimited; other non-2xx → Http error.
    async fn get(&self, path: &str, query: &Query) -> Result<Option<Value>, RedditError> {
        for attempt in 0..2 {
            let token = self.token().await?;
            let resp = self
                .http
                .get(format!("{API_BASE}{path}"))
                .header("Authorization", format!("Bearer {token}"))
                .query(query)
                .send()
                .await
                .map_err(|e| RedditError::Http(e.to_string()))?;
            match resp.status() {
                s if s.is_success() => {
                    let v = resp.json::<Value>().await.map_err(|e| RedditError::Http(e.to_string()))?;
                    return Ok(Some(v));
                }
                StatusCode::UNAUTHORIZED if attempt == 0 => {
                    *self.token.lock().await = None; // force refresh, retry once
                    continue;
                }
                StatusCode::NOT_FOUND => return Ok(None),
                StatusCode::TOO_MANY_REQUESTS => {
                    let reset = resp
                        .headers()
                        .get("x-ratelimit-reset")
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(60.0) as u64;
                    return Err(RedditError::RateLimited { reset_secs: reset });
                }
                s => {
                    let body = resp.text().await.unwrap_or_default();
                    return Err(RedditError::Http(format!("HTTP {s}: {}", body.chars().take(200).collect::<String>())));
                }
            }
        }
        Err(RedditError::Http("token refresh retry exhausted".into()))
    }

    pub async fn get_listing(&self, sub: &str, sort: &str, limit: u32, time_filter: &str) -> Result<Option<Value>, RedditError> {
        let (path, q) = listing_request(sub, sort, limit, time_filter);
        self.get(&path, &q).await
    }

    pub async fn search(&self, query: &str, sub: Option<&str>, sort: &str, time: &str, limit: u32, restrict_sr: bool) -> Result<Option<Value>, RedditError> {
        let (path, q) = search_request(query, sub, sort, time, limit, restrict_sr);
        self.get(&path, &q).await
    }

    pub async fn get_comments(&self, post_id: &str, sub: Option<&str>, limit: u32) -> Result<Option<Value>, RedditError> {
        let (path, q) = comments_request(post_id, sub, limit);
        self.get(&path, &q).await
    }

    pub async fn get_subreddit_about(&self, sub: &str) -> Result<Option<Value>, RedditError> {
        let (path, q) = about_request(sub);
        self.get(&path, &q).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(q: &Query, k: &str, v: &str) -> bool {
        q.iter().any(|(qk, qv)| *qk == k && qv == v)
    }
    fn key(q: &Query, k: &str) -> bool {
        q.iter().any(|(qk, _)| *qk == k)
    }

    #[test]
    fn norm_sub_strips_prefix() {
        assert_eq!(norm_sub("r/rust"), "rust");
        assert_eq!(norm_sub("rust"), "rust");
    }

    #[test]
    fn basic_auth_base64() {
        assert_eq!(basic_auth("a", "b"), "YTpi"); // base64("a:b")
    }

    #[test]
    fn cache_validity_honors_expiry() {
        let now = Instant::now();
        assert_eq!(valid_cached(&None, now), None);
        let future = Some(CachedToken { token: "T".into(), expiry: now + Duration::from_secs(10) });
        assert_eq!(valid_cached(&future, now), Some("T".to_string()));
        let past = Some(CachedToken { token: "T".into(), expiry: now - Duration::from_secs(1) });
        assert_eq!(valid_cached(&past, now), None);
    }

    #[test]
    fn listing_hot_has_no_time_filter() {
        let (path, q) = listing_request("r/rust", "hot", 5, "month");
        assert_eq!(path, "/r/rust/hot"); // prefix stripped
        assert!(has(&q, "limit", "5"));
        assert!(!key(&q, "t"));
    }

    #[test]
    fn listing_top_includes_time_filter() {
        let (path, q) = listing_request("rust", "top", 10, "week");
        assert_eq!(path, "/r/rust/top");
        assert!(has(&q, "t", "week"));
    }

    #[test]
    fn search_scoped_sets_restrict_sr() {
        let (path, q) = search_request("async", Some("rust"), "top", "year", 15, true);
        assert_eq!(path, "/r/rust/search");
        assert!(has(&q, "q", "async"));
        assert!(has(&q, "restrict_sr", "1"));
        assert!(has(&q, "t", "year"));
    }

    #[test]
    fn search_global_has_no_restrict_sr() {
        let (path, q) = search_request("gpu", None, "relevance", "month", 25, true);
        assert_eq!(path, "/search");
        assert!(!key(&q, "restrict_sr"));
    }

    #[test]
    fn comments_strips_t3_and_scopes_sub() {
        let (path, _) = comments_request("t3_1abc", Some("r/rust"), 50);
        assert_eq!(path, "/r/rust/comments/1abc");
        let (path2, _) = comments_request("1abc", None, 50);
        assert_eq!(path2, "/comments/1abc");
    }

    #[test]
    fn about_path() {
        let (path, _) = about_request("r/rust");
        assert_eq!(path, "/r/rust/about");
    }
}
