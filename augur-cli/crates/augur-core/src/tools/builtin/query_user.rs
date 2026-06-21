//! Built-in query_user tool: pauses the agent turn and collects structured user input.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use augur_domain::domain::string_newtypes::{
    ChoiceText, OutputText, PromptText, StringNewtype, ToolName,
};
use augur_domain::tools::builtin::query_user::QueryUserRequest;
use augur_domain::tools::definition::ToolDefinition;
use tokio::sync::{mpsc, oneshot};

const TOOL_NAME: &str = "query_user";

/// Tool that lets the LLM pause its turn and ask the user a structured question.
///
/// Validates the `question` argument, builds a `QueryUserRequest`, sends it to the
/// TUI actor via `request_tx`, and awaits the reply. The resolved answer is returned
/// as the `ToolCallResult` output. Registered in `wiring.rs::build_registry` at startup.
pub struct QueryUserTool {
    request_tx: mpsc::Sender<QueryUserRequest>,
}

impl QueryUserTool {
    /// Create a new `QueryUserTool` bound to `request_tx`.
    ///
    /// `request_tx` is the sending half of the mpsc channel whose receiving half is
    /// held by the TUI actor. Each `execute` call sends one `QueryUserRequest` and
    /// suspends until the TUI sends a reply on the oneshot channel.
    pub fn new(request_tx: mpsc::Sender<QueryUserRequest>) -> Self {
        Self { request_tx }
    }
}

#[async_trait::async_trait]
impl ToolHandler for QueryUserTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Pause the agent turn and ask the user a question. \
             When the question has a finite set of valid answers - such as yes/no, \
             multiple-choice, or option selection - always include the `choices` array. \
             Only omit `choices` for genuinely open-ended freeform questions.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "The question to display to the user."
                    },
                    "choices": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Choices the user can navigate with up/down arrow keys and select with Enter. \
                                        Include this whenever the question has a known set of valid answers. \
                                        Omit only for truly open-ended freeform questions."
                    }
                },
                "required": ["question"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let question = match args["question"].as_str() {
            Some(q) if !q.is_empty() => PromptText::new(q),
            _ => return error_result("missing or empty 'question' argument"),
        };
        let choices = parse_choices(&args);
        let (reply_tx, reply_rx) = oneshot::channel();
        let req = QueryUserRequest::builder()
            .question(question)
            .choices(choices)
            .reply_tx(reply_tx)
            .build();
        if self.request_tx.send(req).await.is_err() {
            return error_result("TUI query channel closed");
        }
        match reply_rx.await {
            Ok(answer) => ToolCallResult::builder()
                .name(ToolName::new(TOOL_NAME))
                .output(answer)
                .is_error(augur_domain::domain::newtypes::IsPredicate::from(false))
                .build(),
            Err(_) => error_result("query cancelled"),
        }
    }
}

/// Extract the `choices` array from the tool args, filtering to non-empty strings.
///
/// Returns an empty vec when the `choices` key is absent, null, or not an array.
/// Called by `execute` before constructing the `QueryUserRequest`.
fn parse_choices(args: &serde_json::Value) -> Vec<ChoiceText> {
    match args["choices"].as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|value| {
                value
                    .as_str()
                    .filter(|choice| !choice.is_empty())
                    .map(ChoiceText::new)
            })
            .collect(),
        None => vec![],
    }
}

/// Build an error `ToolCallResult` for the `query_user` tool with the given message.
fn error_result(msg: &str) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(msg))
        .is_error(augur_domain::domain::newtypes::IsPredicate::from(true))
        .build()
}
