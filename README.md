# reddit-mcp-server

A read-only Reddit [MCP](https://modelcontextprotocol.io) server in Rust. It gives MCP
clients (such as Claude Code) first-class Reddit **read** tools via Reddit's app-only OAuth
(`client_credentials`). It cannot post, comment, or vote by design.

Outputs are compact (trimmed selftext/comment bodies, only the useful fields) so results
stay friendly to an LLM's context window.

## Tools

| Tool | Description |
|---|---|
| `reddit_listing` | Posts from a subreddit (`hot`/`new`/`top`/`rising`/`controversial`). |
| `reddit_search` | Search posts, optionally restricted to a subreddit. |
| `reddit_comments` | A post plus its comment thread (flattened, top-first). |
| `reddit_subreddit_about` | Subreddit info (subscribers, description, NSFW flag). |

## Requirements

- Rust (stable) and Cargo.
- Reddit API credentials: create an app at <https://www.reddit.com/prefs/apps> (a "script"
  app works) to get a `client_id` and `client_secret`. Read-only app-only access needs only
  those two values, no Reddit username or password.

## Build

```sh
cargo build --release
# binary: target/release/reddit-mcp-server
```

## Use with Claude Code

```sh
claude mcp add -s user reddit \
  -e REDDIT_CLIENT_ID=your_client_id \
  -e REDDIT_CLIENT_SECRET=your_client_secret \
  -e "REDDIT_USER_AGENT=script:reddit-mcp:1.0 (by /u/your_username)" \
  -- /absolute/path/to/reddit-mcp-server/target/release/reddit-mcp-server

claude mcp list   # should show: reddit - Connected
```

The tools appear as `mcp__reddit__reddit_listing`, etc. (the server speaks stdio MCP, so it
works with any MCP client, not just Claude Code).

## Configuration (environment variables)

| Variable | Required | Notes |
|---|---|---|
| `REDDIT_CLIENT_ID` | yes | App client id. |
| `REDDIT_CLIENT_SECRET` | yes | App secret. |
| `REDDIT_USER_AGENT` | no | Defaults to `rust:reddit-mcp:0.1 (read-only)`. Reddit asks for a descriptive UA. |

## Development

```sh
cargo test                              # unit tests + MCP handshake integration test
cargo test --test live -- --ignored     # live smoke against real Reddit (needs creds in env)
cargo test --test call -- --ignored     # full tools/call round-trip over stdio (needs creds)
```

Logs go to stderr; stdout is reserved for the MCP JSON-RPC stream.

## License

MIT. See [LICENSE](LICENSE).
