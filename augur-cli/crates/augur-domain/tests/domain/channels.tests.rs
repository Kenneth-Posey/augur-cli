#![allow(clippy::duplicate_mod)]
use augur_domain::domain::channels::TOKEN_TRACKER_COMMAND_CAPACITY;
#[path = "../support/rustdoc.tests.rs"]
mod rustdoc_support;

/// Verifies channel-capacity constants use domain numeric wrappers in public APIs.
#[test]
fn channel_capacity_constants_use_domain_numeric_wrappers() {
    let html = rustdoc_support::rustdoc_html(
        "augur_domain/domain/channels/constant.LLM_COMMAND_CAPACITY.html",
    );
    assert!(
        html.contains("struct.Count.html") || html.contains("struct.ChannelCapacity.html"),
        "expected LLM_COMMAND_CAPACITY rustdoc to reference a domain wrapper type",
    );
}

/// Verifies TOKEN_TRACKER_COMMAND_CAPACITY equals 64.
#[test]
fn test_token_tracker_command_capacity_is_64() {
    assert_eq!(*TOKEN_TRACKER_COMMAND_CAPACITY, 64usize);
}
