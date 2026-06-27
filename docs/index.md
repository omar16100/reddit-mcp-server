# Documentation index — reddit-mcp-server

Read-only Reddit MCP server (Rust, stdio) giving Claude Code first-class Reddit read tools
via app-only OAuth (`client_credentials`). No posting/commenting/voting.

## Conventions
- Dated docs: `DDMMYYYY_topic.md`. Evergreen: `topic.md`.
- `c4model.md` is the architecture source of truth — update it for any architecture change.

## Index
| Doc | Type | Description |
|---|---|---|
| [c4model.md](c4model.md) | evergreen | C4 context/container/component for the server and its data flow to the Reddit API. |

## Quick reference
- Build: `cargo build --release` → `target/release/reddit-mcp-server`.
- Test: `cargo test` (units + MCP handshake); `cargo test --test live -- --ignored` (real Reddit, needs creds).
- Register: `claude mcp add -s user reddit -e REDDIT_CLIENT_ID=… -e REDDIT_CLIENT_SECRET=… -e "REDDIT_USER_AGENT=…" -- /Users/macmini/projects/reddit-mcp-server/target/release/reddit-mcp-server`
- Env: `REDDIT_CLIENT_ID`, `REDDIT_CLIENT_SECRET` (required), `REDDIT_USER_AGENT` (optional).
- Tools: `reddit_listing`, `reddit_search`, `reddit_comments`, `reddit_subreddit_about`.
