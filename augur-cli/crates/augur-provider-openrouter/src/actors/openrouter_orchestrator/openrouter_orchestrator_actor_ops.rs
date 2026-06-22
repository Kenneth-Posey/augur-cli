//! Private helper operations for the OpenRouter orchestrator actor.

use augur_domain::newtypes::{Count, NumericNewtype};

const DEFAULT_MAX_PARALLEL_WORKERS: Count = Count::of(4);

/// Resolve spawn-time worker parallelism, applying default when configured as zero.
pub(super) fn resolve_max_parallel_workers(configured: Count) -> Count {
    if configured.inner() == 0 {
        DEFAULT_MAX_PARALLEL_WORKERS
    } else {
        configured
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_max_parallel_workers_uses_default_for_zero() {
        assert_eq!(
            resolve_max_parallel_workers(Count::of(0)),
            DEFAULT_MAX_PARALLEL_WORKERS
        );
    }

    #[test]
    fn resolve_max_parallel_workers_keeps_configured_value() {
        assert_eq!(resolve_max_parallel_workers(Count::of(7)), Count::of(7));
    }
}
