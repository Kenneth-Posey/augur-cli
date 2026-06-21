//! Private helper operations for the tool actor.

use super::tool_ops::ToolCallCommand;
use crate::tools::handler::ToolCallResult;
use crate::tools::registry::ToolRegistry;
use augur_domain::domain::newtypes::IsPredicate;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};
use std::sync::Arc;

/// Resolve and execute one tool call, then reply with the result.
///
/// If no handler exists for `cmd.call.name`, responds with a `"tool not found"`
/// error result. Otherwise executes the handler asynchronously.
pub(super) async fn dispatch_tool_call(cmd: ToolCallCommand, registry: Arc<ToolRegistry>) {
    let result = match registry.find(&cmd.call.name) {
        None => ToolCallResult::builder()
            .name(cmd.call.name)
            .output(OutputText::new("tool not found"))
            .is_error(IsPredicate::from(true))
            .build(),
        Some(handler) => handler.execute(cmd.call.arguments).await,
    };
    let _ = cmd.reply_tx.send(result);
}
