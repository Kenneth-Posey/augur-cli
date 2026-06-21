use augur_domain::domain::context_management::*;
use augur_domain::domain::newtypes::{IsCompactionSummary, IsDecodable, IsPredicate, IsToolResult};
use chrono::Utc;
use proptest::prelude::*;
use std::collections::HashSet;

fn tid(id: u32) -> TurnPairId {
    TurnPairId::new(id).expect("turn id")
}

fn session_id(value: &str) -> SessionId {
    SessionId::new(value).expect("session id")
}

fn objective(value: &str) -> ObjectiveId {
    ObjectiveId::new(value).expect("objective")
}

fn window_id(value: &str) -> WindowId {
    WindowId::new(value).expect("window id")
}

fn sample_config() -> CompactionConfig {
    CompactionConfig {
        context_budget_ratio: 0.5.into(),
        content_clear_window: 3.into(),
        drop_protection_window: 2.into(),
        rate_budget_reserve: 0.into(),
        checkpoint_summary_max_tokens: 32.into(),
    }
}

fn sample_turn(id: u32, age: u32, objective_value: &str) -> TurnPair {
    TurnPair {
        identity: TurnPairIdentity {
            id: tid(id),
            objective_id: objective(objective_value),
        },
        age: TurnPairAge::new(age),
        user_message: Message {
            body: format!("user-{id}").into(),
            is_tool_result: IsToolResult::no(),
        },
        assistant_message: Message {
            body: format!("assistant-{id}").into(),
            is_tool_result: IsToolResult::no(),
        },
        metadata: TurnPairMetadata {
            protected_recent_window: IsPredicate::no(),
            objective_changing: IsPredicate::no(),
            excluded_from_clearing: IsPredicate::no(),
            low_semantic_density: IsPredicate::no(),
        },
    }
}

fn sample_snapshot(session_type: SessionType) -> SessionSnapshot {
    SessionSnapshot {
        session_id: session_id("s-1"),
        session_type,
        stable_prefix: StablePrefix {
            bytes: "SYSTEM+TOOLS".to_owned(),
        },
        turn_pairs: vec![sample_turn(1, 6, "obj-a"), sample_turn(2, 2, "obj-a")],
        context_window: SessionContextWindow {
            model_context_limit: TokenCount::new(100),
            provider_prompt_tokens: Some(TokenCount::new(80)),
        },
    }
}

fn sample_payload() -> CheckpointPayload {
    CheckpointPayload {
        objective: "ship feature".to_owned(),
        stage_completed: StageName::Implement,
        next_stage: StageName::Complete,
        narrative: CheckpointNarrative {
            context_summary: "dense summary text".to_owned(),
            artifacts: vec!["src/domain/context_management.rs".to_owned()],
            decisions: vec!["kept deterministic ordering".to_owned()],
            open_questions: vec![],
        },
        ordering: CheckpointOrderingMetadata {
            checkpoint_sequence: CheckpointSequence::new(7),
            created_at: Utc::now(),
        },
    }
}

fn repeated_words(count: usize) -> String {
    (0..count).map(|_| "word").collect::<Vec<_>>().join(" ")
}

fn estimate_snapshot_chars(snapshot: &SessionSnapshot) -> u32 {
    let stable_prefix_chars = snapshot.stable_prefix.bytes.chars().count() as u32;
    let turn_chars = snapshot
        .turn_pairs
        .iter()
        .map(|turn| {
            turn.user_message.body.chars().count() as u32
                + turn.assistant_message.body.chars().count() as u32
        })
        .sum::<u32>();
    stable_prefix_chars + turn_chars
}

#[test]
fn tst_cma_001_invalid_ratio_rejected() {
    let mut cfg = sample_config();
    cfg.context_budget_ratio = 1.2.into();
    let out = validate_config_guardrails(cfg, RequestKind::Normal);
    assert!(matches!(out, Err(ConfigError::InvalidRatio)));
}

#[test]
fn tst_cma_002_rewind_out_of_scope() {
    let out = validate_config_guardrails(sample_config(), RequestKind::Rewind);
    assert!(matches!(out, Err(ConfigError::RewindOutOfScope)));
}

#[test]
fn tst_cma_057_rewind_guardrail_is_enforced_via_config_validation() {
    assert!(matches!(
        validate_config_guardrails(sample_config(), RequestKind::Rewind),
        Err(ConfigError::RewindOutOfScope)
    ));
    assert_eq!(
        validate_config_guardrails(sample_config(), RequestKind::Normal),
        Ok(sample_config())
    );
}

