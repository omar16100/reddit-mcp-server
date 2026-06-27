//! Read-only Reddit MCP server (stdio transport) for Claude Code and other MCP clients.

use anyhow::Result;
use reddit_mcp_server::server::RedditMcpServer;
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // Logs MUST go to stderr — stdout is reserved for the MCP JSON-RPC stream.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("starting reddit-mcp-server (read-only)");

    let service = RedditMcpServer::new()?
        .serve(stdio())
        .await
        .inspect_err(|e| tracing::error!("serve error: {e:?}"))?;

    service.waiting().await?;
    Ok(())
}
