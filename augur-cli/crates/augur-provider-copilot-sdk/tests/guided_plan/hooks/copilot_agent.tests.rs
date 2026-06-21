/// Verifies that `check_verdict_suffix` returns `Passed` when the accumulated
/// response text contains `"VERDICT: PASS"` anywhere in the string.
#[test]
fn check_verdict_suffix_pass_pattern_returns_passed() {
    use augur_domain::guided_plan::HookOutcome;
    use augur_provider_copilot_sdk::guided_plan::hooks::copilot_agent::check_verdict_suffix;

    let text = "The implementation looks correct. VERDICT: PASS";
    let outcome = check_verdict_suffix(text);
    assert!(
        matches!(outcome, Some(HookOutcome::Passed)),
        "VERDICT: PASS pattern must return Passed; got {outcome:?}"
    );
}

/// Verifies that `check_verdict_suffix` returns `NeedsRework` with the extracted
/// reason when the text contains `"VERDICT: REWORK(<reason>)"`.
#[test]
fn check_verdict_suffix_rework_pattern_extracts_reason() {
    use augur_domain::guided_plan::HookOutcome;
    use augur_provider_copilot_sdk::guided_plan::hooks::copilot_agent::check_verdict_suffix;

    let text = "Found issues in the implementation. VERDICT: REWORK(missing error handling)";
    let outcome = check_verdict_suffix(text);
    match outcome {
        Some(HookOutcome::NeedsRework(reason)) => {
            assert_eq!(
                reason.to_string(),
                "missing error handling",
                "extracted reason must match"
            );
        }
        other => panic!("expected NeedsRework; got {other:?}"),
    }
}

/// Verifies that `check_verdict_suffix` returns `None` when no verdict pattern
/// is present so the caller can treat the session as failed.
#[test]
fn check_verdict_suffix_no_pattern_returns_none() {
    use augur_provider_copilot_sdk::guided_plan::hooks::copilot_agent::check_verdict_suffix;

    let text = "The review is still in progress, conclusions TBD.";
    let outcome = check_verdict_suffix(text);
    assert!(
        outcome.is_none(),
        "text with no verdict pattern must return None"
    );
}

/// Verifies that `check_verdict_suffix` matches `VERDICT: PASS` even when
/// additional text follows it (e.g., the model continues after the verdict).
#[test]
fn check_verdict_suffix_pass_with_trailing_text() {
    use augur_domain::guided_plan::HookOutcome;
    use augur_provider_copilot_sdk::guided_plan::hooks::copilot_agent::check_verdict_suffix;

    let text = "VERDICT: PASS\n\nOverall the phase meets the acceptance criteria.";
    let outcome = check_verdict_suffix(text);
    assert!(
        matches!(outcome, Some(HookOutcome::Passed)),
        "VERDICT: PASS with trailing text must still return Passed"
    );
}
