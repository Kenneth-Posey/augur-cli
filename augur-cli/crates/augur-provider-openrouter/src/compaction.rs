//! OpenRouter-only request compaction helpers for the agent actor.

use augur_domain::domain::task_types::MessageCompactor;
use augur_domain::newtypes::ToolResultStripFraction;
use augur_domain::string_newtypes::ModelId;
use augur_domain::string_newtypes::StringNewtype;
use augur_domain::types::{Message, Role};
use augur_domain::{NumericNewtype, OutputText, TokenCount};
use std::sync::Arc;

/// Main context budget for OpenRouter requests (400k tokens).
///
/// This is the maximum token allowance for complete messages before
/// compaction logic runs. The auto-compaction retry uses 1/4 of this budget.
const OPENROUTER_CONTEXT_BUDGET_TOKENS: TokenCount = TokenCount::of(400_000);

/// Default fraction of the oldest tool-result messages to strip during compaction.
///
/// Set to 90%: the compaction pre-pass drops the body of the oldest 90% of
/// `Role::Tool` messages before any turn-dropping logic runs.  Tool results
/// carry the bulk of shell-exec and file-read output, so stripping them
/// aggressively reclaims context while preserving the conversation structure.
const DEFAULT_STRIP_OLD_TOOL_RESULT_FRACTION: f64 = 0.9;

/// Build a `MessageCompactor` closure that resolves per-model config from the
/// provider catalog and always applies compaction, bypassing the budget check
/// so `/compact` always reclaims context even when under the auto-compact
/// threshold.
///
/// The resulting closure takes a full message list (including the leading system
/// prompt) and an optional model ID, then applies the same compaction logic
/// used for automatic compaction in the OpenRouter task actor, but without the
/// "already under budget" early exit. This ensures `/compact` always strips old
/// tool results and drops oldest turns to reduce context size.
///
/// Model-specific `compaction_target` and `strip_fraction` are resolved from the
/// provider catalog at call time using the active model ID. When no model ID is
/// provided, the catalog is still loaded and the first matching model or fallback
/// defaults are used. The env-var override
/// `AUGUR_CLI_OPENROUTER_CONTEXT_BUDGET_TOKENS` is not used by this
/// closure; the catalog YAML is the source of truth.
pub fn build_openrouter_message_compactor() -> MessageCompactor {
    Arc::new(|messages: Vec<Message>, model_id: Option<ModelId>| {
        let config = crate::model_config::resolve_model_config(model_id.as_ref());
        compact_messages_for_openrouter_forced(
            messages,
            config.compaction_target,
            config.strip_fraction,
        )
    })
}

/// Estimate the total token count of a message slice for compaction decisions.
///
/// This is the same logic used internally by the compaction machinery.
pub fn estimate_request_tokens_for_compaction(messages: &[Message]) -> TokenCount {
    let tokens: usize = messages.iter().map(estimate_message_tokens).sum();
    usize_to_token_count(tokens)
}

/// Compact a request using the OpenRouter safety budget and caller-provided
/// model-specific parameters.
///
/// `compaction_threshold` is the token budget for the compaction pass. Pass
/// [`openrouter_context_budget_tokens()`] or a model-specific value.
///
/// `strip_fraction` is the fraction of oldest tool-result messages to strip
/// during the pre-compaction pass. Pass
/// [`default_strip_old_tool_result_fraction()`] or a model-specific value.
pub fn compact_messages_for_openrouter(
    messages: Vec<Message>,
    compaction_threshold: TokenCount,
    strip_fraction: ToolResultStripFraction,
) -> Vec<Message> {
    compact_messages_with_threshold(messages, compaction_threshold, strip_fraction)
}

/// Like [`compact_messages_for_openrouter`] but without the early-exit budget
/// check.  Always applies tool-result stripping and turn-dropping until the
/// estimated request fits under `compaction_threshold`.
///
/// This is the variant used by the `/compact` command so that compaction
/// happens even when the conversation is below the auto-compact threshold.
pub fn compact_messages_for_openrouter_forced(
    messages: Vec<Message>,
    compaction_threshold: TokenCount,
    strip_fraction: ToolResultStripFraction,
) -> Vec<Message> {
    let threshold = token_count_to_usize(compaction_threshold);
    if threshold == 0 || messages.is_empty() {
        return messages;
    }
    compact_messages_with_threshold_impl(messages, threshold, strip_fraction)
}

/// Compact a request to a caller-provided threshold, using the default strip fraction.
///
/// Preserves the leading system prompt, drops the oldest conversation turns
/// first, and only removes leading instruction-prefix messages when no turns
/// remain to trim.
///
/// Uses [`default_strip_old_tool_result_fraction()`] for the pre-compaction pass.
pub fn compact_messages_with_default_strip(
    messages: Vec<Message>,
    compaction_threshold: TokenCount,
) -> Vec<Message> {
    compact_messages_with_threshold(
        messages,
        compaction_threshold,
        default_strip_old_tool_result_fraction(),
    )
}

