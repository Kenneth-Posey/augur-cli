use super::{AgentOutputReceiver, ChatRuntime, ChatRuntimeInput, CoreRuntime};
use augur_core::actors;
use augur_domain::config::types::AppConfig;
use augur_domain::domain::string_newtypes::{EndpointName, ModelId};
use augur_domain::domain::task_types::AgentSpecName;
use augur_domain::domain::traits::{BackgroundTaskRunnerPort, ChatProvider};
use augur_domain::domain::types::{AgentOutput, FeedEntry};
use augur_domain::domain::StringNewtype;
use std::sync::Arc;

#[derive(Clone)]
pub struct EndpointRoutingChatProvider {
    agent: actors::AgentHandle,
    session: actors::SessionHandle,
    task_runner: Option<Arc<dyn BackgroundTaskRunnerPort>>,
    openrouter_orchestrator:
        augur_provider_openrouter::actors::openrouter_orchestrator::handle::OpenRouterOrchestratorHandle,
}

impl EndpointRoutingChatProvider {
    fn active_endpoint(&self) -> EndpointName {
        self.session.active_endpoint()
    }
}

impl ChatProvider for EndpointRoutingChatProvider {
    fn submit(&self, prompt: augur_domain::domain::PromptText, endpoint: Option<EndpointName>) {
        let selected = endpoint.unwrap_or_else(|| self.active_endpoint());
        self.agent.submit(prompt, selected);
    }

    fn interrupt(&self) {
        self.agent.interrupt();
    }

    fn shutdown(&self) {
        self.agent.shutdown();
    }

    fn restore(&self, records: Vec<augur_domain::persistence::types::MessageRecord>) {
        self.agent.restore(records);
    }

    fn subscribe_output(
        &self,
    ) -> tokio::sync::broadcast::Receiver<augur_domain::domain::types::AgentOutput> {
        self.agent.subscribe_output()
    }

    fn compact(&self) {
        self.agent.compact();
    }

    fn submit_with_attachments(
        &self,
        prompt: augur_domain::domain::PromptText,
        endpoint: Option<EndpointName>,
        attachments: Vec<augur_domain::domain::FilePath>,
    ) {
        let selected = endpoint.unwrap_or_else(|| self.active_endpoint());
        self.agent
            .submit_with_attachments(prompt, Some(selected), attachments);
    }

    fn set_model(&self, model_id: ModelId) {
        self.agent.set_model(model_id);
    }

    fn set_model_with_options(
        &self,
        model_id: ModelId,
        reasoning_effort: Option<augur_domain::domain::thinking_mode::ReasoningEffort>,
    ) {
        self.agent
            .set_model_with_options(model_id, reasoning_effort);
    }

    fn replace_session(&self, sdk_session_id: Option<augur_domain::domain::SdkSessionId>) {
        self.agent.replace_session(sdk_session_id);
        if let Err(error) = self.openrouter_orchestrator.reset_session() {
            tracing::warn!("failed to reset OpenRouter orchestrator session: {error}");
        }
    }

    fn run_background_agent(
        &self,
        agent: augur_domain::domain::AgentName,
        prompt: augur_domain::domain::PromptText,
    ) {
        if let Some(runner) = &self.task_runner {
            let spec_name = AgentSpecName::new(agent.as_str());
            runner.run(spec_name, prompt);
        }
    }
}

/// Construct and wire the `ChatRuntime` from the given config and core handles.
///
/// Builds the task runner, spawns background model-listener and feed-forwarder
/// tasks, creates the `EndpointRoutingChatProvider`, and restores any saved
/// model selection before returning the assembled `ChatRuntime`.
pub async fn spawn_chat_runtime(
    config: &AppConfig,
    core: &mut CoreRuntime,
    input: ChatRuntimeInput,
) -> ChatRuntime {
    let task_runner_outcome = build_chat_task_runner(core).await;
    let task_runner = task_runner_outcome.runner;
    let active_model = task_runner_outcome.active_model;

    spawn_active_model_listener(&input.agent_handle, active_model);
    spawn_openrouter_feed_forwarder(core, input.agent_feed_tx.clone());

    let output_rx = input.agent_handle.subscribe_output();
    let provider: Arc<dyn ChatProvider> = Arc::new(EndpointRoutingChatProvider {
        agent: input.agent_handle,
        session: input.session_handle,
        task_runner,
        openrouter_orchestrator: core.context.control.openrouter_orchestrator_handle.clone(),
    });
    restore_saved_model_selection(
        provider.as_ref(),
        config,
        augur_core::config::user_settings::load_user_settings(),
    );
    ChatRuntime {
        provider,
        output_rx,
        join: None,
    }
}

