use std::fs;

#[test]
fn supervisor_actor_module_exposes_spawn_and_run_symbols() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/supervisor/supervisor_actor.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("supervisor_actor.rs must be readable");

    assert!(source.contains("pub struct SupervisorActor;"));
    assert!(source.contains("pub fn spawn("));
    assert!(source.contains("async fn run("));
}