/// Compact a request to a caller-provided threshold.
///
/// Preserves the leading system prompt, drops the oldest conversation turns
/// first, and only removes leading instruction-prefix messages when no turns
/// remain to trim.
pub fn compact_messages_with_threshold(
    messages: Vec<Message>,
    compaction_threshold: TokenCount,
    strip_fraction: ToolResultStripFraction,
) -> Vec<Message> {
    let threshold = token_count_to_usize(compaction_threshold);
    if threshold == 0 || estimate_request_tokens(&messages) <= threshold {
        return messages;
    }
    compact_messages_with_threshold_impl(messages, threshold, strip_fraction)
}

fn compact_messages_with_threshold_impl(
    messages: Vec<Message>,
    threshold: usize,
    strip_fraction: ToolResultStripFraction,
) -> Vec<Message> {
    let mut plan = MessagePlan::new(messages);
    strip_old_tool_results(&mut plan, strip_fraction);
    let mut dropped_prefixes = 0usize;
    let mut dropped_turns = 0usize;
    let mut prefix_start = 0usize;
    let mut turn_start = 0usize;

    loop {
        let candidate = build_candidate(
            &plan,
            CompactionStart {
                prefix_start,
                turn_start,
            },
            CompactionDropped {
                prefixes: dropped_prefixes,
                turns: dropped_turns,
            },
        );
        match next_compaction_step(
            CompactionThreshold {
                candidate_tokens: estimate_request_tokens(&candidate),
                threshold,
            },
            CompactionCursor {
                prefix_start,
                turn_start,
            },
            &plan,
        ) {
            CompactionStep::Done => return candidate,
            CompactionStep::DropTurn => {
                turn_start += 1;
                dropped_turns += 1;
            }
            CompactionStep::DropPrefix => {
                prefix_start += 1;
                dropped_prefixes += 1;
            }
        }
    }
}

/// Pre-compaction pass: strip the body of the oldest `fraction` of `Role::Tool`
/// messages across all turns.
///
/// Tool-result messages carry bulky output from `shell_exec`, `file_read`, and
/// other tools. Stripping them aggressively reclaims context while preserving
/// the user/assistant conversation structure so the model can still follow the
/// discussion flow.
///
/// Operates on the [`MessagePlan`] in place, scanning turns in oldest-first order.
fn strip_old_tool_results(plan: &mut MessagePlan, fraction: ToolResultStripFraction) {
    let fraction: f64 = fraction.into();

    // Collect indices of every tool-result message in turn order.
    let mut tool_indices: Vec<(usize, usize)> = Vec::new();
    for (turn_idx, turn) in plan.turns.iter().enumerate() {
        for (msg_idx, msg) in turn.iter().enumerate() {
            if msg.role == Role::Tool {
                tool_indices.push((turn_idx, msg_idx));
            }
        }
    }

    if tool_indices.is_empty() {
        return;
    }

    let to_strip = ((tool_indices.len() as f64) * fraction).ceil() as usize;
    let stripped_requests = to_strip.min(tool_indices.len());

    for &(turn_idx, msg_idx) in tool_indices.iter().take(stripped_requests) {
        if let Some(turn) = plan.turns.get_mut(turn_idx)
            && let Some(msg) = turn.get_mut(msg_idx)
        {
            msg.content = OutputText::new("");
        }
    }
}

struct MessagePlan {
    prefix_messages: Vec<Message>,
    system_prompt: Option<Message>,
    turns: Vec<Vec<Message>>,
}

impl MessagePlan {
    fn new(messages: Vec<Message>) -> Self {
        let leading_systems = messages
            .iter()
            .take_while(|m| m.role == Role::System)
            .count();
        let (leading, rest) = messages.split_at(leading_systems);
        let mut prefix_messages = leading.to_vec();
        let system_prompt = prefix_messages.pop();
        let turns = split_into_turns(rest);
        Self {
            prefix_messages,
            system_prompt,
            turns,
        }
    }
}

#[derive(Clone, Copy)]
struct CompactionStart {
    prefix_start: usize,
    turn_start: usize,
}

#[derive(Clone, Copy)]
struct CompactionDropped {
    prefixes: usize,
    turns: usize,
}

#[derive(Clone, Copy)]
struct CompactionCursor {
    prefix_start: usize,
    turn_start: usize,
}

enum CompactionStep {
    DropTurn,
    DropPrefix,
    Done,
}

fn split_into_turns(messages: &[Message]) -> Vec<Vec<Message>> {
    let mut turns: Vec<Vec<Message>> = Vec::new();
    let mut current: Vec<Message> = Vec::new();

    for message in messages {
        if message.role == Role::User && !current.is_empty() {
            turns.push(current);
            current = Vec::new();
        }
        current.push(message.clone());
    }

    if !current.is_empty() {
        turns.push(current);
    }

    turns
}

