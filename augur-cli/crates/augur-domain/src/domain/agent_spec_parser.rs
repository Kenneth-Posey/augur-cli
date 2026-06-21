//! Pure parsing logic for agent specification files.
//!
//! Parses optional YAML frontmatter and instruction body from agent spec source
//! text. Contains no IO and no async - the IO wrapper lives in the actors layer
//! at `augur_provider_openrouter::actors::openrouter_task::spec_loader`.

use crate::domain::string_newtypes::StringNewtype;
use crate::domain::{
    AgentInstructions, AgentSpec, AgentSpecMeta, AgentSpecName, AgentToolSet, ModelId, OutputText,
};
use std::fmt;

/// Internal raw deserialization target for agent spec YAML frontmatter.
///
/// All fields are optional; absent fields fall back to defaults derived
/// from the `name` argument passed to [`parse_agent_spec`].
#[derive(serde::Deserialize, Default)]
struct RawAgentSpecMeta {
    description: Option<String>,
    model: Option<String>,
    tools: Option<ToolsField>,
}

/// YAML encoding of the tool permission set for an agent specification.
///
/// An untagged enum: the string `"all"` deserializes to `All`; a sequence of
/// strings deserializes to `Named`.
#[allow(dead_code)]
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ToolsField {
    /// Any string value (conventionally `"all"`) grants all tools.
    All(String),
    /// A list of tool spec names restricts the agent to those tools only.
    Named(Vec<String>),
}

/// Error returned when parsing an agent specification from source text fails.
#[derive(Debug)]
pub enum AgentSpecParseError {
    /// Included for API completeness; the parser treats absent frontmatter as
    /// valid (the whole file becomes the instruction body).
    MissingFrontmatter,
    /// The YAML frontmatter block contained malformed YAML.
    YamlError(String),
}

impl fmt::Display for AgentSpecParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentSpecParseError::MissingFrontmatter => {
                write!(f, "missing YAML frontmatter block")
            }
            AgentSpecParseError::YamlError(msg) => write!(f, "YAML parse error: {msg}"),
        }
    }
}

impl std::error::Error for AgentSpecParseError {}

/// Parse an agent specification from its raw file source text.
///
/// Accepts source text that optionally begins with a YAML frontmatter block
/// delimited by `---\n` fences. When no frontmatter is present the entire
/// source becomes the instruction body and all metadata falls back to defaults
/// derived from `name`.
///
/// # Parameters
///
/// - `source`: raw text content of the agent spec file.
/// - `name`: the logical name used as a fallback description when none is
///   specified in the frontmatter.
///
/// # Errors
///
/// Returns [`AgentSpecParseError::YamlError`] when the frontmatter block
/// contains malformed YAML that cannot be deserialized.
pub fn parse_agent_spec(
    source: impl AsRef<str>,
    name: AgentSpecName,
) -> Result<AgentSpec, AgentSpecParseError> {
    let (yaml_block, instructions_body) = split_frontmatter(source.as_ref());

    let raw: RawAgentSpecMeta = serde_yaml::from_str(yaml_block)
        .map_err(|e| AgentSpecParseError::YamlError(e.to_string()))?;

    let description = raw
        .description
        .map(OutputText::new)
        .unwrap_or_else(|| OutputText::new(name.to_string()));

    let model = raw.model.map(ModelId::new);

    let tools = parse_tool_set(raw.tools);

    let meta = AgentSpecMeta::builder()
        .description(description)
        .maybe_model(model)
        .tools(tools)
        .build();

    let spec = AgentSpec::builder()
        .name(name)
        .meta(meta)
        .instructions(AgentInstructions::new(instructions_body))
        .build();

    Ok(spec)
}

fn split_frontmatter(source: &str) -> (&str, &str) {
    const FENCE: &str = "---\n";
    let Some(after_open) = source.strip_prefix(FENCE) else {
        return ("", source.trim());
    };
    let Some(offset) = after_open.find(FENCE) else {
        return ("", source.trim());
    };
    let yaml_end = offset;
    let body_start = yaml_end + FENCE.len();
    (&after_open[..yaml_end], after_open[body_start..].trim())
}

fn parse_tool_set(tools: Option<ToolsField>) -> AgentToolSet {
    match tools {
        Some(ToolsField::Named(v)) => {
            AgentToolSet::Named(v.into_iter().map(AgentSpecName::new).collect())
        }
        Some(ToolsField::All(_)) | None => AgentToolSet::All,
    }
}
