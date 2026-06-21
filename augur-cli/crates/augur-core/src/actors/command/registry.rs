//! Command registry: pure logic for registering, executing, and listing slash commands.

use super::types::{CommandDef, CommandOutcome};
use augur_domain::domain::string_newtypes::{
    AgentName, EndpointName, FilePath, ModelId, OutputText, PromptText, StringNewtype, ToolsText,
};
use augur_domain::tools::definition::ToolDefinition;
use std::sync::LazyLock;

/// Maximum completions shown in the command hint area above the input box.
///
/// Caps `hint_rows` in `LayoutSizes` so a large command list cannot crowd the
/// output pane. Applied in `CommandRegistry::completions` and by the layout engine.
pub const MAX_COMPLETIONS: usize = 12;

/// Width (chars) reserved for the usage column in formatted hint lines.
///
/// Used by `CommandRegistry::help_text` to align the description column.
/// The render module defines its own matching constant for the completion list.
const USAGE_COLUMN_WIDTH: usize = 22;

/// Width (chars) reserved for the tool name column in `/tools` output.
///
/// Used by `format_tools_text` to align tool descriptions. Wide enough for the
/// longest built-in tool name (`file_read_range`) with a small margin.
const TOOL_NAME_COLUMN_WIDTH: usize = 24;

static BUILTIN_COMMAND_ROWS: LazyLock<Vec<CommandDef>> = LazyLock::new(|| {
    vec![
        CommandDef::builder()
            .name("ask")
            .usage("/ask")
            .description("Open the ask panel for a side-channel LLM conversation")
            .build(),
        CommandDef::builder()
            .name("agent")
            .usage("/agent <name> <prompt>")
            .description("Launch a background agent session streaming output to the feed panel.")
            .build(),
        CommandDef::builder()
            .name("clear")
            .usage("/clear")
            .description("Start a new chat session and reset token totals")
            .build(),
        CommandDef::builder()
            .name("commit")
            .usage("/commit")
            .description("Create a git commit for the current changes")
            .build(),
        CommandDef::builder()
            .name("compact")
            .usage("/compact")
            .description("Compact the conversation context window")
            .build(),
        CommandDef::builder()
            .name("exit")
            .usage("/exit")
            .description("Exit the application (alias for /quit)")
            .build(),
        CommandDef::builder()
            .name("generate-catalog")
            .usage("/generate-catalog [--provider <name>]")
            .description("Generate model catalog from provider APIs")
            .build(),
        CommandDef::builder()
            .name("help")
            .usage("/help")
            .description("Display all available commands")
            .build(),
        CommandDef::builder()
            .name("model")
            .usage("/model <id>")
            .description("Switch the active Copilot model")
            .build(),
        CommandDef::builder()
            .name("new-session")
            .usage("/new-session")
            .description("Start a new conversation session (saves the current one)")
            .build(),
        CommandDef::builder()
            .name("ping")
            .usage("/ping")
            .description("Ping the application")
            .build(),
        CommandDef::builder()
            .name("push")
            .usage("/push")
            .description("Push the current branch to the remote server")
            .build(),
        CommandDef::builder()
            .name("quit")
            .usage("/quit")
            .description("Exit the application")
            .build(),
        CommandDef::builder()
            .name("run-pipeline")
            .usage("/run-pipeline [--resume] [--slug <slug>] [<context>]")
            .description("Start the deterministic orchestrator pipeline; --resume skips already-completed steps")
            .build(),
        CommandDef::builder()
            .name("run-plan")
            .usage("/run-plan <path>")
            .description("Load and execute a guided plan from a file")
            .build(),
        CommandDef::builder()
            .name("stop")
            .usage("/stop")
            .description("Stop the current command execution")
            .build(),
        CommandDef::builder()
            .name("switch")
            .usage("/switch <name>")
            .description("Switch to a different endpoint")
            .build(),
        CommandDef::builder()
            .name("tools")
            .usage("/tools")
            .description("List all available tools and their descriptions")
            .build(),
    ]
});