async fn build_chat_task_runner(
    core: &CoreRuntime,
) -> crate::wiring::task_runner::TaskRunnerOutcome {
    use crate::wiring::task_runner::{build_task_runner, TaskRunnerBuildArgs};
    build_task_runner(
        TaskRunnerBuildArgs::builder()
            .orchestrator(core.context.control.openrouter_orchestrator_handle.clone())
            .active_model(core.context.control.openrouter_active_model_handle.clone())
            .build(),
    )
    .await
}

fn restore_saved_model_selection(
    provider: &dyn ChatProvider,
    config: &AppConfig,
    settings: augur_core::config::user_settings::UserSettings,
) {
    let Some(model_str) = settings.last_model else {
        return;
    };
    let endpoint_matches = settings
        .last_endpoint
        .as_deref()
        .map(|ep| ep == config.default_endpoint.as_str())
        .unwrap_or(true);
    if !endpoint_matches {
        return;
    }
    let effort = settings
        .last_reasoning_effort
        .as_deref()
        .and_then(augur_domain::domain::thinking_mode::ReasoningEffort::parse_optional);
    provider.set_model_with_options(ModelId::new(model_str.as_str()), effort);
}

fn spawn_openrouter_feed_forwarder(
    core: &mut CoreRuntime,
    agent_feed_tx: tokio::sync::mpsc::Sender<FeedEntry>,
) {
    if let Some(mut openrouter_feed_rx) = core.context.control.openrouter_feed_rx.take() {
        tokio::spawn(async move {
            while let Some(event) = openrouter_feed_rx.recv().await {
                let _ = agent_feed_tx.send(event).await;
            }
        });
    }
}

fn spawn_active_model_listener(
    agent_handle: &actors::AgentHandle,
    active_model: actors::ActiveModelHandle,
) {
    let mut agent_output_rx = agent_handle.subscribe_output();
    tokio::spawn(async move {
        while let Some(event) = recv_agent_output_event(&mut agent_output_rx).await {
            if let AgentOutput::ActiveModelChanged(model_id) = event {
                active_model.set_model(model_id);
            }
        }
    });
}

