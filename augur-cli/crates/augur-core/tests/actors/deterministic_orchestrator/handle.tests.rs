use augur_core::actors::deterministic_orchestrator::handle::PipelineResumeMode;

#[test]
fn pipeline_resume_mode_has_distinct_variants() {
    assert_ne!(
        PipelineResumeMode::ResumeExisting,
        PipelineResumeMode::StartFresh
    );
}
