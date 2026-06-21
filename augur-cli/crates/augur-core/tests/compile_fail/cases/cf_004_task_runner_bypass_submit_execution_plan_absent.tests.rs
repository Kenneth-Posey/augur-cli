use augur_core::actors::orchestrator::ingestion::run_without_orchestrator_submit;
use augur_domain::domain::ExecutionPlan;

fn main() {
    let plan = ExecutionPlan::new(Vec::new(), None);
    let _ = run_without_orchestrator_submit(plan);
}
