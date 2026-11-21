//! Integration test: injection-shaped tool arguments are rejected with JSON-RPC
//! `invalid_params` (-32602) *before* any network request. Dummy creds and no network are
//! enough, because a request that got past validation would fail on the OAuth fetch with a
//! different error. Also checks `serverInfo` identifies this crate, not rmcp.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn call(id: u32, tool: &str, args: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{tool}","arguments":{args}}}}}"#
    )
}

#[test]
fn injection_shaped_arguments_are_rejected_before_any_request() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_reddit-mcp-server"))
        .env("REDDIT_CLIENT_ID", "dummy")
        .env("REDDIT_CLIENT_SECRET", "dummy")
        .env("REDDIT_USER_AGENT", "validation-test/0.1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn server binary");

    let cases = [
        ("reddit_listing", r#"{"subreddit":"../../api/v1/me?x="}"#),
        (
            "reddit_listing",
            r#"{"subreddit":"rust","sort":"hot/../../message/inbox"}"#,
        ),
        (
            "reddit_listing",
            r#"{"subreddit":"rust","sort":"top","time_filter":"all&x=1"}"#,
        ),
        ("reddit_search", r#"{"query":"x","sort":"bogus"}"#),
        ("reddit_search", r#"{"query":"x","subreddit":"a/b"}"#),
        ("reddit_comments", r#"{"post_id":"1abc/../../api/v1/me"}"#),
        (
            "reddit_comments",
            r#"{"post_id":"1abc","subreddit":"rust?x="}"#,
        ),
        (
            "reddit_subreddit_about",
            r#"{"subreddit":"rust/../../prefs"}"#,
        ),
    ];

    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":0,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"test","version":"0"}}}}}}"#).unwrap();
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
    )
    .unwrap();
    for (i, (tool, args)) in cases.iter().enumerate() {
        writeln!(stdin, "{}", call(i as u32 + 1, tool, args)).unwrap();
    }
    stdin.flush().unwrap();

    let stdout = child.stdout.take().unwrap();
    let mut server_name = None;
    let mut codes = vec![None; cases.len()];
    let mut seen = 0;
    for line in BufReader::new(stdout).lines() {
        let line = line.unwrap();
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("non-JSON on stdout: {e}: {line:?}"));
        let Some(id) = v.get("id").and_then(|i| i.as_u64()) else {
            continue;
        };
        if id == 0 {
            server_name = v
                .pointer("/result/serverInfo/name")
                .and_then(|n| n.as_str())
                .map(String::from);
            continue;
        }
        codes[id as usize - 1] = v.pointer("/error/code").and_then(|c| c.as_i64());
        seen += 1;
        if seen == cases.len() {
            break;
        }
    }
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();

    assert_eq!(server_name.as_deref(), Some("reddit-mcp-server"));
    for ((tool, args), code) in cases.iter().zip(&codes) {
        assert_eq!(
            *code,
            Some(-32602),
            "{tool} {args} should be rejected as invalid_params"
        );
    }
}
