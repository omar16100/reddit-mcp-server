//! Gated end-to-end test: real `tools/call` over stdio against live Reddit. Verifies the
//! full seam (Parameters deserialize → async dispatch → Json<T> → CallToolResult), that the
//! result carries a readable text block + structuredContent, and that serde defaults work
//! (search called with only `query`). Ignored by default:
//!   REDDIT_CLIENT_ID=… REDDIT_CLIENT_SECRET=… cargo test --test call -- --ignored

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
#[ignore = "hits real Reddit over the full MCP stdio path; requires REDDIT_CLIENT_ID/SECRET"]
fn tools_call_round_trip_returns_readable_content() {
    let id = std::env::var("REDDIT_CLIENT_ID").expect("REDDIT_CLIENT_ID required");
    let secret = std::env::var("REDDIT_CLIENT_SECRET").expect("REDDIT_CLIENT_SECRET required");

    let mut child = Command::new(env!("CARGO_BIN_EXE_reddit-mcp-server"))
        .env("REDDIT_CLIENT_ID", id)
        .env("REDDIT_CLIENT_SECRET", secret)
        .env("REDDIT_USER_AGENT", "reddit-mcp-call-test/0.1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn server");

    let mut stdin = child.stdin.take().unwrap();
    // about (id 3) exercises the dispatch seam; search-with-only-query (id 4) exercises serde defaults.
    let msgs = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"reddit_subreddit_about","arguments":{"subreddit":"rust"}}}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"reddit_search","arguments":{"query":"async"}}}"#,
    ];
    for m in msgs {
        writeln!(stdin, "{m}").unwrap();
    }
    stdin.flush().unwrap();

    let reader = BufReader::new(child.stdout.take().unwrap());
    let mut about_ok = false;
    let mut search_ok = false;
    for line in reader.lines() {
        let line = line.unwrap();
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(&line).expect("stdout must be JSON-RPC");
        match v.get("id").and_then(|i| i.as_i64()) {
            Some(3) => {
                let r = &v["result"];
                assert_eq!(r["isError"].as_bool(), Some(false), "about isError");
                // readable text block present (not just structuredContent)
                let has_text = r["content"]
                    .as_array()
                    .map(|a| a.iter().any(|c| c["type"] == "text"))
                    .unwrap_or(false);
                assert!(has_text, "about result needs a text content block");
                assert_eq!(
                    r.pointer("/structuredContent/subreddit/display_name")
                        .and_then(|x| x.as_str()),
                    Some("rust")
                );
                about_ok = true;
            }
            Some(4) => {
                let r = &v["result"];
                assert_eq!(
                    r["isError"].as_bool(),
                    Some(false),
                    "search isError (defaults must deserialize)"
                );
                let n = r
                    .pointer("/structuredContent/posts")
                    .and_then(|p| p.as_array())
                    .map(Vec::len)
                    .unwrap_or(0);
                assert!(
                    n > 0,
                    "search with only `query` should return posts; got {n}"
                );
                search_ok = true;
            }
            _ => {}
        }
        if about_ok && search_ok {
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        about_ok && search_ok,
        "missing responses: about_ok={about_ok} search_ok={search_ok}"
    );
}
