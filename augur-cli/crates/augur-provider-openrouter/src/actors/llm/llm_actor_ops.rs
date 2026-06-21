//! Private helper operations for the LLM actor run loop.

use super::llm_actor::{dispatch_request, inject_openrouter_headers, LlmRunConfig};
use augur_domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::types::StreamChunk;
use augur_provider_shared::request_context::{build_request_context, CompleteFields};
use tokio::sync::mpsc;

/// Build request context and dispatch one completion request task.
///
/// On context-build failure, emits a single `StreamChunk::Error` to `err_tx`.
/// On success, injects OpenRouter headers and spawns `dispatch_request`.
pub(super) fn dispatch_complete(
    fields: CompleteFields,
    err_tx: mpsc::Sender<StreamChunk>,
    cfg: &LlmRunConfig,
) {
    match build_request_context(fields, &cfg.app) {
        Err(error) => {
            let err_text = error.to_string();
            tokio::spawn(async move {
                let _ = err_tx
                    .send(StreamChunk::Error(OutputText::new(err_text)))
                    .await;
            });
        }
        Ok(mut context) => {
            inject_openrouter_headers(&mut context, &cfg.or_cache, &cfg.session_id);
            tokio::spawn(dispatch_request(context));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use augur_domain::config::provider_catalog::OpenRouterCacheConfig;
    use augur_domain::config::{AgentConfig, AppConfig, CopilotConfig, PersistenceConfig};
    use augur_domain::newtypes::{Temperature, TokenCount};
    use augur_domain::string_newtypes::{EndpointName, FilePath, OutputText};
    use augur_domain::types::StreamChunk;
    use augur_domain::NumericNewtype;
    use augur_provider_shared::request_context::{CompleteFields, CompleteRoute, RequestPayload};

    fn test_app_config() -> AppConfig {
        AppConfig {
            endpoints: vec![],
            default_endpoint: EndpointName::new("default"),
            agent: AgentConfig {
                system_prompt: OutputText::new(""),
                max_tokens: TokenCount::new(128),
                temperature: Temperature::new(0.5),
                allowed_dirs: vec![FilePath::new("./")],
            },
            copilot: CopilotConfig::default(),
            persistence: PersistenceConfig {
                log_dir: FilePath::new("./logs"),
                sessions_dir: None,
            },
            program_settings: Default::default(),
            user_settings: Default::default(),
        }
    }

    fn test_logger() -> augur_domain::domain::actor_contracts::LoggerHandle {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        augur_domain::domain::actor_contracts::LoggerHandle::new(tx)
    }

    #[tokio::test]
    async fn dispatch_complete_emits_error_when_endpoint_is_missing() {
        let (reply_tx, mut reply_rx) = mpsc::channel(1);
        let fields = CompleteFields::builder()
            .route(
                CompleteRoute::builder()
                    .endpoint(EndpointName::new("missing"))
                    .build(),
            )
            .payload(
                RequestPayload::builder()
                    .messages(vec![])
                    .tools(vec![])
                    .build(),
            )
            .reply_tx(reply_tx.clone())
            .build();
        let cfg = LlmRunConfig {
            app: test_app_config(),
            or_cache: OpenRouterCacheConfig::default(),
            session_id: "test-session-id".to_string(),
            logger: test_logger(),
        };

        dispatch_complete(fields, reply_tx, &cfg);

        let received = tokio::time::timeout(std::time::Duration::from_secs(2), reply_rx.recv())
            .await
            .expect("error message should arrive")
            .expect("channel should stay open long enough for one message");

        assert!(matches!(received, StreamChunk::Error(_)));
    }
}