/// Owns the registered slash commands and handles both execution and completion.
///
/// All methods are pure: no I/O, no channels, no side effects. Constructed once
/// at startup via `with_builtins()` and shared read-only through `CommandHandle`.
pub struct CommandRegistry {
    pub(crate) commands: Vec<CommandDef>,
    tools_text: ToolsText,
}

impl CommandRegistry {
    /// Create a registry pre-loaded with all built-in commands.
    ///
    /// `tools` is the list of registered tool definitions; the registry
    /// pre-formats them into a displayable string for `/tools` output. Pass an
    /// empty slice when no tools are available (e.g., in tests).
    ///
    /// The built-in command set is: `/ask`, `/agent <name> <prompt>`, `/commit`,
    /// `/clear`, `/compact`, `/exit`, `/help`, `/model <id>`, `/new-session`, `/ping`,
    /// `/push`, `/quit`, `/run-pipeline`, `/run-plan <path>`, `/stop`,
    /// `/switch <name>`, `/tools`.
    pub fn with_builtins(tools: &[ToolDefinition]) -> Self {
        CommandRegistry {
            commands: builtin_commands(),
            tools_text: format_tools_text(tools),
        }
    }

    /// Execute a prompt string, returning the appropriate outcome.
    ///
    /// Returns `NotACommand` when `text` does not start with `/` so the caller
    /// can forward it to the agent. Returns `UnknownCommand` when the text starts
    /// with `/` but matches no registered command, enabling an error message.
    ///
    /// Dispatch is two-level: zero-argument commands are handled by
    /// `execute_simple`; argument-bearing commands are handled by
    /// `execute_parameterized`.
    pub(crate) fn execute(&self, text: &PromptText) -> CommandOutcome {
        if !text.as_str().starts_with('/') {
            return CommandOutcome::NotACommand;
        }
        self.execute_simple(text.as_str())
            .or_else(|| execute_parameterized(text.as_str()))
            .unwrap_or(CommandOutcome::UnknownCommand)
    }

    /// Dispatch zero-argument literal commands.
    ///
    /// Returns `Some(outcome)` for each recognised exact-match slash command
    /// that requires no arguments, or `None` when `text` is not one of the
    /// handled literals.
    fn execute_simple(&self, text: &str) -> Option<CommandOutcome> {
        execute_simple_control(text).or_else(|| self.execute_simple_info(text))
    }

    /// Dispatch zero-argument info commands that need access to registry-owned data.
    ///
    /// Handles `/help`, `/tools`, and `/ping` which require `&self` to format output.
    fn execute_simple_info(&self, text: &str) -> Option<CommandOutcome> {
        match text {
            "/help" => Some(CommandOutcome::SystemMessage(OutputText::from(
                self.help_text(),
            ))),
            "/tools" => Some(CommandOutcome::SystemMessage(OutputText::from(
                self.tools_text.as_str(),
            ))),
            "/ping" => Some(CommandOutcome::SystemMessage(OutputText::from(
                "[system] pong",
            ))),
            _ => None,
        }
    }

    /// Return commands whose name starts with `prefix` (the text after the `/`),
    /// alpha-sorted and capped at `MAX_COMPLETIONS`.
    ///
    /// `prefix` may be an empty string (when the user typed only `/`), in which
    /// case all commands are returned. Sorting ensures a stable, predictable order
    /// for keyboard navigation. Results are capped at `MAX_COMPLETIONS` rows.
    pub(crate) fn completions(&self, prefix: &PromptText) -> Vec<CommandDef> {
        let mut results: Vec<CommandDef> = self
            .commands
            .iter()
            .filter(|c| c.name.starts_with(prefix.as_str()))
            .copied()
            .collect();
        results.sort_by_key(|c| c.name);
        results.truncate(MAX_COMPLETIONS);
        results
    }

    /// Return all registered commands.
    ///
    /// Used by `CommandHandle::all_commands` for callers that need the full
    /// list independent of any typed prefix.
    pub fn all_commands(&self) -> &[CommandDef] {
        &self.commands
    }

    fn help_text(&self) -> String {
        let mut lines = vec!["Available commands:".to_owned()];
        for cmd in &self.commands {
            lines.push(format!(
                "  {:<width$}{}",
                cmd.usage,
                cmd.description,
                width = USAGE_COLUMN_WIDTH
            ));
        }
        lines.join("\n")
    }
}

