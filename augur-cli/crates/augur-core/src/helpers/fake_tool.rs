//! FakeToolExecutor: configurable tool execution for use in agent actor tests.

use crate::actors::tool::handle::ToolExecutor;
use crate::actors::tool::tool_ops::ToolCall;
use crate::tools::handler::ToolCallResult;
use augur_domain::domain::string_newtypes::OutputText;
use augur_domain::tools::definition::ToolDefinition;

/// A test double for `ToolExecutor` backed by a configurable handler closure.
///
/// `always_ok(output)` creates an executor that returns a successful result
/// for every call, echoing the tool name with the given output text. The
/// `handler` field can be replaced for tests that need custom behavior.
pub struct FakeToolExecutor {
    defs: Vec<ToolDefinition>,
    /// Closure invoked on every `execute` call; returns the tool result.
    pub handler: Box<dyn Fn(ToolCall) -> ToolCallResult + Send + Sync>,
}

impl FakeToolExecutor {
    /// Create a fake that always returns a successful result with `output` text.
    ///
    /// The tool name from the call is preserved in the result. `is_error` is
    /// `false`. Suitable for tests that only need to verify the agent loop
    /// continues without testing tool output content.
    pub fn always_ok(output: impl Into<OutputText>) -> Self {
        let out = output.into();
        FakeToolExecutor {
            defs: vec![],
            handler: Box::new(move |call| {
                ToolCallResult::builder()
                    .name(call.name)
                    .output(out.clone())
                    .is_error(augur_domain::domain::newtypes::IsPredicate::from(false))
                    .build()
            }),
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for FakeToolExecutor {
    fn definitions(&self) -> &[ToolDefinition] {
        &self.defs
    }

    async fn execute(&self, call: ToolCall) -> anyhow::Result<ToolCallResult> {
        Ok((self.handler)(call))
    }
}