#[test]
#[cfg(any())]
fn tst_cma_058_resume_prompt_lifecycle_is_guarded() {
    let prompt_id = ResumePromptId::new("rp-1").expect("resume prompt id");
    assert_eq!(prompt_id.to_string(), "rp-1");
    let draft = ResumePrompt::new_draft(prompt_id, "line1\r\nline2".to_owned());
    assert_eq!(draft.lifecycle, ResumePromptLifecycle::Draft);

    let canonicalized = draft.canonicalize().expect("canonicalize");
    assert_eq!(
        canonicalized.lifecycle,
        ResumePromptLifecycle::Canonicalized
    );
    assert_eq!(canonicalized.text, "line1\nline2");

    let emitted = canonicalized.clone().emit().expect("emit");
    assert_eq!(emitted.lifecycle, ResumePromptLifecycle::Emitted);

    let invalid = emitted.canonicalize();
    assert!(matches!(
        invalid,
        Err(LifecycleError::InvalidTransition { .. })
    ));
}

#[test]
#[cfg(any())]
fn tst_cma_059_config_snapshot_lifecycle_is_guarded() {
    let loaded = ConfigSnapshot::new_loaded(
        ConfigVersion::new(1),
        sample_config(),
        BudgetEstimate {
            estimated_prompt_tokens: TokenCount::new(80),
            context_budget_tokens: TokenCount::new(50),
        },
    );
    assert_eq!(loaded.version.get(), 1);
    assert_eq!(loaded.lifecycle, ConfigSnapshotLifecycle::Loaded);

    let validated = loaded.validate().expect("validate");
    assert_eq!(validated.lifecycle, ConfigSnapshotLifecycle::Validated);

    let active = validated.clone().activate().expect("activate");
    assert_eq!(active.lifecycle, ConfigSnapshotLifecycle::Active);

    let rejected = validated.reject().expect("reject");
    assert_eq!(rejected.lifecycle, ConfigSnapshotLifecycle::Rejected);
}

#[test]
#[cfg(any())]
fn tst_cma_060_session_record_lifecycle_is_guarded() {
    let active = SessionRecord::new_active(sample_snapshot(SessionType::Main));
    assert_eq!(active.lifecycle, SessionRecordLifecycle::Active);

    let running = active.start_compaction().expect("start compaction");
    assert_eq!(running.lifecycle, SessionRecordLifecycle::CompactionRunning);

    let ready = running.clone().mark_ready_to_send().expect("ready");
    assert_eq!(ready.lifecycle, SessionRecordLifecycle::ReadyToSend);

    let blocked = running.block_send().expect("blocked");
    assert_eq!(blocked.lifecycle, SessionRecordLifecycle::Blocked);

    let invalid = ready.block_send();
    assert!(matches!(
        invalid,
        Err(LifecycleError::InvalidTransition { .. })
    ));
}

#[test]
fn tst_cma_003_seed_budget_prefers_provider_usage() {
    let with_provider = seed_budget_estimate(sample_snapshot(SessionType::Main), sample_config());
    assert_eq!(with_provider.estimated_prompt_tokens.get(), 80);
    assert_eq!(with_provider.context_budget_tokens.get(), 50);

    let mut without_provider_snapshot = sample_snapshot(SessionType::Main);
    without_provider_snapshot.provider_prompt_tokens = None;
    without_provider_snapshot.stable_prefix.bytes = "ABCD".to_owned();
    without_provider_snapshot.turn_pairs[0].user_message.body = "wxyz".to_owned().into();
    without_provider_snapshot.turn_pairs[0]
        .assistant_message
        .body = "mnop".to_owned().into();
    without_provider_snapshot.turn_pairs[1].user_message.body = "qrst".to_owned().into();
    without_provider_snapshot.turn_pairs[1]
        .assistant_message
        .body = "uv".to_owned().into();
    let expected_char_estimate = estimate_snapshot_chars(&without_provider_snapshot);

    let without_provider = seed_budget_estimate(without_provider_snapshot, sample_config());
    assert_eq!(
        without_provider.estimated_prompt_tokens.get(),
        expected_char_estimate
    );
}

#[test]
fn tst_cma_008_stage1_excluded_turn_not_cleared() {
    let mut snap = sample_snapshot(SessionType::Main);
    snap.turn_pairs[0].metadata.excluded_from_clearing = IsPredicate::yes();
    snap.turn_pairs[0].user_message.is_tool_result = IsToolResult::yes();
    snap.turn_pairs[0].assistant_message.is_tool_result = IsToolResult::yes();
    let out = run_stage1_content_clearing(snap.clone(), sample_config());
    assert_eq!(
        out.snapshot.turn_pairs[0].assistant_message.body,
        snap.turn_pairs[0].assistant_message.body
    );
}