/// Dispatch zero-argument control commands that have no argument-free variants.
///
/// Handles `/quit`, `/exit`, `/stop`, `/clear`, `/compact`, `/commit`, `/push`,
/// `/new-session`, and `/ask` - the commands that change application state
/// but require no registry-owned data.
fn execute_simple_control(text: &str) -> Option<CommandOutcome> {
    execute_simple_control_aliases(text).or_else(|| execute_simple_control_direct(text))
}

fn execute_simple_control_aliases(text: &str) -> Option<CommandOutcome> {
    if matches!(text, "/quit" | "/exit") {
        return Some(CommandOutcome::Quit);
    }
    None
}

fn execute_simple_control_direct(text: &str) -> Option<CommandOutcome> {
    [
        ("/stop", CommandOutcome::StopExecution),
        ("/clear", CommandOutcome::NewSession),
        ("/compact", CommandOutcome::CompactSession),
        ("/commit", CommandOutcome::CommitChanges),
        ("/push", CommandOutcome::PushBranch),
        ("/new-session", CommandOutcome::NewSession),
        ("/ask", CommandOutcome::OpenAskPanel),
    ]
    .into_iter()
    .find_map(|(command, outcome)| (text == command).then_some(outcome))
}

/// Dispatch argument-bearing slash commands that require parameter parsing.
///
/// Returns `Some(outcome)` for each recognised parameterised command, or `None`
/// when `text` does not match any of the five handled prefixes. This function is
/// intentionally free (no `&self`) because none of the argument-bearing commands
/// need registry-owned data.
fn execute_parameterized(text: &str) -> Option<CommandOutcome> {
    [
        parse_run_pipeline_outcome as fn(&str) -> Option<CommandOutcome>,
        parse_generate_catalog_outcome,
        parse_model_outcome,
        parse_agent_outcome,
        parse_run_plan_outcome,
        parse_switch_outcome,
    ]
    .into_iter()
    .find_map(|handler| handler(text))
}

fn parse_run_pipeline_outcome(text: &str) -> Option<CommandOutcome> {
    if text == "/run-pipeline" || text.starts_with("/run-pipeline ") {
        let rest = text.strip_prefix("/run-pipeline").unwrap_or("").trim();
        let resume = rest.split_whitespace().any(|w| w == "--resume");
        return Some(CommandOutcome::StartPipeline { resume });
    }
    None
}

fn parse_generate_catalog_outcome(text: &str) -> Option<CommandOutcome> {
    if text == "/generate-catalog" || text.starts_with("/generate-catalog ") {
        return Some(parse_generate_catalog(text));
    }
    None
}

fn parse_model_outcome(text: &str) -> Option<CommandOutcome> {
    if text == "/model" || text.starts_with("/model ") {
        return Some(
            parse_model(text)
                .map(CommandOutcome::SelectModel)
                .unwrap_or(CommandOutcome::SelectAutoModel),
        );
    }
    None
}

fn parse_agent_outcome(text: &str) -> Option<CommandOutcome> {
    text.starts_with("/agent").then(|| parse_agent(text))
}

fn parse_run_plan_outcome(text: &str) -> Option<CommandOutcome> {
    text.starts_with("/run-plan").then(|| parse_run_plan(text))
}

fn parse_switch_outcome(text: &str) -> Option<CommandOutcome> {
    parse_switch(text).map(CommandOutcome::SwitchEndpoint)
}

fn builtin_commands() -> Vec<CommandDef> {
    BUILTIN_COMMAND_ROWS.clone()
}

/// Format a list of tool definitions into the `/tools` display text.
///
/// Produces a two-column layout: tool name left-padded to `TOOL_NAME_COLUMN_WIDTH`
/// followed by the description. Tool entries are separated by a blank line so the
/// listing is easy to scan. Returns a fallback message when the list is empty.
/// Called once at registry construction and stored for zero-cost `/tools` execution.
fn format_tools_text(tools: &[ToolDefinition]) -> ToolsText {
    if tools.is_empty() {
        return ToolsText::from("No tools registered.");
    }
    let header = format!("Available tools ({}):", tools.len());
    let entries: Vec<String> = tools
        .iter()
        .map(|tool| {
            format!(
                "  {:<width$}{}",
                tool.name.as_str(),
                tool.description,
                width = TOOL_NAME_COLUMN_WIDTH
            )
        })
        .collect();
    ToolsText::from(format!("{}\n\n{}", header, entries.join("\n\n")))
}

