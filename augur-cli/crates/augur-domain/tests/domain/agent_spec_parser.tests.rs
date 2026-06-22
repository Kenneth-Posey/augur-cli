use augur_domain::domain::agent_spec_parser::{AgentSpecParseError, parse_agent_spec};
use augur_domain::domain::{AgentSpecName, AgentToolSet, ModelId, StringNewtype};

/// Verifies that a minimal frontmatter block is parsed with description and body.
#[test]
fn parse_minimal_frontmatter() {
    let source = "---\ndescription: \"My agent\"\n---\n# body";
    let name = AgentSpecName::new("test-agent");
    let spec = parse_agent_spec(source, name).unwrap();
    assert_eq!(spec.meta.description, "My agent");
    assert!(spec.instructions.as_ref().contains("# body"));
}

/// Verifies that a model override is captured as `Some(ModelId)`.
#[test]
fn parse_with_model_override() {
    let source = "---\nmodel: \"openai/gpt-4o\"\n---\nInstructions.";
    let name = AgentSpecName::new("test-agent");
    let spec = parse_agent_spec(source, name).unwrap();
    assert_eq!(spec.meta.model, Some(ModelId::new("openai/gpt-4o")));
}

/// Verifies that a named tool list produces `AgentToolSet::Named`.
#[test]
fn parse_with_named_tools() {
    let source = "---\ntools:\n  - file_read\n  - list_directory\n---\nDo things.";
    let name = AgentSpecName::new("test-agent");
    let spec = parse_agent_spec(source, name).unwrap();
    match &spec.meta.tools {
        AgentToolSet::Named(tools) => {
            assert_eq!(tools.len(), 2);
            assert_eq!(tools[0].as_ref(), "file_read");
            assert_eq!(tools[1].as_ref(), "list_directory");
        }
        other => panic!("expected Named, got {other:?}"),
    }
}

/// Verifies that `tools: all` string produces `AgentToolSet::All`.
#[test]
fn parse_tools_all() {
    let source = "---\ntools: all\n---\nDo everything.";
    let name = AgentSpecName::new("test-agent");
    let spec = parse_agent_spec(source, name).unwrap();
    assert!(matches!(spec.meta.tools, AgentToolSet::All));
}

/// Verifies that a file with no frontmatter uses the entire source as instructions.
#[test]
fn parse_no_frontmatter() {
    let source = "Just plain instructions without any YAML block.";
    let name = AgentSpecName::new("plain-agent");
    let spec = parse_agent_spec(source, name).unwrap();
    assert_eq!(spec.instructions.as_ref(), source);
    assert!(matches!(spec.meta.tools, AgentToolSet::All));
    assert!(spec.meta.model.is_none());
}

/// Verifies that a missing `description` key falls back to the agent name.
#[test]
fn parse_missing_description_uses_name_default() {
    let source = "---\nmodel: \"anthropic/claude-3\"\n---\nInstructions here.";
    let name = AgentSpecName::new("my-agent");
    let spec = parse_agent_spec(source, name).unwrap();
    assert_eq!(spec.meta.description, "my-agent");
}

/// Verifies that invalid YAML in the frontmatter returns `AgentSpecParseError::YamlError`.
#[test]
fn parse_invalid_yaml_returns_error() {
    let source = "---\n: invalid: yaml: [\n---\nbody";
    let name = AgentSpecName::new("bad-agent");
    let result = parse_agent_spec(source, name);
    assert!(matches!(result, Err(AgentSpecParseError::YamlError(_))));
}
