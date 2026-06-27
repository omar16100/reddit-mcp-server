//! Integration test: drive the built binary over stdio with a real MCP handshake and
//! assert it advertises the 4 read tools. Also asserts stdout carries ONLY JSON-RPC
//! (the cardinal stdio-MCP rule). No network: dummy creds suffice for initialize/tools-list.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn handshake_lists_the_four_read_tools_and_keeps_stdout_clean() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_reddit-mcp-server"))
        .env("REDDIT_CLIENT_ID", "dummy")
        .env("REDDIT_CLIENT_SECRET", "dummy")
        .env("REDDIT_USER_AGENT", "handshake-test/0.1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn server binary");

    let mut stdin = child.stdin.take().unwrap();
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;
    let initialized = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    let list = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;
    write!(stdin, "{init}\n{initialized}\n{list}\n").unwrap();
    stdin.flush().unwrap();
    drop(stdin); // close stdin → server drains buffered input then exits → stdout hits EOF

    let stdout = child.stdout.take().unwrap();
    let mut tool_names: Vec<String> = Vec::new();
    for line in BufReader::new(stdout).lines() {
        let line = line.unwrap();
        if line.trim().is_empty() {
            continue;
        }
        // Cardinal rule: every stdout line must be valid JSON-RPC, never stray text.
        let v: serde_json::Value = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("non-JSON on stdout: {e}: {line:?}"));
        if let Some(tools) = v.pointer("/result/tools").and_then(|t| t.as_array()) {
            tool_names = tools
                .iter()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect();
        }
    }
    let _ = child.wait();

    for expected in [
        "reddit_listing",
        "reddit_search",
        "reddit_comments",
        "reddit_subreddit_about",
    ] {
        assert!(
            tool_names.iter().any(|n| n == expected),
            "missing tool {expected}; got {tool_names:?}"
        );
    }
    assert_eq!(
        tool_names.len(),
        4,
        "expected exactly 4 tools, got {tool_names:?}"
    );
}
