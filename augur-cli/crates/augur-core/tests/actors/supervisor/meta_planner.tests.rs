use std::fs;

#[test]
fn meta_planner_module_exposes_prompt_and_update_symbols() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/supervisor/meta_planner.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("meta_planner.rs must be readable");

    assert!(source.contains("pub fn build_meta_prompt"));
    assert!(source.contains("pub struct PlanNodeUpdateParams"));
    assert!(source.contains("fn apply_plan_node_update"));
}
