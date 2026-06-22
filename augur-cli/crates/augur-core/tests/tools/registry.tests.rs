use augur_core::tools::ToolDefinition;
use augur_core::tools::builtin::shell_exec::ShellExecTool;
use augur_core::tools::handler::{ToolCallResult, ToolHandler};
use augur_core::tools::registry::ToolRegistry;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolName};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

struct SnapshotSensitiveTool {
    flipped: Arc<AtomicBool>,
}

impl SnapshotSensitiveTool {
    fn new(flipped: Arc<AtomicBool>) -> Self {
        Self { flipped }
    }
}

#[async_trait::async_trait]
impl ToolHandler for SnapshotSensitiveTool {
    fn definition(&self) -> ToolDefinition {
        let name = if self.flipped.load(Ordering::SeqCst) {
            "mutated_name"
        } else {
            "stable_name"
        };
        ToolDefinition::new(name, "snapshot-sensitive tool", serde_json::json!({}))
    }

    async fn execute(&self, _args: serde_json::Value) -> ToolCallResult {
        ToolCallResult::builder()
            .name(ToolName::new("stable_name"))
            .output(OutputText::new("unused"))
            .is_error(augur_domain::domain::newtypes::IsPredicate::from(false))
            .build()
    }
}

/// Verifies that a registered tool can be found by name.
#[test]
fn register_and_find_tool() {
    let mut registry = ToolRegistry::new();
    registry.register(ShellExecTool);
    let found = registry.find(&ToolName::new("shell_exec"));
    assert!(found.is_some());
}

/// Verifies that find returns None for a tool that was never registered.
#[test]
fn find_unknown_tool_returns_none() {
    let registry = ToolRegistry::new();
    let found = registry.find(&ToolName::new("does_not_exist"));
    assert!(found.is_none());
}

/// Verifies that definitions() returns all registered schemas.
#[test]
fn definitions_returns_all_registered() {
    let mut registry = ToolRegistry::new();
    registry.register(ShellExecTool);
    registry.register(SnapshotSensitiveTool::new(Arc::new(AtomicBool::new(false))));
    assert_eq!(registry.definitions().len(), 2);
}

/// Verifies that ToolRegistry::default creates an empty registry.
#[test]
fn default_creates_empty_registry() {
    let registry = ToolRegistry::default();
    assert!(registry.definitions().is_empty());
    assert!(registry.find(&ToolName::new("any")).is_none());
}

/// Verifies that multiple registered tools remain individually findable by name.
#[test]
fn find_returns_matching_handler_for_each_registered_tool() {
    let flipped = Arc::new(AtomicBool::new(false));
    let mut registry = ToolRegistry::new();
    registry.register(ShellExecTool);
    registry.register(SnapshotSensitiveTool::new(flipped));

    let shell_exec = registry
        .find(&ToolName::new("shell_exec"))
        .expect("shell_exec should be registered");
    let stable_name = registry
        .find(&ToolName::new("stable_name"))
        .expect("stable_name should be registered");

    assert_eq!(shell_exec.definition().name, ToolName::new("shell_exec"));
    assert_eq!(stable_name.definition().name, ToolName::new("stable_name"));
}

/// Verifies that find continues to use the registered definition snapshot even if a handler's definition later changes.
#[test]
fn find_uses_registered_definition_snapshot() {
    let flipped = Arc::new(AtomicBool::new(false));
    let mut registry = ToolRegistry::new();
    registry.register(SnapshotSensitiveTool::new(flipped.clone()));

    assert_eq!(registry.definitions()[0].name, ToolName::new("stable_name"));

    flipped.store(true, Ordering::SeqCst);

    assert!(
        registry.find(&ToolName::new("stable_name")).is_some(),
        "find should use the name captured at registration time"
    );
    assert!(
        registry.find(&ToolName::new("mutated_name")).is_none(),
        "find should not expose names introduced after registration"
    );
}
