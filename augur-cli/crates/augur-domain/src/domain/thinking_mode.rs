//! Reasoning effort levels for model thinking mode selection.
//!
//! `ReasoningEffort` maps to the GitHub Copilot SDK's `SetModelOptions::reasoning_effort`
//! field. The five variants cover the full set of accepted string values:
//! `"none"`, `"low"`, `"medium"`, `"high"`, and `"auto"`.
//!
//! These values are presented to the user in the thinking mode picker after they
//! select a model with `/model <id>`. The picker renders them in the completion
//! hint area above the input, identical to the model picker overlay.

use crate::domain::string_newtypes::{EffortLabel, StringNewtype};

/// Reasoning effort level for a model session.
///
/// Passed to `session.set_model(id, Some(SetModelOptions { reasoning_effort: Some(s) }))`
/// via `ChatProvider::set_model_with_options`. `Auto` lets the model decide. `None` disables
/// extended thinking entirely.
///
/// Consumers: `key_dispatch::submit`, `CopilotChatCmd::SetModel`, `CopilotChatHandle`,
/// `render_thinking_mode_hints`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReasoningEffort {
    /// Let the model automatically choose the thinking depth.
    Auto,
    /// Maximum thinking depth.
    High,
    /// Balanced thinking depth.
    Medium,
    /// Minimal thinking.
    Low,
    /// Disable extended thinking entirely.
    None,
}

impl ReasoningEffort {
    /// Parse a Copilot SDK string back into a `ReasoningEffort` variant.
    ///
    /// Accepts the same lowercase values produced by `AsRef<str>`: `"auto"`,
    /// `"high"`, `"medium"`, `"low"`, `"none"`.  Any other string returns
    /// `Option::None` so callers can fall back gracefully.
    pub fn parse_optional(s: impl AsRef<str>) -> Option<ReasoningEffort> {
        match s.as_ref() {
            "auto" => Some(ReasoningEffort::Auto),
            "high" => Some(ReasoningEffort::High),
            "medium" => Some(ReasoningEffort::Medium),
            "low" => Some(ReasoningEffort::Low),
            "none" => Some(ReasoningEffort::None),
            _ => Option::None,
        }
    }

    /// Return the display label shown in the thinking mode picker.
    ///
    /// Each label is formatted as `"{name}     ({hint})"` where the hint
    /// provides brief guidance to the user. `Auto` is marked recommended;
    /// `None` is marked disabled.
    pub fn display_label(&self) -> EffortLabel {
        match self {
            ReasoningEffort::Auto => EffortLabel::new("auto     (recommended)"),
            ReasoningEffort::High => EffortLabel::new("high"),
            ReasoningEffort::Medium => EffortLabel::new("medium"),
            ReasoningEffort::Low => EffortLabel::new("low"),
            ReasoningEffort::None => EffortLabel::new("none     (disabled)"),
        }
    }

    /// Return all five reasoning effort variants in picker display order.
    ///
    /// Order: `Auto`, `High`, `Medium`, `Low`, `None`.
    /// The picker renders them in this order top-to-bottom.
    pub fn options() -> Vec<ReasoningEffort> {
        vec![
            ReasoningEffort::Auto,
            ReasoningEffort::High,
            ReasoningEffort::Medium,
            ReasoningEffort::Low,
            ReasoningEffort::None,
        ]
    }
}

impl AsRef<str> for ReasoningEffort {
    fn as_ref(&self) -> &str {
        match self {
            ReasoningEffort::Auto => "auto",
            ReasoningEffort::High => "high",
            ReasoningEffort::Medium => "medium",
            ReasoningEffort::Low => "low",
            ReasoningEffort::None => "none",
        }
    }
}
