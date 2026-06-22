//! StreamState domain tests.

use augur_domain::domain::{
    EndpointName, IsPredicate, LlmTokenCounts, LlmUsage, NumericNewtype, OutputText, StreamState,
    StringNewtype, Temperature, TokenCount, ToolCall, ToolCallResult, ToolDefinition, ToolExecutor,
    ToolName,
};

#[derive(Clone)]
struct MockToolExecutor;

#[async_trait::async_trait]
impl ToolExecutor for MockToolExecutor {
    fn definitions(&self) -> &[ToolDefinition] {
        &[]
    }

    async fn execute(&self, _call: ToolCall) -> anyhow::Result<ToolCallResult> {
        Ok(ToolCallResult {
            name: ToolName::new("mock_tool"),
            output: OutputText::new("mock_output"),
            is_error: IsPredicate(false),
            session_log: None,
        })
    }
}

#[test]
fn test_stream_state_construction() {
    let executor = MockToolExecutor;
    let endpoint = EndpointName::new("openrouter");
    let usage = LlmUsage {
        model: OutputText::new("gpt-4"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(100),
            tokens_out: TokenCount::new(50),
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.001.into(),
        },
        temperature: Temperature::new(0.7),
    };

    let state = StreamState::new(&executor, &endpoint, Some(usage.clone()));
    assert_eq!(*state.endpoint, EndpointName::new("openrouter"));
    assert!(state.last_usage.is_some());
}

#[test]
fn test_stream_state_field_access_with_usage() {
    let executor = MockToolExecutor;
    let endpoint = EndpointName::new("anthropic");
    let usage = LlmUsage {
        model: OutputText::new("claude-3"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(200),
            tokens_out: TokenCount::new(100),
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.002.into(),
        },
        temperature: Temperature::new(0.7),
    };

    let state = StreamState::new(&executor, &endpoint, Some(usage.clone()));
    assert_eq!(*state.endpoint, EndpointName::new("anthropic"));
    assert!(state.last_usage.is_some());
    assert!(state.prior_usage().is_some());
}

#[test]
fn test_stream_state_with_none_usage() {
    let executor = MockToolExecutor;
    let endpoint = EndpointName::new("openrouter");
    let state = StreamState::new(&executor, &endpoint, None);

    assert!(state.last_usage.is_none());
    assert!(state.is_first_invocation().0);
    assert!(state.prior_usage().is_none());
}

#[test]
fn test_stream_state_is_first_invocation() {
    let executor = MockToolExecutor;
    let endpoint = EndpointName::new("openrouter");
    let usage = LlmUsage {
        model: OutputText::new("gpt-4"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(100),
            tokens_out: TokenCount::new(50),
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.7),
    };

    let state_first = StreamState::new(&executor, &endpoint, None);
    let state_not_first = StreamState::new(&executor, &endpoint, Some(usage));

    assert!(state_first.is_first_invocation().0);
    assert!(!state_not_first.is_first_invocation().0);
}

#[test]
fn test_stream_state_lifetime_validity() {
    let executor = MockToolExecutor;
    let endpoint = EndpointName::new("endpoint");
    let state = StreamState::new(&executor, &endpoint, None);
    assert_eq!(*state.endpoint, EndpointName::new("endpoint"));
}

#[test]
fn test_stream_state_clone() {
    let executor = MockToolExecutor;
    let endpoint = EndpointName::new("openrouter");
    let usage = LlmUsage {
        model: OutputText::new("gpt-4"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(100),
            tokens_out: TokenCount::new(50),
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.7),
    };

    let state1 = StreamState::new(&executor, &endpoint, Some(usage.clone()));
    let state2 = state1.clone();
    assert_eq!(*state1.endpoint, *state2.endpoint);
}

#[test]
fn test_stream_state_multiple_endpoints() {
    let executor = MockToolExecutor;
    for endpoint_name in ["openrouter", "anthropic", "ollama"] {
        let endpoint = EndpointName::new(endpoint_name);
        let state = StreamState::new(&executor, &endpoint, None);
        assert_eq!(*state.endpoint, EndpointName::new(endpoint_name));
        assert!(state.is_first_invocation().0);
    }
}

#[test]
fn test_stream_state_helper_consistency() {
    let executor = MockToolExecutor;
    let endpoint = EndpointName::new("test");
    let usage = LlmUsage {
        model: OutputText::new("model"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(1),
            tokens_out: TokenCount::new(1),
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.7),
    };

    let state = StreamState::new(&executor, &endpoint, Some(usage));
    assert!(!state.is_first_invocation().0);
    assert!(state.prior_usage().is_some());
    assert!(state.last_usage.is_some());
}