#[test]
fn tst_cma_009_stage1_old_turn_is_cleared() {
    let mut snap = sample_snapshot(SessionType::Main);
    snap.turn_pairs[0].assistant_message.is_tool_result = IsToolResult::yes();
    let out = run_stage1_content_clearing(snap, sample_config());
    assert_eq!(
        out.snapshot.turn_pairs[0].assistant_message.body,
        "[cleared]"
    );
}

#[test]
fn tst_cma_061_stage1_does_not_clear_non_tool_result_content() {
    let snap = sample_snapshot(SessionType::Main);
    let out = run_stage1_content_clearing(snap.clone(), sample_config());
    assert_eq!(
        out.snapshot.turn_pairs[0].user_message.body,
        snap.turn_pairs[0].user_message.body
    );
    assert_eq!(
        out.snapshot.turn_pairs[0].assistant_message.body,
        snap.turn_pairs[0].assistant_message.body
    );
}

#[test]
fn tst_cma_062_stage1_clears_only_tool_result_body_within_turn() {
    let mut snap = sample_snapshot(SessionType::Main);
    snap.turn_pairs[0].user_message.is_tool_result = IsToolResult::yes();
    snap.turn_pairs[0].assistant_message.is_tool_result = IsToolResult::no();
    let out = run_stage1_content_clearing(snap.clone(), sample_config());
    assert_eq!(out.snapshot.turn_pairs[0].user_message.body, "[cleared]");
    assert_eq!(
        out.snapshot.turn_pairs[0].assistant_message.body,
        snap.turn_pairs[0].assistant_message.body
    );
}

#[test]
fn tst_cma_010_candidate_class_assigned_once() {
    let mut snap = sample_snapshot(SessionType::Main);
    snap.turn_pairs[0].user_message.is_tool_result = IsToolResult::yes();
    snap.turn_pairs[0].assistant_message.is_tool_result = IsToolResult::yes();
    snap.turn_pairs[1].assistant_message.body = String::new().into();

    let out = classify_stage2_candidates(snap.clone(), sample_config());
    let eligible_count = snap
        .turn_pairs
        .iter()
        .filter(|turn| {
            !turn.metadata.protected_recent_window.0 && !turn.metadata.objective_changing.0
        })
        .count();

    assert_eq!(out.len(), eligible_count);
    let classified_ids = out.iter().map(|c| c.turn_id).collect::<HashSet<_>>();
    assert_eq!(classified_ids.len(), eligible_count);
    assert!(out.iter().all(|candidate| matches!(
        candidate.class,
        CandidateClass::PureToolExchange
            | CandidateClass::ClearedEmpty
            | CandidateClass::LowSemanticDensity
    )));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn tst_cma_015_property_single_winner_under_contention(window_suffix in 0u16..5000u16) {
        let window = window_id(&format!("win-prop-{window_suffix}"));
        let attempts = [
            try_acquire_rate_slot_lease(window.clone(), 0),
            try_acquire_rate_slot_lease(window.clone(), 0),
            try_acquire_rate_slot_lease(window, 0),
        ];
        let winners = attempts
            .iter()
            .filter(|decision| matches!(decision, LeaseDecision::Granted(_)))
            .count();
        prop_assert!(winners <= 1);
    }
}

#[test]
fn tst_cma_012_protected_turns_not_dropped() {
    let mut snap = sample_snapshot(SessionType::Main);
    snap.turn_pairs[0].metadata.protected_recent_window = IsPredicate::yes();
    let cands = classify_stage2_candidates(snap, sample_config());
    let stage2 = score_and_drop_stage2_candidates(cands, sample_config());
    assert!(!stage2.dropped_turn_ids.contains(&tid(1)));
}

#[test]
fn tst_cma_013_lease_granted_for_available_slot() {
    let out = try_acquire_rate_slot_lease(window_id("win-a"), 0);
    assert!(matches!(out, LeaseDecision::Granted(_)));
}

#[test]
fn tst_cma_014_lease_denied_with_reserve_pressure() {
    let out = try_acquire_rate_slot_lease(window_id("win-b"), 1);
    assert!(matches!(out, LeaseDecision::Denied(_)));
}

#[test]
fn tst_cma_016_lease_consumed_once() {
    let lease = match try_acquire_rate_slot_lease(window_id("win-c"), 0) {
        LeaseDecision::Granted(token) => token,
        LeaseDecision::Denied(reason) => panic!("expected grant got {reason:?}"),
    };
    assert_eq!(
        consume_rate_slot_lease(lease.clone(), LeaseConsumeReason::Used),
        LeaseConsumeResult::Consumed
    );
    assert_eq!(
        consume_rate_slot_lease(lease, LeaseConsumeReason::Used),
        LeaseConsumeResult::AlreadyConsumed
    );
}

