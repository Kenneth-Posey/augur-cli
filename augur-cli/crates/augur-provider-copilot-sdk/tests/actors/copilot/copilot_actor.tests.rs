use std::path::PathBuf;

use augur_domain::config::types::CopilotChatConfig;
use augur_domain::persistence::handle::PersistenceHandle;
use augur_domain::tools::builtin::query_user::QueryUserRequest;
use augur_domain::{
    FeedEntry, HistoryAdapterCmd, HistoryAdapterHandle, LogCommand, LoggerHandle,
    TokenTrackerCommand, TokenTrackerHandle,
};
use augur_provider_copilot_sdk::actors::copilot::copilot_actor::{
    spawn, CopilotChannels, CopilotSpawnArgs,
};
use tokio::sync::mpsc;

#[tokio::test]
async fn spawn_exits_immediately_when_copilot_chat_is_disabled() {
    let (log_tx, _log_rx) = mpsc::channel::<LogCommand>(8);
    let (history_tx, _history_rx) = mpsc::channel::<HistoryAdapterCmd>(8);
    let (token_tx, _token_rx) = mpsc::channel::<TokenTrackerCommand>(8);
    let (query_tx, _query_rx) = mpsc::channel::<QueryUserRequest>(8);
    let (feed_tx, _feed_rx) = mpsc::channel::<FeedEntry>(8);
    let args = CopilotSpawnArgs::builder()
        .config(CopilotChatConfig::default())
        .logger(LoggerHandle::new(log_tx))
        .persistence(PersistenceHandle::new(PathBuf::from(".")))
        .history_adapter(HistoryAdapterHandle::new(history_tx))
        .channels(CopilotChannels {
            query_tx,
            agent_feed_tx: feed_tx,
            token_tracker: TokenTrackerHandle::new(token_tx),
        })
        .build();

    let (join, _handle) = spawn(args).await;
    join.await.expect("disabled actor task must join cleanly");
}