/// Parse a `/model <id>` command.
///
/// Returns `Some(ModelId)` when the text has the `/model ` prefix and a
/// non-empty, non-whitespace-only model id following it. Returns `None` for
/// bare `/model` or `/model ` with no id. Consumed by `execute()` to produce
/// `CommandOutcome::SelectModel`. Consumers: `CommandRegistry::execute`.
fn parse_model(text: &str) -> Option<ModelId> {
    let id = text.strip_prefix("/model ")?.trim();
    if id.is_empty() {
        return None;
    }
    Some(ModelId::new(id))
}

/// Parse a `/switch <name>` command.
///
/// Returns `Some(EndpointName)` when the text has the `/switch ` prefix and
/// a non-empty, non-whitespace-only name following it. Returns `None` for bare
/// `/switch` or `/switch ` with no name.
fn parse_switch(text: &str) -> Option<EndpointName> {
    let name = text.strip_prefix("/switch ")?.trim();
    if name.is_empty() {
        return None;
    }
    Some(EndpointName::new(name))
}

/// Parse a `/agent <name> <prompt>` command.
///
/// Strips the `/agent` prefix from `text`, trims leading whitespace, then
/// splits on the first whitespace boundary into `(agent, prompt)`.
/// Returns `CommandOutcome::RunBackgroundAgent { agent, prompt }` when both
/// parts are non-empty. Returns `CommandOutcome::UnknownCommand` for bare
/// `/agent`, `/agent <name>` with no prompt, or any other malformed input.
/// Consumers: `CommandRegistry::execute`.
fn parse_agent(text: &str) -> CommandOutcome {
    let Some(rest) = text.strip_prefix("/agent ") else {
        return CommandOutcome::UnknownCommand;
    };
    let rest = rest.trim();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let agent = parts.next().unwrap_or("").trim();
    let prompt = parts.next().unwrap_or("").trim();
    if agent.is_empty() || prompt.is_empty() {
        CommandOutcome::UnknownCommand
    } else {
        CommandOutcome::RunBackgroundAgent {
            agent: AgentName::from(agent),
            prompt: PromptText::from(prompt),
        }
    }
}

/// Parse a `/run-plan <path>` command.
///
/// Returns `CommandOutcome::RunPlan(path)` when the text has the `/run-plan ` prefix
/// and a non-empty path following it. Returns `CommandOutcome::UnknownCommand` for
/// bare `/run-plan` or `/run-plan ` with no path.
/// Consumers: `CommandRegistry::execute`.
fn parse_run_plan(text: &str) -> CommandOutcome {
    let path = text.strip_prefix("/run-plan ").unwrap_or("").trim();
    if path.is_empty() {
        CommandOutcome::UnknownCommand
    } else {
        CommandOutcome::RunPlan(FilePath::from(path))
    }
}

/// Parse a `/generate-catalog [--provider <name>]` command.
///
/// Returns `CommandOutcome::GenerateCatalog { provider }` where provider is:
/// - `None` if no `--provider` flag is present
/// - `Some(name)` if `--provider <name>` is present
/// - Returns `UnknownCommand` if the command is malformed
///
/// Consumers: `CommandRegistry::execute`.
fn parse_generate_catalog(text: &str) -> CommandOutcome {
    let rest = text.strip_prefix("/generate-catalog").unwrap_or("").trim();

    let mut words = rest.split_whitespace().peekable();
    let mut provider = None;

    while let Some(word) = words.next() {
        if word == "--provider" {
            if let Some(provider_name) = words.next() {
                provider = Some(provider_name.to_string());
            } else {
                return CommandOutcome::UnknownCommand;
            }
        } else {
            return CommandOutcome::UnknownCommand;
        }
    }

    CommandOutcome::GenerateCatalog { provider }
}
