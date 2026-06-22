/// CF-001: non-exhaustive `StepStatus` matches are rejected at compile time.
#[test]
fn compile_fail_step_status_non_exhaustive_match_rejected() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/cases/cf_001_step_status_non_exhaustive.tests.rs");
}

/// CF-002: raw string assignment to `ExecutionStepId` is rejected by type system.
#[test]
fn compile_fail_execution_step_id_raw_string_assignment_rejected() {
    let t = trybuild::TestCases::new();
    t.compile_fail(
        "tests/compile_fail/cases/cf_002_execution_step_id_raw_string_assignment.tests.rs",
    );
}

/// CF-003: legacy direct multi-step dispatch API must remain absent.
#[test]
fn compile_fail_task_runner_legacy_direct_multi_step_dispatch_absent() {
    let t = trybuild::TestCases::new();
    t.compile_fail(
        "tests/compile_fail/cases/cf_003_task_runner_legacy_direct_multi_step_dispatch_absent.tests.rs",
    );
}

/// CF-004: bypass API skipping `submit_execution_plan` must remain absent.
#[test]
fn compile_fail_task_runner_bypass_submit_execution_plan_absent() {
    let t = trybuild::TestCases::new();
    t.compile_fail(
        "tests/compile_fail/cases/cf_004_task_runner_bypass_submit_execution_plan_absent.tests.rs",
    );
}
