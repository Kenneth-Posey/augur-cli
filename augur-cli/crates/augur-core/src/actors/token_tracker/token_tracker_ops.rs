//! Token-tracker actor ops: pure accumulation logic.
//!
//! `accumulate` is a **pure function** - no I/O, no side effects. The actor
//! calls it to fold one `LlmUsage` into the running `ProjectTokenTotals`.

pub use augur_domain::TokenTrackerCommand;
use augur_domain::domain::types::{LlmUsage, ProjectTokenTotals};

/// Fold one `LlmUsage` into the running `ProjectTokenTotals`.
///
/// Pure function: same inputs always produce the same output, with no I/O or
/// observable side effects. All five numeric fields are added independently.
/// The result satisfies the monotone-accumulation invariant (INV-002):
/// every field in `totals` after the call is ≥ the corresponding field before.
///
/// # Examples
///
/// ```
/// # use augur_core::domain::types::{LlmTokenCounts, LlmUsage, ProjectTokenTotals};
/// # use augur_core::domain::{TokenCount, Temperature};
/// # use augur_core::domain::string_newtypes::{OutputText, StringNewtype};
/// # use augur_core::domain::newtypes::NumericNewtype;
/// # use augur_core::actors::token_tracker::token_tracker_ops::accumulate;
/// let mut totals = ProjectTokenTotals::default();
/// let usage = LlmUsage {
///     model: OutputText::new("claude-sonnet-4-6"),
///     token_counts: LlmTokenCounts {
///         tokens_in: TokenCount::new(100),
///         tokens_out: TokenCount::new(50),
///         tokens_cached: TokenCount::new(10),
///         cache_write_tokens: TokenCount::new(5),
///         cost_usd: 0.02.into(),
///     },
///     temperature: Temperature::new(1.0),
/// };
/// accumulate(&mut totals, &usage);
/// assert_eq!(totals.tokens_in, TokenCount::new(100));
/// assert_eq!(totals.cost_usd, 0.02);
/// ```
pub fn accumulate(totals: &mut ProjectTokenTotals, usage: &LlmUsage) {
    totals.tokens_in += usage.tokens_in;
    totals.tokens_out += usage.tokens_out;
    totals.tokens_cached += usage.tokens_cached;
    totals.cache_write_tokens += usage.cache_write_tokens;
    totals.cost_usd += usage.cost_usd;
}
