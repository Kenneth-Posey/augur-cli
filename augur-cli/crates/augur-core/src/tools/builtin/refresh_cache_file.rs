//! Built-in refresh_cache_file tool: forces re-read of a file in the cache actor.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use crate::tools::ports::CacheToolPort;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;
use std::path::PathBuf;

const TOOL_NAME: &str = "refresh_cache_file";

/// Forces the cache actor to re-read a source file and rebuild the snapshot.
///
/// Use when a file has changed on disk and the LLM needs fresh context in the
/// next request. Registered in `wiring.rs::build_registry` when a `CacheHandle`
/// is available.
pub struct RefreshCacheFileTool {
    cache: Box<dyn CacheToolPort>,
}

impl RefreshCacheFileTool {
    /// Create a new tool bound to the given cache provider.
    ///
    /// Each `execute` call sends a refresh request through the provider.
    pub fn new(cache: impl CacheToolPort) -> Self {
        Self {
            cache: Box::new(cache),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for RefreshCacheFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Re-read a source file and refresh its cached content. Use when you know \
             a file has changed and want updated context in the next request.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative path to the source file to refresh."
                    }
                },
                "required": ["path"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args), fields(tool = "refresh_cache_file"))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let path_str = match args["path"].as_str() {
            Some(s) if !s.is_empty() => s.to_owned(),
            _ => return error_result("missing or empty 'path' argument"),
        };
        let path = PathBuf::from(path_str);
        match self.cache.refresh_file(path).await {
            Ok(()) => ToolCallResult::builder()
                .name(ToolName::new(TOOL_NAME))
                .output(OutputText::new(
                    "cache refresh requested; snapshot will be rebuilt",
                ))
                .is_error(augur_domain::domain::newtypes::IsPredicate::from(false))
                .build(),
            Err(e) => ToolCallResult::builder()
                .name(ToolName::new(TOOL_NAME))
                .output(OutputText::new(e.to_string()))
                .is_error(augur_domain::domain::newtypes::IsPredicate::from(true))
                .build(),
        }
    }
}

/// Build an error `ToolCallResult` for `refresh_cache_file` with the given message.
fn error_result(msg: &str) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(msg))
        .is_error(augur_domain::domain::newtypes::IsPredicate::from(true))
        .build()
}