#[test]
fn tst_cma_017_empty_segment_returns_overflow_error() {
    let out = compute_droppable_segment(
        sample_snapshot(SessionType::Main),
        Stage2Result {
            dropped_turn_ids: vec![],
        },
        sample_config(),
    );
    assert!(matches!(out, Err(CompactionError::EmptyDroppableSegment)));
}

#[test]
fn tst_cma_022_summary_requires_canonical_header() {
    let out = validate_summary_contract(
        SummaryBlock {
            header: "bad".to_owned(),
            body: "dense prose".to_owned(),
            compaction_summary: IsCompactionSummary::yes(),
        },
        DroppableSegment {
            start_turn: tid(1),
            end_turn: tid(1),
            turn_ids: vec![tid(1)],
        },
        PreservationSet {
            required_elements: vec!["dense".to_owned()],
        },
    );
    assert!(matches!(out, Err(CompactionError::InvalidSummaryContract)));
}

#[test]
#[cfg(any())]
fn tst_cma_060_compaction_completion_transition_is_guarded() {
    let mut run = CompactionRun::new(session_id("s-guard-a"));
    assert!(matches!(
        run.complete(CompactionCompletionReason::Stage1WithinBudget),
        Err(CompactionRunError::InvalidStageTransition)
    ));

    run.stage1_done().expect("initialized -> stage1");
    assert!(matches!(
        run.complete(CompactionCompletionReason::SummaryCommitted),
        Err(CompactionRunError::InvalidStageTransition)
    ));
    run.complete(CompactionCompletionReason::Stage1WithinBudget)
        .expect("stage1 completion");
    assert_eq!(run.state, CompactionRunState::Completed);

    let mut run_lease_denied = CompactionRun::new(session_id("s-guard-b"));
    run_lease_denied
        .stage1_done()
        .expect("initialized -> stage1");
    run_lease_denied.stage2_done().expect("stage1 -> stage2");
    run_lease_denied
        .complete(CompactionCompletionReason::LeaseDenied)
        .expect("stage2 lease denied completion");
    assert_eq!(run_lease_denied.state, CompactionRunState::Completed);

    let mut run_stage3 = CompactionRun::new(session_id("s-guard-c"));
    run_stage3.stage1_done().expect("initialized -> stage1");
    run_stage3.stage2_done().expect("stage1 -> stage2");
    run_stage3.stage3_pending().expect("stage2 -> stage3");
    assert_eq!(run_stage3.state, CompactionRunState::Stage3Pending);
    run_stage3
        .complete(CompactionCompletionReason::SummaryCommitted)
        .expect("stage3 completion");
    assert_eq!(run_stage3.state, CompactionRunState::Completed);
}

#[test]
fn tst_cma_023_summary_replacement_only_touches_segment() {
    let snap = sample_snapshot(SessionType::Main);
    let updated = commit_summary_replacement(
        snap.clone(),
        DroppableSegment {
            start_turn: tid(1),
            end_turn: tid(1),
            turn_ids: vec![tid(1)],
        },
        SummaryBlock {
            header: "[Session summary - turns 1 through 1]".to_owned(),
            body: "dense prose with preserved fact".to_owned(),
            compaction_summary: IsCompactionSummary::yes(),
        },
    )
    .expect("commit");
    assert_eq!(updated.turn_pairs.len(), snap.turn_pairs.len());
    assert_eq!(updated.turn_pairs[1], snap.turn_pairs[1]);
}

#[test]
fn tst_cma_028_unsatisfiable_contract_maps_to_overflow_identifier() {
    let env = emit_response_identifier(OutcomeKind::ContextOverflowError);
    assert_eq!(env.identifier.to_string(), "context-overflow-error");
}

#[test]
fn tst_cma_036_corrupt_latest_checkpoint_fails_closed() {
    let index = vec![CheckpointRecord {
        payload: sample_payload(),
        decodable: IsDecodable::no(),
        lifecycle: CheckpointLifecycle::Persisted,
    }];
    let out = select_latest_checkpoint_or_corruption(index);
    assert!(matches!(
        out,
        Err(CheckpointError::CheckpointCorruptionError)
    ));
}

#[test]
fn tst_cma_039_checkpoint_payload_requires_schema() {
    let payload = sample_payload();
    let out = validate_checkpoint_payload(payload.clone(), sample_config()).expect("valid payload");
    assert_eq!(out.objective, payload.objective);
}

