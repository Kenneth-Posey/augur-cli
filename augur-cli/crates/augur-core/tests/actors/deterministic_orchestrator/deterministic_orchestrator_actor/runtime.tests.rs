//! Cargo-runnable integration coverage for the deterministic orchestrator runtime.

use augur_core::actors::DeterministicOrchestratorHandle;
use augur_core::actors::deterministic_orchestrator::deterministic_orchestrator_actor::spawn;
use augur_core::actors::deterministic_orchestrator::handle::PipelineResumeMode;
use augur_core::domain::deterministic_orchestrator::DeterministicOrchestratorEvent;
use augur_domain::domain::WorkflowStepId;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn temp_repo() -> TempDir {
    TempDir::new().expect("temp repo")
}

fn write_repo_file(repo_root: &Path, relative_path: &str, contents: &str) {
    let path = repo_root.join(relative_path);
    let parent = path
        .parent()
        .expect("test fixture path should always have a parent");
    fs::create_dir_all(parent).expect("test fixture directory should be created");
    fs::write(path, contents).expect("test fixture file should be written");
}

fn write_expected_inputs(repo_root: &Path) {
    write_repo_file(repo_root, "plans/example/input-a.md", "fixture input a");
    write_repo_file(repo_root, "plans/example/input-b.md", "fixture input b");
}

fn two_step_workflow_fixture(first_step_id: &str, second_step_id: &str) -> String {
    format!(
        r#"
stages:
  - stage_id: "review"
    steps:
      - step_id: "{first_step_id}"
        step_type: "worker_with_gate"
        model: "runner-default"
        thinking_depth: "runner-default"
        worker_agent: "worker-alpha"
        gate_agent: "gate-alpha"
        expected_inputs:
          - "plans/example/input-a.md"
        created_artifacts:
          - "plans/example/output-a.md"
        on_pass:
          next_step: "{second_step_id}"
        on_fail:
          action: "halt"
      - step_id: "{second_step_id}"
        step_type: "worker_with_gate"
        model: "runner-default"
        thinking_depth: "runner-default"
        worker_agent: "worker-beta"
        gate_agent: "gate-beta"
        expected_inputs:
          - "plans/example/input-b.md"
        created_artifacts:
          - "plans/example/output-b.md"
        on_fail:
          action: "halt"
"#
    )
}

fn parallel_single_pass_review_start_fixture(
    first_member_id: &str,
    second_member_id: &str,
) -> String {
    format!(
        r#"
stages:
  - stage_id: "review"
    steps:
      - step_id: "review-checkers"
        step_type: "parallel_group"
        model: "runner-default"
        thinking_depth: "runner-default"
        members:
          - step_id: "{first_member_id}"
            step_type: "single_pass"
            model: "runner-default"
            thinking_depth: "runner-default"
            worker_agent: "worker-alpha"
            expected_inputs:
              - "plans/example/input-a.md"
            created_artifacts:
              - "plans/example/output-a.md"
            on_pass:
              next_step: "{second_member_id}"
            on_fail:
              action: "record-fail-and-continue-group"
          - step_id: "{second_member_id}"
            step_type: "single_pass"
            model: "runner-default"
            thinking_depth: "runner-default"
            worker_agent: "worker-beta"
            expected_inputs:
              - "plans/example/input-b.md"
            created_artifacts:
              - "plans/example/output-b.md"
            on_pass:
              next_step: "review-consolidate"
            on_fail:
              action: "record-fail-and-continue-group"
        on_pass:
          next_step: "review-consolidate"
        on_fail:
          action: "continue-to-next-step"
          next_step: "review-consolidate"
      - step_id: "review-consolidate"
        step_type: "single_pass"
        model: "runner-default"
        thinking_depth: "runner-default"
        worker_agent: "consolidator"
        expected_inputs:
          - "plans/example/input-a.md"
        created_artifacts:
          - "plans/example/output-c.md"
        on_fail:
          action: "halt"
"#
    )
}

