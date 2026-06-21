//! SDK tool registration helpers for `CopilotChatActor`.
//!
//! Extracted from `actor.rs` to keep the actor event loop within the 200-line
//! logic threshold. Covers tool definition, handler registration, and
//! permission handler setup.

use augur_domain::string_newtypes::{ChoiceText, PromptText, StringNewtype};
use augur_domain::tools::builtin::query_user::QueryUserRequest;

/// Build the SDK `Tool` definition for `query_user`.
///
/// Returns a `copilot_sdk::Tool` with the name, description, and JSON schema
/// matching the built-in `QueryUserTool` handler. `skip_permission(true)`
/// bypasses the Copilot CLI permission gate so the tool is never denied before
/// our handler runs.
///
/// Returns the fully configured `copilot_sdk::Tool`.
/// Consumers: `actor::run_with_sdk`, `actor::attempt_session_restart`.
pub fn query_user_tool_def() -> copilot_sdk::Tool {
    copilot_sdk::Tool::new("query_user")
        .description(
            "Pause the agent turn and ask the user a question. \
             When the question has a finite set of valid answers, you MUST include \
             the `choices` array - do not omit it for yes/no questions, \
             option-selection questions, or any question where the answer space \
             is bounded. Only omit `choices` for genuinely open-ended questions.",
        )
        .schema(serde_json::json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to display to the user."
                },
                "choices": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "REQUIRED for finite-answer questions (yes/no, option selection, \
                                    bounded choice sets). Provide the full list of valid answers. \
                                    The user navigates choices with up/down arrows and selects with Enter. \
                                    Omit only for genuinely open-ended, free-text questions."
                }
            },
            "required": ["question"]
        }))
        .skip_permission(true)
}

/// Register the `query_user` tool handler on `session`.
///
/// The SDK handler is synchronous (`Arc<Fn>`). `block_in_place` temporarily
/// moves the current thread to a blocking context so `query_tx.send` and the
/// oneshot reply can be awaited without stalling the tokio scheduler.
///
/// Flow on every invocation:
/// 1. Parse `question` and optional `choices` from tool arguments.
/// 2. Build a `QueryUserRequest` with a fresh oneshot reply channel.
/// 3. Send the request to the TUI actor via `query_tx`.
/// 4. Block until the TUI sends the user's answer on the oneshot channel.
/// 5. Return the answer as a `ToolResultObject::text` to the Copilot model.
///
/// Parameters:
/// - `session`: the active Copilot SDK session.
/// - `query_tx`: sender half of the TUI query channel.
///
/// Consumers: `actor::run_with_sdk` and `actor::attempt_session_restart`
/// immediately after `create_session` succeeds.
#[tracing::instrument(skip(session, query_tx), level = "debug")]
pub async fn register_query_user_tool(
    session: &copilot_sdk::Session,
    query_tx: tokio::sync::mpsc::Sender<QueryUserRequest>,
) {
    use std::sync::Arc;

    let handler: copilot_sdk::ToolHandler =
        Arc::new(move |_name, args: &serde_json::Value| query_user_tool_result(args, &query_tx));

    session
        .register_tool_with_handler(query_user_tool_def(), Some(handler))
        .await;
}

fn query_user_tool_result(
    args: &serde_json::Value,
    query_tx: &tokio::sync::mpsc::Sender<QueryUserRequest>,
) -> copilot_sdk::ToolResultObject {
    use copilot_sdk::ToolResultObject;
    use tokio::sync::oneshot;

    let Some(question) = parse_question(args) else {
        return ToolResultObject::error("missing or empty 'question' argument");
    };
    let choices = parse_choices(args);
    let (reply_tx, reply_rx) = oneshot::channel();
    let req = QueryUserRequest::builder()
        .question(question)
        .choices(choices)
        .reply_tx(reply_tx)
        .build();
    tokio::task::block_in_place(|| wait_for_query_response(query_tx, req, reply_rx))
}

fn parse_question(args: &serde_json::Value) -> Option<PromptText> {
    args["question"]
        .as_str()
        .filter(|question| !question.is_empty())
        .map(PromptText::new)
}

fn parse_choices(args: &serde_json::Value) -> Vec<ChoiceText> {
    args["choices"]
        .as_array()
        .map(|choices| {
            choices
                .iter()
                .filter_map(|choice| {
                    choice
                        .as_str()
                        .filter(|text| !text.is_empty())
                        .map(ChoiceText::new)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn wait_for_query_response(
    query_tx: &tokio::sync::mpsc::Sender<QueryUserRequest>,
    req: QueryUserRequest,
    reply_rx: tokio::sync::oneshot::Receiver<augur_domain::string_newtypes::OutputText>,
) -> copilot_sdk::ToolResultObject {
    use copilot_sdk::ToolResultObject;

    let handle = tokio::runtime::Handle::current();
    if handle.block_on(query_tx.send(req)).is_err() {
        return ToolResultObject::error("TUI query channel closed");
    }
    match handle.block_on(reply_rx) {
        Ok(answer) => ToolResultObject::text(answer.into_inner()),
        Err(_) => ToolResultObject::error("query cancelled"),
    }
}
