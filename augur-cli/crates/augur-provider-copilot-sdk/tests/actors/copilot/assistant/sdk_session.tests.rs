//! Tests for `sdk_session` assistant module.
//!
//! The session lifecycle functions (`create_session`, `resume_session`,
//! `create_or_resume_session`) require a live Copilot SDK subprocess and are
//! covered by end-to-end integration tests. This file provides structural
//! smoke tests that confirm module exports are accessible.

#[cfg(test)]
mod suite {
    /// Confirms that `create_or_resume_session` is accessible via the assistant
    /// module re-export. Symbol accessibility is verified by binding the
    /// function item directly, which only compiles when the symbol exists and is
    /// exported at the expected path.

    #[test]
    fn create_or_resume_session_is_accessible_via_assistant_module() {
        use augur_provider_copilot_sdk::actors::copilot::assistant::create_or_resume_session;
        let _ = create_or_resume_session;
    }
}
