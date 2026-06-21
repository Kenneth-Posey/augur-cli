use std::fs;

#[test]
fn llm_feed_consumer_handle_exposes_consume_and_shutdown_commands() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/llm_feed_consumer/handle.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("llm_feed_consumer handle source must be readable");
    assert!(source.contains("pub fn consume(&self, chunk: StreamChunk)"));
    assert!(source.contains("LlmFeedConsumerCmd::Consume(chunk)"));
    assert!(source.contains("pub fn shutdown(&self)"));
    assert!(source.contains("LlmFeedConsumerCmd::Shutdown"));
}
