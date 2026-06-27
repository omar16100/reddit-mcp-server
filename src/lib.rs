//! Read-only Reddit MCP server library.
//!
//! Split into:
//! - [`reddit`]: the HTTP client (app-only OAuth, token cache) + pure request builders.
//! - [`model`]: tool params, compact outputs, and raw-JSON → compact mappers.
//! - [`server`]: the rmcp tool surface exposed to Claude Code.

pub mod model;
pub mod reddit;
pub mod server;
