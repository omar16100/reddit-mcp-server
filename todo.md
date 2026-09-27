# todo: reddit-mcp-server

Read-only Reddit MCP server (Rust) for Claude Code. Original build plan was a local, unpublished note.

## 2026-06-26: initial build (TDD)
- [x] Cargo crate; deps: rmcp 1.8 (features: server, transport-io), reqwest 0.13 (json, query, default TLS), tokio, serde, serde_json, schemars, anyhow, base64, tracing.
- [x] `src/reddit.rs`: app-only `client_credentials` OAuth, in-memory token cache, 4 read methods; logic factored into pure request-builders. Tests written first (red→green).
- [x] `src/model.rs`: tool params + compact output structs (PostSummary/CommentSummary/CommentsResult/SubredditInfo + object wrappers PostList/AboutResult) + raw-JSON→compact mappers, with tests.
- [x] `src/server.rs`: 4 rmcp tools (reddit_listing / reddit_search / reddit_comments / reddit_subreddit_about), read-only; `get_info` states no-posting.
- [x] `src/main.rs`: stdio transport, tracing→stderr (stdout reserved for JSON-RPC).
- [x] Tests: 13 unit + `tests/handshake.rs` (MCP initialize→tools/list over stdio, asserts 4 tools + clean stdout) + `tests/live.rs` (#[ignore], real Reddit client) + `tests/call.rs` (#[ignore], full real `tools/call` over stdio: dispatch seam, readable text+structuredContent, serde-default args). All green. `cargo build --release` clean.
- [x] Verified all 4 tools via real `tools/call` over stdio on live Reddit (about/search/listing/comments incl. nested-reply flattening). Results carry both a text block and structuredContent; isError=false.
- [x] Registered globally: `claude mcp add -s user reddit -- target/release/reddit-mcp-server` (creds via `-e`). `claude mcp list` → connected.
- [x] Pre-allowed `mcp__reddit__*` tools in `~/.claude/settings.json`.

## 2026-09-27: rmcp 2.1 security update
Plan: [docs/27092026_rmcp2_security_deps_plan.md](docs/27092026_rmcp2_security_deps_plan.md).
- [x] rmcp 1.8.0 -> 2.1.0 (rmcp-macros 2.2.0). Closes Dependabot alerts GHSA-33f5-2c5q-wgwj, GHSA-9pj6-vhgr-3mwh, GHSA-9g45-5xwm-f3wc. None was reachable here (they sit in rmcp's OAuth client and Streamable HTTP transports; this server explicitly requests only `server` + `transport-io`; no `auth` or Streamable HTTP code is compiled in), but the bump clears them.
- [x] Lockfile-only: h2 0.4.15 -> 0.4.19 (RUSTSEC-2026-0258) and rustls 0.23.41 -> 0.23.45 (RUSTSEC-2026-0285), both reached via reqwest. Pulled aws-lc-rs 1.18.1, aws-lc-sys 0.45.0, rustls-webpki 0.103.15. `cargo audit` clean.
- [x] No source change needed. fmt, clippy `-D warnings`, 13 unit + handshake test green.
- [x] Offline stdio smoke (network denied via macOS `sandbox-exec`): tools/list output identical to rmcp 1.8 for protocol versions 2024-11-05, 2025-06-18, 2025-11-25; an unknown version still negotiates 2025-11-25. tools/call error paths identical.
- [x] rmcp 2.x protocol-edge deltas (details in the plan doc): no reply to non-JSON lines (was `-32700`), `-32600` for non-JSON-RPC objects, repeat `initialize` honours the requested version, malformed `task.ttl` gives `-32601` (was `-32602`), cancel-safe stdio reads. No expected impact on compliant clients.
- [ ] Live Reddit `tools/call` (`cargo test --test call -- --ignored`) not re-run for this bump (needs creds + network). Run it once when convenient and rebuild the installed release binary.
- [ ] `serverInfo` reports `{"name":"rmcp","version":"<rmcp version>"}` (inherited `ServerInfo::default()`), not this crate's name/version. Pre-existing; optional fix: set `info.server_info` from `CARGO_PKG_NAME`/`CARGO_PKG_VERSION`.

## Output-schema gotcha (resolved)
- MCP requires tool `outputSchema` root type = object. `Json<Vec<_>>` (array root) panics at startup → wrapped lists in `PostList { count, posts }` and about in `AboutResult { subreddit }`.

## Next / optional
- [ ] Rebuild the release binary after any source change (Claude Code launches the compiled binary, not `cargo run`): `cargo build --release`.
- [ ] Possible tools: reddit_user_about / reddit_post (by id). Posting tools are intentionally OUT (read-only app creds).
- [ ] Consider committing (git repo initialized by `cargo init`); not committed yet.
