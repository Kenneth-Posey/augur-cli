use augur_core::actors::orchestrator::OrchestratorContext;

fn main() {
    let ctx = OrchestratorContext::new();
    let _ = ctx.direct_multi_step_dispatch();
}
