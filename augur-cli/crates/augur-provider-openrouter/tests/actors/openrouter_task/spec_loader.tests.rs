use augur_domain::task_types::AgentSpecName;
use augur_provider_openrouter::actors::openrouter_task::spec_loader::{
    find_agent_spec_path, load_agent_spec, strip_agent_name_prefix,
};
use std::fs;

#[test]
fn strip_agent_name_prefix_handles_prefixed_and_plain_names() {
    assert_eq!(
        strip_agent_name_prefix(&AgentSpecName::new("0-global-06-git-operator")).as_ref(),
        "git-operator"
    );
    assert_eq!(
        strip_agent_name_prefix(&AgentSpecName::new("git-operator")).as_ref(),
        "git-operator"
    );
    assert_eq!(
        strip_agent_name_prefix(&AgentSpecName::new("x-global-06-git-operator")).as_ref(),
        "x-global-06-git-operator"
    );
}

#[test]
fn find_agent_spec_path_prefers_exact_then_suffix_match() {
    let dir = tempfile::tempdir().expect("temp dir");
    let exact = dir.path().join("direct.agent.md");
    let suffixed = dir.path().join("0-global-06-git-operator.agent.md");
    fs::write(&exact, "direct").expect("write exact");
    fs::write(&suffixed, "suffix").expect("write suffix");

    let exact_found = find_agent_spec_path(dir.path(), &AgentSpecName::new("direct"));
    assert_eq!(exact_found.as_deref(), Some(exact.as_path()));

    let suffix_found = find_agent_spec_path(dir.path(), &AgentSpecName::new("git-operator"));
    assert_eq!(suffix_found.as_deref(), Some(suffixed.as_path()));
}

#[test]
fn find_agent_spec_path_returns_none_when_name_is_missing() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(find_agent_spec_path(dir.path(), &AgentSpecName::new("missing")).is_none());
}

#[tokio::test]
async fn load_agent_spec_reads_and_parses_instruction_body() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec_path = dir.path().join("planner.agent.md");
    fs::write(&spec_path, "plan this task").expect("write spec");

    let spec = load_agent_spec(&spec_path, AgentSpecName::new("planner"))
        .await
        .expect("spec should load and parse");

    assert_eq!(spec.name.as_ref(), "planner");
    assert_eq!(spec.instructions.as_ref(), "plan this task");
}
