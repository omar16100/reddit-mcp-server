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

Register it for the project where you actually want Reddit access (`-s project` or
`-s local`) rather than user-wide, so its untrusted output never shares a session with
unrelated credentials or tooling (see [Security](#security)).

```sh
claude mcp add -s local reddit \
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

## Security

Everything this server returns (titles, self-text, comments, flair, subreddit
descriptions) is written by arbitrary Reddit users and goes straight into your model's
context, so it can carry **prompt injection**. The server limits what that can achieve:

- **Read-only, app-only OAuth.** No user token exists, so nothing can act as you or read
  your inbox, and the server has no write tools.
- **Strict input validation.** Every value that reaches a request URL is checked against
  an allow-list (subreddit `^[A-Za-z0-9_]{2,21}$`, base36 post ids, fixed sort/time
  values, bounded search queries). Path-traversal or query-smuggling arguments such as
  `../../api/v1/me` are rejected with `invalid_params` before any request is made, and the
  API host is a constant.
- **Hidden-text stripping.** Control characters and invisible Unicode commonly used to hide
  instructions (zero-width characters, bidi overrides, tag characters, variation selectors)
  are removed from every returned string; every field is length-bounded.
- **Untrusted-content labelling.** Tool descriptions and server instructions tell the client
  model that returned text is data, never instructions.
- **Bounded responses.** Response bodies are capped (8 MiB API, 64 KiB token) and error
  excerpts are short and sanitised.

None of this makes prompt injection impossible; it is inherent to feeding user-generated
text to a model. Contain it on the client side as well:

- Don't enable this server in sessions that also hold sensitive credentials or broad tools,
  and don't pre-approve shell, file-write or web-fetch tools in those sessions.
- Prefer running the agent in a sandbox whose network egress is limited to
  `www.reddit.com` and `oauth.reddit.com`.

CI runs `cargo audit` on every change and weekly; Dependabot tracks Cargo and Actions
updates.

## Development

```sh
cargo test                              # unit, MCP handshake and input-validation tests
cargo test --test live -- --ignored     # live smoke against real Reddit (needs creds in env)
cargo test --test call -- --ignored     # full tools/call round-trip over stdio (needs creds)
```

Logs go to stderr; stdout is reserved for the MCP JSON-RPC stream.

## License

MIT. See [LICENSE](LICENSE).