fn build_candidate(
    plan: &MessagePlan,
    start: CompactionStart,
    dropped: CompactionDropped,
) -> Vec<Message> {
    let mut result = Vec::new();
    result.extend(plan.prefix_messages[start.prefix_start..].iter().cloned());
    if let Some(system_prompt) = &plan.system_prompt {
        result.push(system_prompt.clone());
    }
    if dropped.prefixes > 0 || dropped.turns > 0 {
        result.push(Message::system(compaction_note(
            dropped.prefixes,
            dropped.turns,
        )));
    }
    for turn in &plan.turns[start.turn_start..] {
        result.extend(turn.iter().cloned());
    }
    result
}

fn compaction_note(dropped_prefixes: usize, dropped_turns: usize) -> String {
    match (dropped_prefixes, dropped_turns) {
        (0, 0) => String::new(),
        (0, turns) => format!(
            "[system] context compacted: {turns} older turn(s) omitted to fit the OpenRouter request budget"
        ),
        (prefixes, 0) => format!(
            "[system] context compacted: {prefixes} instruction block(s) omitted to fit the OpenRouter request budget"
        ),
        (prefixes, turns) => format!(
            "[system] context compacted: {turns} older turn(s) and {prefixes} instruction block(s) omitted to fit the OpenRouter request budget"
        ),
    }
}

struct CompactionThreshold {
    candidate_tokens: usize,
    threshold: usize,
}

fn next_compaction_step(
    budget: CompactionThreshold,
    cursor: CompactionCursor,
    plan: &MessagePlan,
) -> CompactionStep {
    if budget.candidate_tokens <= budget.threshold {
        CompactionStep::Done
    } else if cursor.turn_start.saturating_add(1) < plan.turns.len() {
        CompactionStep::DropTurn
    } else if cursor.prefix_start < plan.prefix_messages.len() {
        CompactionStep::DropPrefix
    } else {
        CompactionStep::Done
    }
}

fn estimate_request_tokens(messages: &[Message]) -> usize {
    messages.iter().map(estimate_message_tokens).sum()
}

fn estimate_message_tokens(message: &Message) -> usize {
    let mut total = token_count_to_usize(estimate_text_tokens(&message.content)).saturating_add(8);
    if let Some(tool_call_id) = &message.tool_call_id {
        total = total.saturating_add(token_count_to_usize(estimate_text_tokens(tool_call_id)));
    }
    if let Some(tool_calls) = &message.tool_calls {
        for call in tool_calls {
            total = total.saturating_add(token_count_to_usize(estimate_text_tokens(&call.id)));
            total = total.saturating_add(token_count_to_usize(estimate_text_tokens(&call.name)));
            let arguments = OutputText::new(call.arguments.to_string());
            total = total.saturating_add(token_count_to_usize(estimate_text_tokens(&arguments)));
        }
    }
    total
}

/// Estimate token count for wrapped text used in OpenRouter context budgeting.
///
/// Uses the same heuristic as before:
/// - word-based estimate (`split_whitespace().count()`)
/// - character-based estimate (`ceil(chars / 2)`)
/// - lower-bounded to one token
///
/// Returns the maximum of word and character estimates to preserve conservative
/// budgeting behavior.
pub fn estimate_text_tokens(input: &impl StringNewtype) -> TokenCount {
    let by_words = input.as_str().split_whitespace().count();
    let by_chars = (input.as_str().len().saturating_add(1)) / 2;
    usize_to_token_count(by_words.max(by_chars).max(1))
}

/// Resolve the context budget from an optional env-var override, falling back
/// to the compile-time constant budget.
///
/// Reads `AUGUR_CLI_OPENROUTER_CONTEXT_BUDGET_TOKENS` from the
/// environment. When the env var is unset, empty, or invalid, returns the
/// default 400_000 token budget.
pub fn openrouter_context_budget_tokens() -> TokenCount {
    std::env::var("AUGUR_CLI_OPENROUTER_CONTEXT_BUDGET_TOKENS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .map(TokenCount::new)
        .filter(|budget| *budget > TokenCount::ZERO)
        .unwrap_or(OPENROUTER_CONTEXT_BUDGET_TOKENS)
}

/// Return the default strip-old-tool-result fraction (0.9).
pub fn default_strip_old_tool_result_fraction() -> ToolResultStripFraction {
    ToolResultStripFraction::new(DEFAULT_STRIP_OLD_TOOL_RESULT_FRACTION)
}

fn token_count_to_usize(value: TokenCount) -> usize {
    usize::try_from(value.inner()).unwrap_or(usize::MAX)
}

fn usize_to_token_count(value: usize) -> TokenCount {
    TokenCount::new(u64::try_from(value).unwrap_or(u64::MAX))
}
