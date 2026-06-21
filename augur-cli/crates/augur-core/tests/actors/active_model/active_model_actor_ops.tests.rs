use augur_core::actors::active_model::active_model_ops::ActiveModelCommand;
use augur_core::actors::active_model::handle::ActiveModelHandle;
use augur_core::actors::active_model::spawn;
use augur_domain::domain::string_newtypes::ModelId;
use tokio::sync::{mpsc, watch};

/// Verifies the actor starts with no current model and applies `set_model`.
#[test]
fn spawn_sets_and_reads_current_model() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        let handle = spawn();
        assert_eq!(handle.current_model(), None);
        handle.set_model(ModelId::from("openrouter/gpt-5"));
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert_eq!(
            handle.current_model(),
            Some(ModelId::from("openrouter/gpt-5"))
        );
    });
}

/// Verifies `ActiveModelHandle::current_model` reads the latest watch snapshot.
#[test]
fn current_model_reads_watch_snapshot() {
    let (cmd_tx, _cmd_rx) = mpsc::channel(1);
    let (model_tx, model_rx) = watch::channel(Some(ModelId::from("x")));
    let handle = ActiveModelHandle::new(cmd_tx, model_rx);
    let _ = model_tx.send(Some(ModelId::from("y")));
    assert_eq!(handle.current_model(), Some(ModelId::from("y")));
}

/// Verifies `ActiveModelCommand::Set` carries the model id payload.
#[test]
fn set_command_carries_model_id() {
    let cmd = ActiveModelCommand::Set(ModelId::from("model-a"));
    match cmd {
        ActiveModelCommand::Set(model_id) => assert_eq!(model_id, ModelId::from("model-a")),
    }
}