fn structural_group_member_start_fixture(
    structural_member_id: &str,
    executable_member_id: &str,
) -> String {
    format!(
        r#"
stages:
  - stage_id: "review"
    steps:
      - step_id: "review-checkers"
        step_type: "parallel_group"
        model: "runner-default"
        thinking_depth: "runner-default"
        members:
          - step_id: "{structural_member_id}"
            model: "runner-default"
            thinking_depth: "runner-default"
            worker_agent: "worker-structural"
            expected_inputs:
              - "plans/example/input-a.md"
            created_artifacts:
              - "plans/example/output-a.md"
            on_pass:
              next_step: "{executable_member_id}"
            on_fail:
              action: "record-fail-and-continue-group"
          - step_id: "{executable_member_id}"
            step_type: "single_pass"
            model: "runner-default"
            thinking_depth: "runner-default"
            worker_agent: "worker-beta"
            expected_inputs:
              - "plans/example/input-b.md"
            created_artifacts:
              - "plans/example/output-b.md"
            on_pass:
              next_step: "review-consolidate"
            on_fail:
              action: "record-fail-and-continue-group"
        on_pass:
          next_step: "review-consolidate"
        on_fail:
          action: "continue-to-next-step"
          next_step: "review-consolidate"
      - step_id: "review-consolidate"
        step_type: "single_pass"
        model: "runner-default"
        thinking_depth: "runner-default"
        worker_agent: "consolidator"
        expected_inputs:
          - "plans/example/input-a.md"
        created_artifacts:
          - "plans/example/output-c.md"
        on_fail:
          action: "halt"
"#
    )
}

async fn wait_for_event<F>(
    rx: &mut tokio::sync::broadcast::Receiver<DeterministicOrchestratorEvent>,
    predicate: F,
    timeout: Duration,
) -> Option<DeterministicOrchestratorEvent>
where
    F: Fn(&DeterministicOrchestratorEvent) -> bool,
{
    let deadline = Instant::now() + timeout;

    loop {
        let now = Instant::now();
        if now >= deadline {
            return None;
        }

        let remaining = deadline.saturating_duration_since(now);
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(event)) if predicate(&event) => return Some(event),
            Ok(Ok(_)) => {}
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => return None,
            Err(_) => return None,
        }
    }
}

fn subscribe_pair(
    handle: &DeterministicOrchestratorHandle,
) -> (
    tokio::sync::broadcast::Receiver<DeterministicOrchestratorEvent>,
    tokio::sync::broadcast::Receiver<DeterministicOrchestratorEvent>,
) {
    (handle.subscribe(), handle.subscribe())
}

/// Verifies the public runtime handle executes the local workflow structure from
/// `.github/local/plan_execution.yml` rather than any hardcoded step order.
#[tokio::test]
async fn workflow_executes_from_local_yaml_structure() {
    let repo = temp_repo();
    write_repo_file(
        repo.path(),
        ".github/local/plan_execution.yml",
        two_step_workflow_fixture("local-review-start", "local-review-finish").as_str(),
    );
    write_expected_inputs(repo.path());

    let handle = spawn(repo.path().to_path_buf());
    let mut rx = handle.subscribe();
    handle.start(None, None, PipelineResumeMode::StartFresh);

    let started = wait_for_event(
        &mut rx,
        |event| {
            matches!(
                event,
                DeterministicOrchestratorEvent::Started {
                    first_step_id: Some(step_id),
                } if step_id == &WorkflowStepId::from("local-review-start")
            )
        },
        Duration::from_millis(150),
    )
    .await;
    assert!(
        started.is_some(),
        "public runtime startup should begin from the first executable step declared in the local workflow YAML once the declared expected inputs exist",
    );

    handle.shutdown();
}

/// Verifies lowered review-group members with explicit `single_pass` semantics
/// become the first public executable steps instead of remaining structural-only
/// `GroupMember` placeholders.
#[tokio::test]
async fn lowered_single_pass_review_members_start_as_public_executable_steps() {
    let repo = temp_repo();
    write_repo_file(
        repo.path(),
        ".github/local/plan_execution.yml",
        parallel_single_pass_review_start_fixture(
            "review-architecture-check",
            "review-security-check",
        )
        .as_str(),
    );
    write_expected_inputs(repo.path());

    let handle = spawn(repo.path().to_path_buf());
    let mut rx = handle.subscribe();
    handle.start(None, None, PipelineResumeMode::StartFresh);

    let started = wait_for_event(
        &mut rx,
        |event| {
            matches!(
                event,
                DeterministicOrchestratorEvent::Started {
                    first_step_id: Some(step_id),
                } if step_id == &WorkflowStepId::from("review-architecture-check")
            )
        },
        Duration::from_millis(150),
    )
    .await;
    assert!(
        started.is_some(),
        "public runtime startup should expose the first lowered single_pass review member as the executable starting step",
    );

    handle.shutdown();
}

