# todo — reddit-mcp-server

Read-only Reddit MCP server (Rust) for Claude Code. Plan: `/Users/macmini/.claude/plans/check-which-project-can-floating-lampson.md`.

## 2026-06-26 — initial build (TDD)
- [x] Cargo crate; deps: rmcp 1.8 (features: server, transport-io), reqwest 0.13 (json, query, default TLS), tokio, serde, serde_json, schemars, anyhow, base64, tracing.
- [x] `src/reddit.rs` — app-only `client_credentials` OAuth, in-memory token cache, 4 read methods; logic factored into pure request-builders. Tests written first (red→green).
- [x] `src/model.rs` — tool params + compact output structs (PostSummary/CommentSummary/CommentsResult/SubredditInfo + object wrappers PostList/AboutResult) + raw-JSON→compact mappers, with tests.
- [x] `src/server.rs` — 4 rmcp tools (reddit_listing / reddit_search / reddit_comments / reddit_subreddit_about), read-only; `get_info` states no-posting.
- [x] `src/main.rs` — stdio transport, tracing→stderr (stdout reserved for JSON-RPC).
- [x] Tests: 13 unit + `tests/handshake.rs` (MCP initialize→tools/list over stdio, asserts 4 tools + clean stdout) + `tests/live.rs` (#[ignore], real Reddit client) + `tests/call.rs` (#[ignore], full real `tools/call` over stdio: dispatch seam, readable text+structuredContent, serde-default args). All green. `cargo build --release` clean.
- [x] Verified all 4 tools via real `tools/call` over stdio on live Reddit (about/search/listing/comments incl. nested-reply flattening). Results carry both a text block and structuredContent; isError=false.
- [x] Registered globally: `claude mcp add -s user reddit -- target/release/reddit-mcp-server` (creds via `-e`). `claude mcp list` → connected.
- [x] Pre-allowed `mcp__reddit__*` tools in `/Users/macmini/.claude/settings.json`.

## Output-schema gotcha (resolved)
- MCP requires tool `outputSchema` root type = object. `Json<Vec<_>>` (array root) panics at startup → wrapped lists in `PostList { count, posts }` and about in `AboutResult { subreddit }`.

## Next / optional
- [ ] Rebuild the release binary after any source change (Claude Code launches the compiled binary, not `cargo run`): `cargo build --release`.
- [ ] Possible tools: reddit_user_about / reddit_post (by id). Posting tools are intentionally OUT (read-only app creds).
- [ ] Consider committing (git repo initialized by `cargo init`); not committed yet.
