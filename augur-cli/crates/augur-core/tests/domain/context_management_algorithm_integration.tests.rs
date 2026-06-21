use augur_domain::domain::context_management::*;
use chrono::Utc;

fn tid(id: u32) -> TurnPairId {
    TurnPairId::new(id).expect("turn id")
}

fn session_id(value: &str) -> SessionId {
    SessionId::new(value).expect("session id")
}

fn objective(value: &str) -> ObjectiveId {
    ObjectiveId::new(value).expect("objective")
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
            is_tool_result: false.into(),
        },
        assistant_message: Message {
            body: format!("assistant-{id}").into(),
            is_tool_result: false.into(),
        },
        metadata: TurnPairMetadata {
            protected_recent_window: false.into(),
            objective_changing: false.into(),
            excluded_from_clearing: false.into(),
            low_semantic_density: false.into(),
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

#[test]
fn tst_cma_004_integration_within_budget_skips_compaction_pipeline() {
    let mut snap = sample_snapshot(SessionType::Main);
    snap.provider_prompt_tokens = Some(TokenCount::new(10));
    let out = run_compaction_pipeline(snap.clone(), sample_config()).expect("pipeline result");
    assert_eq!(out.outcome, OutcomeKind::ProceedWithoutCompaction);
    assert_eq!(
        emit_response_identifier(out.outcome).identifier.to_string(),
        "proceed-without-compaction"
    );
    assert_eq!(out.snapshot.turn_pairs, snap.turn_pairs);
}

#[test]
fn tst_cma_007_integration_post_stage2_within_budget_skips_stage3() {
    let mut snap = sample_snapshot(SessionType::Main);
    snap.provider_prompt_tokens = Some(TokenCount::new(60));
    let out = run_compaction_pipeline(snap, sample_config()).expect("pipeline");
    assert_eq!(out.outcome, OutcomeKind::ProceedWithoutStage3);
    assert_eq!(
        emit_response_identifier(out.outcome).identifier.to_string(),
        "proceed-without-stage3"
    );
}

#[test]
fn tst_cma_019_integration_summary_commit_path_can_proceed() {
    let segment = DroppableSegment {
        start_turn: tid(1),
        end_turn: tid(1),
        turn_ids: vec![tid(1)],
    };
    let summary = generate_stage3_summary(SummaryRequest {
        segment: segment.clone(),
        preservation_set: PreservationSet {
            required_elements: vec!["objective".to_owned()],
        },
    })
    .expect("summary generation");
    let validated = validate_summary_contract(
        summary,
        segment.clone(),
        PreservationSet {
            required_elements: vec!["objective".to_owned()],
        },
    )
    .expect("summary validation");
    let committed =
        commit_summary_replacement(sample_snapshot(SessionType::Main), segment, validated)
            .expect("summary commit");
    assert_eq!(
        committed.turn_pairs[0].user_message.body,
        "[compaction-summary]"
    );
}

#[test]
fn tst_cma_020_integration_overflow_identifier_emits_context_overflow() {
    let mut snap = sample_snapshot(SessionType::Main);
    for turn in &mut snap.turn_pairs {
        turn.metadata.protected_recent_window = true.into();
        turn.metadata.excluded_from_clearing = true.into();
        turn.user_message.body = "word ".repeat(40).into();
        turn.assistant_message.body = "word ".repeat(40).into();
    }
    snap.provider_prompt_tokens = None;
    let out = run_compaction_pipeline(snap, sample_config()).expect("pipeline");
    assert_eq!(out.outcome, OutcomeKind::ContextOverflowError);
    assert_eq!(
        emit_response_identifier(out.outcome).identifier.to_string(),
        "context-overflow-error"
    );
}

#[test]
fn tst_cma_021_integration_generation_error_maps_to_response_identifier() {
    let out = generate_stage3_summary(SummaryRequest {
        segment: DroppableSegment {
            start_turn: tid(1),
            end_turn: tid(1),
            turn_ids: vec![],
        },
        preservation_set: PreservationSet {
            required_elements: vec!["objective".to_owned()],
        },
    });
    assert!(matches!(out, Err(CompactionError::SummaryGenerationError)));
    assert_eq!(
        emit_response_identifier(OutcomeKind::SummaryGenerationError)
            .identifier
            .to_string(),
        "summary-generation-error"
    );
}

#[test]
fn tst_cma_029_integration_background_within_budget_can_send() {
    let decision = evaluate_background_policy(
        sample_snapshot(SessionType::Background),
        BudgetEstimate {
            estimated_prompt_tokens: TokenCount::new(10),
            context_budget_tokens: TokenCount::new(50),
        },
    );
    assert!(decision.should_send_request);
    assert_eq!(decision.outcome, OutcomeKind::ProceedWithoutStage3);
}

#[test]
fn tst_cma_030_integration_main_over_budget_is_not_background_blocked() {
    let decision = evaluate_background_policy(
        sample_snapshot(SessionType::Main),
        BudgetEstimate {
            estimated_prompt_tokens: TokenCount::new(90),
            context_budget_tokens: TokenCount::new(50),
        },
    );
    assert!(decision.should_send_request);
    assert_eq!(decision.outcome, OutcomeKind::ProceedWithoutStage3);
}

#[test]
fn tst_cma_031_integration_background_over_budget_warns_and_blocks_send() {
    let decision = evaluate_background_policy(
        sample_snapshot(SessionType::Background),
        BudgetEstimate {
            estimated_prompt_tokens: TokenCount::new(90),
            context_budget_tokens: TokenCount::new(50),
        },
    );
    assert!(!decision.should_send_request);
    assert_eq!(decision.outcome, OutcomeKind::ContextPressureWarning);
}

#[test]
fn tst_cma_032_integration_background_at_budget_can_send() {
    let decision = evaluate_background_policy(
        sample_snapshot(SessionType::Background),
        BudgetEstimate {
            estimated_prompt_tokens: TokenCount::new(50),
            context_budget_tokens: TokenCount::new(50),
        },
    );
    assert!(decision.should_send_request);
    assert_eq!(decision.outcome, OutcomeKind::ProceedWithoutStage3);
}

#[test]
fn tst_cma_033_integration_stage_boundary_checkpoint_write_succeeds() {
    let payload = sample_payload();
    assert!(matches!(
        should_write_stage_boundary_checkpoint(
            StageEvent::StageBoundary(StageName::Implement),
            SessionType::Main
        ),
        StageBoundaryCheckpointPolicy::Write
    ));
    let validated =
        validate_checkpoint_payload(payload.clone(), sample_config()).expect("validate");
    let record = write_stage_boundary_checkpoint(validated).expect("write");
    assert_eq!(
        payload.ordering.checkpoint_sequence.get(),
        record.payload.ordering.checkpoint_sequence.get()
    );
    assert_eq!(record.lifecycle, CheckpointLifecycle::Persisted);
}

#[test]
fn tst_cma_034_integration_non_boundary_checkpoint_event_suppressed() {
    assert!(!should_write_stage_boundary_checkpoint(
        StageEvent::NonBoundary,
        SessionType::Main
    ));
    assert!(!should_write_stage_boundary_checkpoint(
        StageEvent::StageBoundary(StageName::Implement),
        SessionType::Background
    ));
}

#[test]
fn tst_cma_046_integration_restart_prefers_latest_checkpoint_when_decodable() {
    let cp = CheckpointRecord {
        payload: sample_payload(),
        decodable: true.into(),
        lifecycle: CheckpointLifecycle::Persisted,
    };
    let out = execute_restart_recovery_matrix(RecoveryAttempt {
        latest_checkpoint: Some(Ok(cp.clone())),
        transcript_state: TranscriptState::Decodable,
        checkpoint_write_state: CheckpointWriteState::Clean,
    })
    .expect("resume");
    assert_eq!(out, RecoveryOutcome::ResumeFromCheckpoint(cp));
}

#[test]
fn tst_cma_048_integration_restart_without_checkpoint_and_corrupt_transcript_errors() {
    let out = execute_restart_recovery_matrix(RecoveryAttempt {
        latest_checkpoint: None,
        transcript_state: TranscriptState::Corrupt,
        checkpoint_write_state: CheckpointWriteState::Clean,
    });
    assert!(matches!(out, Err(RecoveryError::TranscriptCorruptionError)));
}

#[test]
fn tst_cma_049_integration_restart_without_any_state_errors() {
    let out = execute_restart_recovery_matrix(RecoveryAttempt {
        latest_checkpoint: None,
        transcript_state: TranscriptState::Missing,
        checkpoint_write_state: CheckpointWriteState::Clean,
    });
    assert!(matches!(out, Err(RecoveryError::MissingSessionStateError)));
}

#[test]
fn tst_cma_050_integration_prior_checkpoint_write_error_uses_transcript_retry_path() {
    let out = execute_restart_recovery_matrix(RecoveryAttempt {
        latest_checkpoint: None,
        transcript_state: TranscriptState::Decodable,
        checkpoint_write_state: CheckpointWriteState::PriorWriteError,
    })
    .expect("recovery");
    assert_eq!(out, RecoveryOutcome::ResumeFromTranscriptRetryNeeded);
}

#[test]
fn tst_cma_061_integration_background_session_checkpoint_flow_is_blocked() {
    let out = orchestrate_stage_boundary_checkpoint_write(StageBoundaryCheckpointWriteRequest {
        event: StageEvent::StageBoundary(StageName::Implement),
        snapshot: sample_snapshot(SessionType::Background),
        estimate: BudgetEstimate {
            estimated_prompt_tokens: TokenCount::new(90),
            context_budget_tokens: TokenCount::new(50),
        },
        payload: sample_payload(),
        config: sample_config(),
    });
    assert!(matches!(out, Err(CheckpointError::CheckpointWriteError)));
}

#[test]
fn tst_cma_062_integration_stage_completion_requires_successful_boundary_checkpoint_write() {
    let out = orchestrate_stage_boundary_checkpoint_write(StageBoundaryCheckpointWriteRequest {
        event: StageEvent::StageBoundary(StageName::Implement),
        snapshot: sample_snapshot(SessionType::Main),
        estimate: BudgetEstimate {
            estimated_prompt_tokens: TokenCount::new(10),
            context_budget_tokens: TokenCount::new(50),
        },
        payload: sample_payload(),
        config: sample_config(),
    })
    .expect("boundary checkpoint write succeeds");
    assert_eq!(out.lifecycle, CheckpointLifecycle::Persisted);
}

#[test]
fn tst_cma_063_integration_background_session_resume_flow_is_blocked() {
    let out = execute_restart_recovery_for_session(SessionRecoveryRequest {
        session_type: SessionType::Background,
        attempt: RecoveryAttempt {
            latest_checkpoint: None,
            transcript_state: TranscriptState::Decodable,
            checkpoint_write_state: CheckpointWriteState::Clean,
        },
    });
    assert!(matches!(out, Err(RecoveryError::MissingSessionStateError)));
}
