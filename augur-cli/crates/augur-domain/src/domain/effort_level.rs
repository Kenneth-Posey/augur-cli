//! Effort level classification based on LLM temperature settings.

use crate::domain::newtypes::NumericNewtype;
use crate::domain::{EffortLabel, Temperature};

/// Human-readable tier that maps a temperature float to a named effort level.
///
/// Used in the status bar to display the active configuration as e.g.
/// `"claude-sonnet-4-6 (high)"`. Constructed via
/// [`EffortLevel::from_temperature`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EffortLevel {
    /// Temperature == 0.0.
    Low,
    /// Temperature > 0.0 and <= 0.5.
    Medium,
    /// Temperature > 0.5.
    High,
}

impl EffortLevel {
    /// Maps a temperature float to the nearest effort tier.
    ///
    /// Thresholds: `0.0` → `Low`, `(0.0, 0.5]` → `Medium`, `> 0.5` → `High`.
    pub fn from_temperature(temp: Temperature) -> Self {
        let v = temp.inner();
        match () {
            _ if v <= 0.0 => EffortLevel::Low,
            _ if v <= 0.5 => EffortLevel::Medium,
            _ => EffortLevel::High,
        }
    }

    /// Returns the lowercase display label used in the status bar.
    ///
    /// Possible values are wrapped in [`EffortLabel`].
    pub fn label(&self) -> EffortLabel {
        match self {
            EffortLevel::Low => EffortLabel::from("low"),
            EffortLevel::Medium => EffortLabel::from("medium"),
            EffortLevel::High => EffortLabel::from("high"),
        }
    }
}