async fn recv_agent_output_event(
    output_rx: &mut tokio::sync::broadcast::Receiver<AgentOutput>,
) -> Option<AgentOutput> {
    loop {
        match output_rx.recv().await {
            Ok(event) => return Some(event),
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use augur_domain::config::types::{
        AgentConfig, AppConfig, CopilotConfig, EndpointConfig, EndpointCredentials,
        PersistenceConfig, ProgramSettings, Provider,
    };
    use augur_domain::domain::newtypes::{NumericNewtype, Temperature, TimestampSecs, TokenCount};
    use augur_domain::domain::string_newtypes::{
        AgentName, EndpointName, EndpointUrl, FilePath, ModelName, OutputText, PromptText,
    };
    use augur_domain::domain::task_types::AgentSpecName;
    use augur_domain::domain::traits::BackgroundTaskRunnerPort;
    use augur_domain::domain::types::{AgentOutput, FeedEntry};
    use augur_domain::domain::StringNewtype;
    use augur_domain::persistence::types::{MessageRecord, MessageType};
    use std::sync::{Arc, Mutex};

    struct MockTaskRunner {
        calls: Mutex<Vec<(AgentSpecName, PromptText)>>,
    }

    impl MockTaskRunner {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.lock().expect("lock").len()
        }
    }

    impl BackgroundTaskRunnerPort for MockTaskRunner {
        fn run(&self, agent: AgentSpecName, prompt: PromptText) {
            self.calls.lock().expect("lock").push((agent, prompt));
        }
    }

    fn app_config_with_default_endpoint(endpoint: &str) -> AppConfig {
        AppConfig {
            endpoints: vec![EndpointConfig {
                name: EndpointName::new(endpoint),
                provider: Provider::OpenRouter,
                base_url: EndpointUrl::new("https://openrouter.ai/api/v1"),
                model: ModelName::new("openai/gpt-4.1-mini"),
                credentials: EndpointCredentials::default(),
            }],
            default_endpoint: EndpointName::new(endpoint),
            agent: AgentConfig {
                system_prompt: OutputText::new("sys"),
                max_tokens: TokenCount::new(1024),
                temperature: Temperature::new(0.7),
                allowed_dirs: vec![FilePath::new(".")],
            },
            copilot: CopilotConfig::default(),
            persistence: PersistenceConfig {
                log_dir: FilePath::new("./logs"),
                sessions_dir: Some(FilePath::new(
                    std::env::temp_dir()
                        .join("augur-cli-chat-provider-tests")
                        .to_str()
                        .unwrap_or("/tmp/augur-cli-chat-provider-tests"),
                )),
            },
            program_settings: Default::default(),
            user_settings: Default::default(),
        }
    }

    async fn make_provider(
        endpoint: &str,
        task_runner: Option<Arc<dyn BackgroundTaskRunnerPort>>,
    ) -> EndpointRoutingChatProvider {
        let config = app_config_with_default_endpoint(endpoint);
        let program_settings = ProgramSettings::default();
        let core =
            super::super::spawn_infrastructure(&config, &program_settings, TimestampSecs::new(1));
        let (feed_tx, _feed_rx) = tokio::sync::mpsc::channel::<FeedEntry>(8);
        let domain = super::super::spawn_domain_actors(
            super::super::DomainRuntimeConfigRef {
                config: &config,
                program_settings: &program_settings,
            },
            &core,
            feed_tx,
        )
        .await;
        EndpointRoutingChatProvider {
            agent: domain.agent.handle,
            session: domain.session.handle,
            task_runner,
            openrouter_orchestrator: core.context.control.openrouter_orchestrator_handle.clone(),
        }
    }

    #[tokio::test]
    async fn run_background_agent_routes_to_task_runner_when_present() {
        let runner = Arc::new(MockTaskRunner::new());
        let provider = make_provider("openrouter", Some(runner.clone())).await;
        provider.run_background_agent(AgentName::new("triage"), PromptText::new("run triage"));
        assert_eq!(runner.call_count(), 1);
    }

    #[tokio::test]
    async fn run_background_agent_is_no_op_without_task_runner() {
        let provider = make_provider("openrouter", None).await;
        provider.run_background_agent(AgentName::new("triage"), PromptText::new("run triage"));
    }

    #[tokio::test]
    async fn submit_routes_to_active_or_override_endpoint() {
        let provider = make_provider("openrouter", None).await;
        provider.submit(PromptText::new("hello"), None);
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        let state = provider.agent.get_state().await;
        assert_eq!(
            state.last_endpoint.as_ref().map(|ep| ep.as_str()),
            Some("openrouter")
        );

        provider.submit(
            PromptText::new("hello again"),
            Some(EndpointName::new("anthropic")),
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        let state = provider.agent.get_state().await;
        assert_eq!(
            state.last_endpoint.as_ref().map(|ep| ep.as_str()),
            Some("anthropic")
        );
    }

    #[tokio::test]
    async fn submit_with_attachments_falls_back_to_submit_path() {
        let provider = make_provider("openrouter", None).await;
        provider.submit_with_attachments(
            PromptText::new("with files"),
            None,
            vec![FilePath::new("README.md")],
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        let state = provider.agent.get_state().await;
        assert_eq!(
            state.last_endpoint.as_ref().map(|ep| ep.as_str()),
            Some("openrouter")
        );
    }

    #[tokio::test]
    async fn set_model_and_set_model_with_options_update_selected_model() {
        let provider = make_provider("openrouter", None).await;
        provider.set_model(augur_domain::domain::ModelId::new("model-a"));
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        let state = provider.agent.get_state().await;
        assert_eq!(
            state.selected_model.as_ref().map(|model| model.as_str()),
            Some("model-a")
        );

        provider.set_model_with_options(
            augur_domain::domain::ModelId::new("model-b"),
            Some(augur_domain::domain::thinking_mode::ReasoningEffort::High),
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        let state = provider.agent.get_state().await;
        assert_eq!(
            state.selected_model.as_ref().map(|model| model.as_str()),
            Some("model-b")
        );
    }

    #[tokio::test]
    async fn restore_filters_error_records_before_agent_history_restore() {
        let provider = make_provider("openrouter", None).await;
        provider.restore(vec![
            MessageRecord {
                message_type: MessageType::Error,
                message: augur_domain::domain::Message::assistant("error annotation"),
            },
            MessageRecord {
                message_type: MessageType::User,
                message: augur_domain::domain::Message::user("keep me"),
            },
        ]);
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        let restored = provider.agent.history_snapshot().await;
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].content, "keep me");
    }

    #[test]
    fn replace_session_routes_to_orchestrator_reset() {
        let source = include_str!("chat_provider.rs");
        assert!(
            source.contains("self.agent.replace_session(sdk_session_id)"),
            "replace_session must first clear the agent's in-memory history via AgentHandle::replace_session"
        );
        assert!(
            source.contains("openrouter_orchestrator.reset_session()"),
            "replace_session must also reset the OpenRouter orchestrator"
        );
    }

    #[tokio::test]
    async fn spawn_active_model_listener_updates_active_model() {
        let provider = make_provider("openrouter", None).await;
        let active_model = augur_core::actors::active_model::spawn();
        spawn_active_model_listener(&provider.agent, active_model.clone());
        let _ = provider
            .agent
            .clone_output_tx()
            .send(AgentOutput::ActiveModelChanged(
                augur_domain::domain::ModelId::new("gpt-5-mini"),
            ));
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert_eq!(
            active_model.current_model().as_ref().map(|m| m.as_str()),
            Some("gpt-5-mini")
        );
    }

    #[tokio::test]
    async fn spawn_openrouter_feed_forwarder_forwards_feed_events() {
        let mut core = super::super::spawn_infrastructure(
            &app_config_with_default_endpoint("openrouter"),
            &augur_domain::config::types::ProgramSettings::default(),
            TimestampSecs::new(1),
        );
        let (agent_feed_tx, mut agent_feed_rx) = tokio::sync::mpsc::channel(8);
        let (openrouter_feed_tx, openrouter_feed_rx) = tokio::sync::mpsc::channel(8);
        core.context.control.openrouter_feed_rx = Some(openrouter_feed_rx);

        spawn_openrouter_feed_forwarder(&mut core, agent_feed_tx);
        openrouter_feed_tx
            .send(FeedEntry {
                feed_id: augur_domain::domain::types::FeedId::Agent("chat-provider-test".into()),
                output: augur_domain::domain::types::AgentFeedOutput::StatusLine(OutputText::new(
                    "forward me",
                )),
            })
            .await
            .expect("send feed event");

        assert!(matches!(
            agent_feed_rx.recv().await,
            Some(FeedEntry { output: augur_domain::domain::types::AgentFeedOutput::StatusLine(line), .. }) if line.as_str() == "forward me"
        ));
    }

    struct RecordingRestoreProvider {
        calls: Mutex<Vec<(String, Option<String>)>>,
        output_tx: tokio::sync::broadcast::Sender<AgentOutput>,
    }

    impl RecordingRestoreProvider {
        fn new() -> Self {
            let (output_tx, _) = tokio::sync::broadcast::channel(1);
            Self {
                calls: Mutex::new(Vec::new()),
                output_tx,
            }
        }

        fn calls(&self) -> Vec<(String, Option<String>)> {
            self.calls.lock().expect("lock").clone()
        }
    }

    impl augur_domain::domain::traits::ChatProvider for RecordingRestoreProvider {
        fn submit(&self, _prompt: PromptText, _endpoint: Option<EndpointName>) {}
        fn interrupt(&self) {}
        fn shutdown(&self) {}
        fn restore(&self, _records: Vec<augur_domain::persistence::types::MessageRecord>) {}
        fn subscribe_output(&self) -> tokio::sync::broadcast::Receiver<AgentOutput> {
            self.output_tx.subscribe()
        }
        fn set_model_with_options(
            &self,
            model_id: augur_domain::domain::ModelId,
            reasoning_effort: Option<augur_domain::domain::thinking_mode::ReasoningEffort>,
        ) {
            self.calls.lock().expect("lock").push((
                model_id.as_str().to_owned(),
                reasoning_effort.map(|effort| effort.as_ref().to_owned()),
            ));
        }
    }

    #[test]
    fn restore_saved_model_selection_applies_model_and_effort_when_endpoint_matches() {
        let provider = RecordingRestoreProvider::new();
        restore_saved_model_selection(
            &provider,
            &app_config_with_default_endpoint("openrouter"),
            augur_core::config::user_settings::UserSettings {
                last_endpoint: Some("openrouter".to_owned()),
                last_model: Some("gpt-5".to_owned()),
                last_reasoning_effort: Some("high".to_owned()),
            },
        );
        assert_eq!(
            provider.calls(),
            vec![("gpt-5".to_owned(), Some("high".to_owned()))]
        );
    }

    #[test]
    fn restore_saved_model_selection_ignores_mismatched_endpoint() {
        let provider = RecordingRestoreProvider::new();
        restore_saved_model_selection(
            &provider,
            &app_config_with_default_endpoint("openrouter"),
            augur_core::config::user_settings::UserSettings {
                last_endpoint: Some("copilot".to_owned()),
                last_model: Some("gpt-5".to_owned()),
                last_reasoning_effort: Some("high".to_owned()),
            },
        );
        assert!(provider.calls().is_empty());
    }
}
