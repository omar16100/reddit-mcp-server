# rmcp 2.1 security dependency update (27 Sep 2026)

Status: implemented on branch `chore/rmcp-2-27092026`, supersedes Dependabot PR #1.

## Goal
Clear the open Dependabot alerts on `Cargo.lock` without changing the server's MCP behaviour.

## Alerts and advisories
| Advisory | Severity | Crate | Fixed in | Reachable here? |
|---|---|---|---|---|
| GHSA-33f5-2c5q-wgwj (OAuth protected resource metadata not validated) | high | rmcp | 2.0.0 | No: OAuth client code (`auth` feature) is not enabled. |
| GHSA-9pj6-vhgr-3mwh (Streamable HTTP server session-table leak) | high | rmcp | 2.0.0 | No: Streamable HTTP server transport is not enabled. |
| GHSA-9g45-5xwm-f3wc (custom headers forwarded on cross-origin redirects) | medium | rmcp | 2.1.0 | No: Streamable HTTP client transport is not enabled. |
| RUSTSEC-2026-0258 (h2 unbounded empty DATA frames) | no CVSS in the RustSec entry | h2 | 0.4.16 | Via reqwest to the Reddit API. |
| RUSTSEC-2026-0285 (rustls TLS 1.3 handshake messages across encryption levels) | medium (CVSS 3.1 5.3) | rustls | 0.23.45 | Via reqwest to the Reddit API. |

The server explicitly requests only rmcp features `server` and `transport-io` (stdio); defaults add `base64` and `macros`, and `transport-io` pulls in `transport-async-rw`. None of these enables the `auth` or Streamable HTTP code. The two RustSec items were found by `cargo audit`; GitHub had not raised alerts for them.

## Changes
- `Cargo.toml`: `rmcp = "2.1.0"` (from Dependabot PR #1). Lock pulls rmcp-macros 2.2.0.
- `Cargo.lock` only: h2 0.4.15 -> 0.4.19; rustls 0.23.41 -> 0.23.45 with aws-lc-rs 1.17.0 -> 1.18.1, aws-lc-sys 0.41.0 -> 0.45.0, rustls-webpki 0.103.13 -> 0.103.15, plus pkg-config 0.3.34 (a new Unix build dependency of aws-lc-sys).
- No source changes. The rmcp 2.0 breaking changes (model types aligned with MCP 2025-11-25) do not touch the APIs used here: `ToolRouter`, `Parameters`, `Json`, `ServerInfo`, `ServerCapabilities`, `ErrorData`, `#[tool]`, `#[tool_router]`, `#[tool_handler]`, `ServiceExt::serve(stdio())`.

## Verification
- `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`: 13 unit tests and the stdio handshake test pass; the two live tests stay `#[ignore]`.
- `cargo audit`: 0 vulnerabilities (was 2 on main, plus the 3 rmcp GHSA alerts).
- Offline stdio smoke against the release binaries of `main` (rmcp 1.8.0) and this branch, with dummy creds and all network denied by a macOS `sandbox-exec` profile:
  - `initialize` + `notifications/initialized` + `tools/list` for protocol versions 2024-11-05, 2025-06-18, 2025-11-25 and an unknown future version. Negotiated version and the full `tools/list` JSON (4 tools, input and output schemas) are identical; unknown versions negotiate 2025-11-25 in both. `initialize` differs only in `serverInfo.version`.
  - `tools/call` with missing params (`isError: true` result), an unknown tool (`-32602`), a valid call whose token fetch fails offline (`-32603 Reddit API error`), and `ping`: same responses on both.

## Behaviour deltas
The first four were reproduced against both release binaries (network denied). None affects a compliant MCP client using this server's four tools.
- A non-JSON line on stdin no longer gets a `-32700 Parse error` reply (rmcp 2.1.0, upstream PR #940).
- Valid JSON that is not a JSON-RPC envelope (e.g. `{"foo":"bar"}`) now gets `-32600 Invalid request` instead of `-32700`.
- A repeated `initialize` now negotiates the requested known version (e.g. 2024-11-05) instead of returning the default 2025-11-25 (upstream PR #930).
- `tools/call` with a malformed `task.ttl` (e.g. `-1`) now fails with `-32601` instead of `-32602`. This server does not support task-based invocation either way.
- The stdio reader is now cancel-safe (upstream PR #941): a partially read message is no longer lost if the read is interrupted by concurrent activity. Not covered by the sequential smoke test.
- `serverInfo` comes from `ServerInfo::default()`, which reports rmcp's own name and version (`rmcp` / `2.1.0`, previously `1.8.0`). Pre-existing; left as a todo.

## Not verified
- Live Reddit calls (`cargo test --test call -- --ignored`, `cargo test --test live -- --ignored`) were not run for this change: they need real credentials and network. The rustls/h2 bumps are semver-compatible patch releases on that path.
