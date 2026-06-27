//! The rmcp tool surface: four read-only Reddit tools exposed to MCP clients.

use crate::model::{
    AboutParams, AboutResult, CommentsParams, CommentsResult, ListingParams, PostList,
    SearchParams, summarize_about, summarize_comments, summarize_listing,
};
use crate::reddit::{RedditClient, RedditError};
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use std::sync::Arc;

#[derive(Clone)]
pub struct RedditMcpServer {
    client: Arc<RedditClient>,
    // Read by the #[tool_handler]-generated dispatch; dead-code analysis can't see that.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl RedditMcpServer {
    /// Build the server, loading Reddit creds from the environment.
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            client: Arc::new(RedditClient::from_env()?),
            tool_router: Self::tool_router(),
        })
    }
}

fn to_mcp(e: RedditError) -> McpError {
    match e {
        RedditError::RateLimited { reset_secs } => McpError::internal_error(
            format!("Reddit rate limited; retry in ~{reset_secs}s"),
            None,
        ),
        RedditError::Http(m) => McpError::internal_error(format!("Reddit API error: {m}"), None),
    }
}

#[tool_router]
impl RedditMcpServer {
    #[tool(
        description = "List posts from a subreddit. sort: hot|new|top|rising|controversial. Read-only."
    )]
    async fn reddit_listing(
        &self,
        Parameters(p): Parameters<ListingParams>,
    ) -> Result<Json<PostList>, McpError> {
        let v = self
            .client
            .get_listing(&p.subreddit, &p.sort, p.limit.clamp(1, 100), &p.time_filter)
            .await
            .map_err(to_mcp)?;
        let posts = summarize_listing(v.as_ref());
        Ok(Json(PostList { posts }))
    }

    #[tool(description = "Search Reddit posts; optionally restrict to a subreddit. Read-only.")]
    async fn reddit_search(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<Json<PostList>, McpError> {
        let v = self
            .client
            .search(
                &p.query,
                p.subreddit.as_deref(),
                &p.sort,
                &p.time,
                p.limit.clamp(1, 100),
                p.restrict_sr,
            )
            .await
            .map_err(to_mcp)?;
        let posts = summarize_listing(v.as_ref());
        Ok(Json(PostList { posts }))
    }

    #[tool(description = "Fetch a post and its comment thread (flattened, top-first). Read-only.")]
    async fn reddit_comments(
        &self,
        Parameters(p): Parameters<CommentsParams>,
    ) -> Result<Json<CommentsResult>, McpError> {
        let limit = p.comment_limit.clamp(1, 500);
        let v = self
            .client
            .get_comments(&p.post_id, p.subreddit.as_deref(), limit)
            .await
            .map_err(to_mcp)?;
        Ok(Json(summarize_comments(v.as_ref(), limit as usize)))
    }

    #[tool(
        description = "Get subreddit info: subscribers, active users, description, NSFW flag. Read-only."
    )]
    async fn reddit_subreddit_about(
        &self,
        Parameters(p): Parameters<AboutParams>,
    ) -> Result<Json<AboutResult>, McpError> {
        let v = self
            .client
            .get_subreddit_about(&p.subreddit)
            .await
            .map_err(to_mcp)?;
        Ok(Json(AboutResult {
            subreddit: summarize_about(v.as_ref()),
        }))
    }
}

#[tool_handler]
impl ServerHandler for RedditMcpServer {
    fn get_info(&self) -> ServerInfo {
        // ServerInfo is #[non_exhaustive] → build from default, then set fields.
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some(
            "Read-only Reddit access via app-only OAuth. Tools: reddit_listing, reddit_search, \
             reddit_comments, reddit_subreddit_about. This server CANNOT post, comment, or vote."
                .into(),
        );
        info
    }
}
