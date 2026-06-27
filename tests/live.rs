//! Live smoke test against the real Reddit API (read-only). Ignored by default; run with:
//!   REDDIT_CLIENT_ID=... REDDIT_CLIENT_SECRET=... cargo test --test live -- --ignored
//! Proves the full path: client_credentials token → oauth.reddit.com fetch → JSON parse.

use reddit_mcp_server::reddit::RedditClient;

#[tokio::test]
#[ignore = "hits the real Reddit API; requires REDDIT_CLIENT_ID/SECRET in env"]
async fn live_about_listing_search() {
    let client = RedditClient::from_env().expect("REDDIT_CLIENT_ID/SECRET must be set");

    let about = client.get_subreddit_about("rust").await.expect("about request ok").expect("subreddit found");
    let subs = about.pointer("/data/subscribers").and_then(|v| v.as_i64()).unwrap_or(0);
    assert!(subs > 0, "expected r/rust subscribers > 0, got {subs}");

    let listing = client.get_listing("rust", "hot", 3, "month").await.expect("listing ok").expect("some");
    let n = listing.pointer("/data/children").and_then(|v| v.as_array()).map(Vec::len).unwrap_or(0);
    assert!(n > 0, "expected some hot posts, got {n}");

    let search = client.search("async", Some("rust"), "top", "year", 3, true).await.expect("search ok").expect("some");
    let m = search.pointer("/data/children").and_then(|v| v.as_array()).map(Vec::len).unwrap_or(0);
    assert!(m > 0, "expected some search results, got {m}");
}