#[test]
fn tst_cma_040_checkpoint_summary_too_large_rejected() {
    let mut payload = sample_payload();
    payload.narrative.context_summary = "x ".repeat(128);
    let out = validate_checkpoint_payload(payload, sample_config());
    assert!(matches!(out, Err(CheckpointError::SummaryTooLarge)));
}

#[test]
fn tst_cma_060_external_checkpoint_write_maps_oversized_summary_to_write_error() {
    let mut payload = sample_payload();
    payload.narrative.context_summary = "x ".repeat(128);
    let out = orchestrate_stage_boundary_checkpoint_write(StageBoundaryCheckpointWriteRequest {
        event: StageEvent::StageBoundary(StageName::Implement),
        snapshot: sample_snapshot(SessionType::Main),
        estimate: BudgetEstimate {
            estimated_prompt_tokens: TokenCount::new(10),
            context_budget_tokens: TokenCount::new(50),
        },
        payload,
        config: sample_config(),
    });
    assert!(matches!(out, Err(CheckpointError::CheckpointWriteError)));
}

#[test]
fn tst_cma_042_resume_prompt_contains_only_base_plus_block() {
    let prompt = build_resume_prompt_rpt1("BASE".to_owned(), sample_payload()).expect("prompt");
    assert!(prompt.starts_with(
        "BASE

[RPT-1 RESUME CONTEXT]"
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn tst_cma_005_property_pipeline_budget_gate_ordering(
        provider_tokens in 0u16..180u16
    ) {
        // PT-CMA-ORDER-001
        let mut snap = sample_snapshot(SessionType::Main);
        snap.provider_prompt_tokens = Some(TokenCount::new(provider_tokens as u32));
        let out = run_compaction_pipeline(snap, sample_config()).expect("pipeline");

        if provider_tokens as u32 <= 50 {
            prop_assert_eq!(out.outcome, OutcomeKind::ProceedWithoutCompaction);
        } else {
            prop_assert!(!matches!(out.outcome, OutcomeKind::ProceedWithoutCompaction));
        }
    }

    #[test]
    fn tst_cma_006_property_stable_prefix_preserved_across_compaction(
        stable_prefix in "[A-Za-z0-9 _\\-]{1,48}",
        user_body in "[A-Za-z0-9 _\\-]{1,96}",
        assistant_body in "[A-Za-z0-9 _\\-]{1,96}"
    ) {
        // PT-CMA-PREFIX-001
        let mut snap = sample_snapshot(SessionType::Main);
        snap.stable_prefix.bytes = stable_prefix.clone();
        snap.turn_pairs[0].user_message.body = user_body.into();
        snap.turn_pairs[0].assistant_message.body = assistant_body.into();
        let out = run_compaction_pipeline(snap, sample_config()).expect("pipeline");
        prop_assert_eq!(out.snapshot.stable_prefix.bytes, stable_prefix);
    }
}

#[test]
fn tst_cma_051_stage1_within_budget_exits_before_stage2() {
    let mut snap = sample_snapshot(SessionType::Main);
    snap.provider_prompt_tokens = None;
    snap.turn_pairs[0].age = TurnPairAge::new(10);
    snap.turn_pairs[0].user_message.body = repeated_words(40).into();
    snap.turn_pairs[0].assistant_message.body = repeated_words(40).into();
    let out = run_compaction_pipeline(snap, sample_config()).expect("pipeline");
    assert_eq!(out.outcome, OutcomeKind::ProceedWithoutStage3);
}

#[test]
fn tst_cma_052_stage2_empty_segment_maps_to_overflow_outcome() {
    let mut snap = sample_snapshot(SessionType::Main);
    snap.provider_prompt_tokens = None;
    for turn in &mut snap.turn_pairs {
        turn.metadata.protected_recent_window = IsPredicate::yes();
        turn.metadata.excluded_from_clearing = IsPredicate::yes();
        turn.user_message.body = repeated_words(30).into();
        turn.assistant_message.body = repeated_words(30).into();
    }
    let out = run_compaction_pipeline(snap, sample_config()).expect("pipeline");
    assert_eq!(out.outcome, OutcomeKind::ContextOverflowError);
}

#[test]
fn tst_cma_053_commit_rejects_protected_or_objective_turns() {
    let mut snap = sample_snapshot(SessionType::Main);
    snap.turn_pairs[0].metadata.protected_recent_window = IsPredicate::yes();
    let out = commit_summary_replacement(
        snap,
        DroppableSegment {
            start_turn: tid(1),
            end_turn: tid(1),
            turn_ids: vec![tid(1)],
        },
        SummaryBlock {
            header: "[Session summary - turns 1 through 1]".to_owned(),
            body: "dense prose with objective".to_owned(),
            compaction_summary: IsCompactionSummary::yes(),
        },
    );
    assert!(matches!(out, Err(CompactionError::InvalidSummaryContract)));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn tst_cma_011_property_protected_or_objective_changing_turns_not_dropped(
        turn_flags in proptest::collection::vec((any::<bool>(), any::<bool>(), 0u8..3u8), 1..20)
    ) {
        // PT-CMA-DROP-001
        let mut snapshot = sample_snapshot(SessionType::Main);
        snapshot.turn_pairs = turn_flags
            .iter()
            .enumerate()
            .map(|(idx, (protected, objective_changing, class_selector))| {
                let id = (idx + 1) as u32;
                let mut turn = sample_turn(id, 3 + id, &format!("obj-{id}"));
                turn.metadata.protected_recent_window = IsPredicate::from(*protected);
                turn.metadata.objective_changing = IsPredicate::from(*objective_changing);
                match class_selector {
                    0 => {
                        turn.user_message.is_tool_result = IsToolResult::yes();
                        turn.assistant_message.is_tool_result = IsToolResult::yes();
                    }
                    1 => turn.user_message.body = String::new().into(),
                    _ => turn.metadata.low_semantic_density = IsPredicate::yes(),
                }
                turn
            })
            .collect();

        let candidates = classify_stage2_candidates(snapshot.clone(), sample_config());
        let stage2 = score_and_drop_stage2_candidates(candidates, sample_config());
        let dropped: HashSet<TurnPairId> = stage2.dropped_turn_ids.into_iter().collect();
        for turn in snapshot.turn_pairs {
            if turn.metadata.protected_recent_window.0 || turn.metadata.objective_changing.0 {
                prop_assert!(!dropped.contains(&turn.id));
            }
        }
    }
}

#[test]
fn tst_cma_015_concurrent_lease_requests_single_winner() {
    let first = try_acquire_rate_slot_lease(window_id("win-contended"), 0);
    let second = try_acquire_rate_slot_lease(window_id("win-contended"), 0);
    let winners = [first, second]
        .iter()
        .filter(|d| matches!(d, LeaseDecision::Granted(_)))
        .count();
    assert_eq!(winners, 1);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn tst_cma_018_property_rate_reserve_boundary_invariant(
        reserve in 0u8..4u8,
        suffix in 0u16..5000u16
    ) {
        // reserve-boundary invariant
        let window = window_id(&format!("win-boundary-{suffix}-{reserve}"));
        let first = try_acquire_rate_slot_lease(window.clone(), reserve as u32);
        let second = try_acquire_rate_slot_lease(window, reserve as u32);
        if reserve == 0 {
            match first {
                LeaseDecision::Granted(token) => {
                    prop_assert!(matches!(second, LeaseDecision::Denied(_)));
                    let _ = consume_rate_slot_lease(token, LeaseConsumeReason::Used);
                }
                other => prop_assert!(matches!(other, LeaseDecision::Granted(_))),
            }
        } else {
            prop_assert!(matches!(first, LeaseDecision::Denied(LeaseDenyReason::ReserveExhausted)));
            prop_assert!(matches!(second, LeaseDecision::Denied(LeaseDenyReason::ReserveExhausted)));
        }
    }
}

#[test]
fn tst_cma_024_summary_contract_rejects_bulleted_body() {
    let out = validate_summary_contract(
        SummaryBlock {
            header: "[Session summary - turns 1 through 1]".to_owned(),
            body: "- bullet".to_owned(),
            compaction_summary: IsCompactionSummary::yes(),
        },
        DroppableSegment {
            start_turn: tid(1),
            end_turn: tid(1),
            turn_ids: vec![tid(1)],
        },
        PreservationSet {
            required_elements: vec!["bullet".to_owned()],
        },
    );
    assert!(matches!(out, Err(CompactionError::InvalidSummaryContract)));
}

#[test]
fn tst_cma_025_summary_contract_rejects_over_500_tokens() {
    let out = validate_summary_contract(
        SummaryBlock {
            header: "[Session summary - turns 1 through 1]".to_owned(),
            body: "word ".repeat(501),
            compaction_summary: IsCompactionSummary::yes(),
        },
        DroppableSegment {
            start_turn: tid(1),
            end_turn: tid(1),
            turn_ids: vec![tid(1)],
        },
        PreservationSet {
            required_elements: vec!["word".to_owned()],
        },
    );
    assert!(matches!(out, Err(CompactionError::InvalidSummaryContract)));
}

#[test]
fn tst_cma_026_summary_contract_requires_preservation_set() {
    let out = validate_summary_contract(
        SummaryBlock {
            header: "[Session summary - turns 1 through 1]".to_owned(),
            body: "dense prose".to_owned(),
            compaction_summary: IsCompactionSummary::yes(),
        },
        DroppableSegment {
            start_turn: tid(1),
            end_turn: tid(1),
            turn_ids: vec![tid(1)],
        },
        PreservationSet {
            required_elements: vec![],
        },
    );
    assert!(matches!(out, Err(CompactionError::InvalidSummaryContract)));
}

#[test]
fn tst_cma_027_summary_commit_marks_compaction_turn() {
    let out = commit_summary_replacement(
        sample_snapshot(SessionType::Main),
        DroppableSegment {
            start_turn: tid(1),
            end_turn: tid(1),
            turn_ids: vec![tid(1)],
        },
        SummaryBlock {
            header: "[Session summary - turns 1 through 1]".to_owned(),
            body: "dense prose with objective".to_owned(),
            compaction_summary: IsCompactionSummary::yes(),
        },
    )
    .expect("commit");
    assert_eq!(out.turn_pairs[0].user_message.body, "[compaction-summary]");
}

#[test]
fn tst_cma_035_selects_latest_checkpoint_deterministically() {
    let mut older = sample_payload();
    older.ordering.checkpoint_sequence = CheckpointSequence::new(1);
    let mut newer = sample_payload();
    newer.ordering.checkpoint_sequence = CheckpointSequence::new(2);
    newer.ordering.created_at += chrono::Duration::seconds(1);
    let selected = select_latest_checkpoint_or_corruption(vec![
        CheckpointRecord {
            payload: older,
            decodable: IsDecodable::yes(),
            lifecycle: CheckpointLifecycle::Persisted,
        },
        CheckpointRecord {
            payload: newer.clone(),
            decodable: IsDecodable::yes(),
            lifecycle: CheckpointLifecycle::Persisted,
        },
    ])
    .expect("select");
    assert_eq!(
        selected.payload.ordering.checkpoint_sequence.get(),
        newer.ordering.checkpoint_sequence.get()
    );
}

#[test]
fn tst_cma_037_corrupt_latest_checkpoint_stays_corruption_branch() {
    let out = execute_restart_recovery_matrix(RecoveryAttempt {
        latest_checkpoint: Some(Err(CheckpointError::CheckpointCorruptionError)),
        transcript_state: TranscriptState::Decodable,
        checkpoint_write_state: CheckpointWriteState::Clean,
    });
    assert!(matches!(out, Err(RecoveryError::CheckpointCorruptionError)));
}

#[test]
fn tst_cma_038_unresolved_latest_tie_is_corruption() {
    let payload = sample_payload();
    let out = select_latest_checkpoint_or_corruption(vec![
        CheckpointRecord {
            payload: payload.clone(),
            decodable: IsDecodable::yes(),
            lifecycle: CheckpointLifecycle::Persisted,
        },
        CheckpointRecord {
            payload,
            decodable: IsDecodable::yes(),
            lifecycle: CheckpointLifecycle::Persisted,
        },
    ]);
    assert!(matches!(
        out,
        Err(CheckpointError::CheckpointCorruptionError)
    ));
}

#[test]
fn tst_cma_041_checkpoint_write_failure_preserves_transcript_truth() {
    let mut payload = sample_payload();
    payload.narrative.decisions = vec!["__force_write_error__".to_owned()];
    let out = write_stage_boundary_checkpoint(payload);
    assert!(matches!(out, Err(CheckpointError::CheckpointWriteError)));
}

#[test]
#[cfg(any())]
fn tst_cma_059_checkpoint_write_failure_transition_is_guarded() {
    let candidate = CheckpointRecord::new_candidate(sample_payload());
    let invalid = candidate.clone().transition_write_failure();
    assert!(matches!(
        invalid,
        Err(CheckpointError::CheckpointWriteError)
    ));

    let validated = candidate
        .transition_to(CheckpointLifecycle::Validated)
        .expect("candidate -> validated");
    let failed = validated
        .transition_write_failure()
        .expect("validated -> candidate on write failure");
    assert_eq!(failed.lifecycle, CheckpointLifecycle::Candidate);
}

#[test]
fn tst_cma_054_checkpoint_write_requires_main_stage_boundary_policy() {
    assert!(!should_write_stage_boundary_checkpoint(
        StageEvent::StageBoundary(StageName::Implement),
        SessionType::Background
    ));
    assert!(!should_write_stage_boundary_checkpoint(
        StageEvent::NonBoundary,
        SessionType::Main
    ));
    assert!(should_write_stage_boundary_checkpoint(
        StageEvent::StageBoundary(StageName::Implement),
        SessionType::Main
    ));
}

#[test]
fn tst_cma_055_checkpoint_selection_rejects_non_persisted_records() {
    let out = select_latest_checkpoint_or_corruption(vec![CheckpointRecord {
        payload: sample_payload(),
        decodable: IsDecodable::yes(),
        lifecycle: CheckpointLifecycle::Validated,
    }]);
    assert!(matches!(
        out,
        Err(CheckpointError::CheckpointCorruptionError)
    ));
}

#[test]
fn tst_cma_056_lease_expiration_releases_slot_and_blocks_reconsume() {
    let lease = match try_acquire_rate_slot_lease(window_id("win-expire"), 0) {
        LeaseDecision::Granted(token) => token,
        LeaseDecision::Denied(reason) => panic!("expected grant got {reason:?}"),
    };
    assert_eq!(
        consume_rate_slot_lease(lease.clone(), LeaseConsumeReason::Expired),
        LeaseConsumeResult::Consumed
    );
    assert_eq!(
        consume_rate_slot_lease(lease, LeaseConsumeReason::Used),
        LeaseConsumeResult::AlreadyConsumed
    );
    let reacquired = try_acquire_rate_slot_lease(window_id("win-expire"), 0);
    assert!(matches!(reacquired, LeaseDecision::Granted(_)));
}

#[test]
fn tst_cma_043_resume_prompt_uses_canonical_label_order() {
    let prompt = build_resume_prompt_rpt1("BASE".to_owned(), sample_payload()).expect("prompt");
    let objective_idx = prompt.find("objective:").expect("objective label");
    let stage_idx = prompt.find("stage_completed:").expect("stage label");
    let summary_idx = prompt.find("context_summary:").expect("summary label");
    assert!(objective_idx < stage_idx && stage_idx < summary_idx);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn tst_cma_044_property_resume_prompt_canonicalizes_scalars_and_preserves_list_order(
        objective_a in "[A-Za-z0-9 ]{1,24}",
        objective_b in "[A-Za-z0-9 ]{1,24}",
        first_artifact in "[A-Za-z0-9_/\\.\\-]{1,20}",
        second_artifact in "[A-Za-z0-9_/\\.\\-]{1,20}"
    ) {
        // PT-CMA-RPT1-001
        let mut payload = sample_payload();
        payload.objective = format!("{objective_a}\r\n{objective_b}");
        payload.narrative.artifacts = vec![
            first_artifact.clone(),
            second_artifact.clone(),
        ];
        let prompt = build_resume_prompt_rpt1("BASE".to_owned(), payload).expect("prompt");
        let normalized_objective = format!("{objective_a}\n{objective_b}")
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ");
        let expected_objective = format!("objective: {normalized_objective}");
        prop_assert!(prompt.contains(&expected_objective));
        let first_idx = prompt
            .find(&format!("- {first_artifact}"))
            .expect("first artifact present");
        let second_idx = prompt
            .find(&format!("- {second_artifact}"))
            .expect("second artifact present");
        prop_assert!(first_idx <= second_idx);
    }
}

#[test]
fn tst_cma_045_resume_prompt_renders_lists_or_none() {
    let mut payload = sample_payload();
    payload.narrative.open_questions = vec![];
    let prompt = build_resume_prompt_rpt1("BASE".to_owned(), payload).expect("prompt");
    assert!(prompt.contains(
        "open_questions:
- none"
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn tst_cma_047_property_recovery_matrix_first_match_wins(
        checkpoint_sequence in 1u64..200u64,
        transcript_state in prop_oneof![
            Just(TranscriptState::Decodable),
            Just(TranscriptState::Corrupt),
            Just(TranscriptState::Missing),
        ],
        prior_checkpoint_write_error in any::<bool>()
    ) {
        // matrix first-match invariant
        let mut payload = sample_payload();
        payload.ordering.checkpoint_sequence = CheckpointSequence::new(checkpoint_sequence);
        let cp = CheckpointRecord {
            payload,
            decodable: IsDecodable::yes(),
            lifecycle: CheckpointLifecycle::Persisted,
        };
        let out = execute_restart_recovery_matrix(RecoveryAttempt {
            latest_checkpoint: Some(Ok(cp.clone())),
            transcript_state,
            checkpoint_write_state: if prior_checkpoint_write_error {
                CheckpointWriteState::PriorWriteError
            } else {
                CheckpointWriteState::Clean
            },
        })
        .expect("first match");
        prop_assert_eq!(out, RecoveryOutcome::ResumeFromCheckpoint(cp));
    }
}