/// Verifies structural-only lowered `GroupMember` entries stay non-executable and
/// do not become the public starting step when a later member declares an
/// explicit executable semantic.
#[tokio::test]
async fn structural_group_members_do_not_become_public_executable_steps() {
    let repo = temp_repo();
    write_repo_file(
        repo.path(),
        ".github/local/plan_execution.yml",
        structural_group_member_start_fixture("review-architecture-check", "review-security-check")
            .as_str(),
    );
    write_expected_inputs(repo.path());

    let handle = spawn(repo.path().to_path_buf());
    let mut rx = handle.subscribe();
    handle.start(None, None, PipelineResumeMode::StartFresh);

    let started = wait_for_event(
        &mut rx,
        |event| {
            matches!(
                event,
                DeterministicOrchestratorEvent::Started {
                    first_step_id: Some(step_id),
                } if step_id == &WorkflowStepId::from("review-security-check")
            )
        },
        Duration::from_millis(150),
    )
    .await;
    assert!(
        started.is_some(),
        "public runtime startup must skip structural-only GroupMember metadata and begin at the first explicit executable member",
    );

    handle.shutdown();
}

/// Verifies an existing local workflow override stays authoritative over the
/// canonical seed file during end-to-end runtime startup.
#[tokio::test]
async fn existing_local_yaml_overrides_canonical_seed() {
    let repo = temp_repo();
    let canonical = two_step_workflow_fixture("canonical-start", "canonical-finish");
    let local = two_step_workflow_fixture("local-override-start", "local-override-finish");
    write_repo_file(
        repo.path(),
        ".github/plan_execution.yml",
        canonical.as_str(),
    );
    write_repo_file(
        repo.path(),
        ".github/local/plan_execution.yml",
        local.as_str(),
    );
    write_expected_inputs(repo.path());

    let handle = spawn(repo.path().to_path_buf());
    let mut rx = handle.subscribe();
    handle.start(None, None, PipelineResumeMode::StartFresh);

    let started = wait_for_event(
        &mut rx,
        |event| {
            matches!(
                event,
                DeterministicOrchestratorEvent::Started {
                    first_step_id: Some(step_id),
                } if step_id == &WorkflowStepId::from("local-override-start")
            )
        },
        Duration::from_millis(150),
    )
    .await;
    assert!(
        started.is_some(),
        "runtime startup should preserve the existing local workflow override instead of reseeding from canonical",
    );

    let local_contents = fs::read_to_string(repo.path().join(".github/local/plan_execution.yml"))
        .expect("local workflow override should remain readable");
    assert_eq!(
        local_contents, local,
        "starting the public runtime must not overwrite an existing local workflow file",
    );

    handle.shutdown();
}

/// Verifies the public handle exposes start, subscribe, and shutdown semantics
/// that are wired to the runtime actor event stream.
#[tokio::test]
async fn public_handle_exposes_start_subscribe_shutdown_semantics() {
    let repo = temp_repo();
    write_repo_file(
        repo.path(),
        ".github/plan_execution.yml",
        two_step_workflow_fixture("public-start", "public-finish").as_str(),
    );
    write_expected_inputs(repo.path());

    let handle = spawn(repo.path().to_path_buf());
    let (mut rx_a, mut rx_b) = subscribe_pair(&handle);

    handle.start(None, None, PipelineResumeMode::StartFresh);

    let started_a = wait_for_event(
        &mut rx_a,
        |event| matches!(event, DeterministicOrchestratorEvent::Started { .. }),
        Duration::from_millis(150),
    )
    .await;
    let started_b = wait_for_event(
        &mut rx_b,
        |event| matches!(event, DeterministicOrchestratorEvent::Started { .. }),
        Duration::from_millis(150),
    )
    .await;

    assert!(
        started_a.is_some() && started_b.is_some(),
        "each public subscriber should observe runtime start after calling start on the handle",
    );

    handle.shutdown();

    let mut post_shutdown_rx = handle.subscribe();
    handle.start(None, None, PipelineResumeMode::StartFresh);

    let restarted_after_shutdown = wait_for_event(
        &mut post_shutdown_rx,
        |event| matches!(event, DeterministicOrchestratorEvent::Started { .. }),
        Duration::from_millis(50),
    )
    .await;
    assert!(
        restarted_after_shutdown.is_none(),
        "shutdown should terminate the public runtime so a later start call does not emit a second start event",
    );
}
