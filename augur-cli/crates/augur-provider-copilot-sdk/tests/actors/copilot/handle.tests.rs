use std::path::PathBuf;

use augur_domain::config::types::CopilotChatConfig;
use augur_domain::persistence::handle::PersistenceHandle;
use augur_domain::string_newtypes::{AgentName, ModelId, PromptText, SdkSessionId, StringNewtype};
use augur_domain::tools::builtin::query_user::QueryUserRequest;
use augur_domain::traits::ChatProvider;
use augur_domain::{
    FeedEntry, HistoryAdapterCmd, HistoryAdapterHandle, LogCommand, LoggerHandle,
    TokenTrackerCommand, TokenTrackerHandle,
};
use augur_provider_copilot_sdk::actors::copilot::copilot_actor::{
    CopilotChannels, CopilotSpawnArgs, spawn,
};
use augur_provider_copilot_sdk::actors::copilot::handle::{CopilotChatHandle, into_chat_provider};
use tokio::sync::mpsc;

fn spawn_args() -> CopilotSpawnArgs {
    let (log_tx, _log_rx) = mpsc::channel::<LogCommand>(8);
    let (history_tx, _history_rx) = mpsc::channel::<HistoryAdapterCmd>(8);
    let (token_tx, _token_rx) = mpsc::channel::<TokenTrackerCommand>(8);
    let (query_tx, _query_rx) = mpsc::channel::<QueryUserRequest>(8);
    let (feed_tx, _feed_rx) = mpsc::channel::<FeedEntry>(8);
    CopilotSpawnArgs::builder()
        .config(CopilotChatConfig::default())
        .logger(LoggerHandle::new(log_tx))
        .persistence(PersistenceHandle::new(PathBuf::from(".")))
        .history_adapter(HistoryAdapterHandle::new(history_tx))
        .channels(CopilotChannels {
            query_tx,
            agent_feed_tx: feed_tx,
            token_tracker: TokenTrackerHandle::new(token_tx),
        })
        .build()
}

#[tokio::test]
async fn chat_provider_wrapper_and_methods_are_callable() {
    assert!(core::any::type_name::<CopilotChatHandle>().contains("CopilotChatHandle"));

    let (join, handle) = spawn(spawn_args()).await;
    let provider = into_chat_provider(handle.clone());

    provider.submit(PromptText::new("hello"), None);
    provider.run_background_agent(AgentName::new("planner"), PromptText::new("analyze"));
    provider.set_model(ModelId::new("gpt-4.1"));
    provider.replace_session(Some(SdkSessionId::new("session-123")));
    provider.shutdown();

    let _ = handle.subscribe_output();
    join.await.expect("disabled actor task must join cleanly");
}
