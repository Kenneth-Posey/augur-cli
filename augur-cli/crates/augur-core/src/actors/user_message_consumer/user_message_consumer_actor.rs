//! User message consumer actor: classifies and routes raw user input strings.

use super::handle::UserMessageConsumerHandle;
use super::user_message_consumer_actor_ops as actor_ops;
use augur_domain::domain::channels::USER_FEED_CAPACITY;
use augur_domain::domain::feeds::UserFeedMessage;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

// ── UserMessageOutputChannels ─────────────────────────────────────────────────

/// Bundle of output sender channels for the two routable user-feed categories.
///
/// `raw_tx` receives every classified message. `parsed_tx` receives only
/// messages where the [`augur_domain::domain::feeds::UserInputTag`] is
/// [`augur_domain::domain::feeds::UserInputTag::ParsedCommand`].
pub struct UserMessageOutputChannels {
    /// Sender for all user input messages, regardless of classification.
    pub raw_tx: mpsc::Sender<UserFeedMessage>,
    /// Sender for slash-command messages only.
    pub parsed_tx: mpsc::Sender<UserFeedMessage>,
}

// ── spawn ─────────────────────────────────────────────────────────────────────

/// Spawn the user message consumer actor and return its join handle and a communication handle.
///
/// Creates a bounded command channel using `USER_FEED_CAPACITY`, wraps the
/// sender in a [`UserMessageConsumerHandle`], and spawns the `run` loop as a
/// Tokio task. Callers send raw input strings via the handle; the actor
/// classifies each and routes to the output channels in `outputs`.
pub fn spawn(outputs: UserMessageOutputChannels) -> (JoinHandle<()>, UserMessageConsumerHandle) {
    let (tx, rx) = mpsc::channel(*USER_FEED_CAPACITY);
    let handle = UserMessageConsumerHandle { tx };
    let join = tokio::spawn(actor_ops::run(rx, outputs));
    (join, handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use augur_domain::domain::feeds::UserInputTag;
    use augur_domain::domain::string_newtypes::OutputText;
    use tokio::time::{timeout, Duration};

    /// Verifies that a plain text input sent via the handle arrives on the raw channel.
    #[tokio::test]
    async fn run_sends_to_raw_channel() {
        let (raw_tx, mut raw_rx) = mpsc::channel(8);
        let (parsed_tx, _parsed_rx) = mpsc::channel(8);

        let outputs = UserMessageOutputChannels { raw_tx, parsed_tx };
        let (_join, handle) = spawn(outputs);

        handle.process_input(OutputText::from("hello"));

        let msg = timeout(Duration::from_secs(2), raw_rx.recv())
            .await
            .expect("must receive within timeout")
            .expect("raw channel must have a message");

        assert_eq!(msg.tag, UserInputTag::RawCommand);
        assert_eq!(msg.text, "hello");
        handle.shutdown();
    }

    /// Verifies that a slash command is delivered to both raw and parsed channels.
    #[tokio::test]
    async fn run_sends_parsed_to_parsed_channel() {
        let (raw_tx, mut raw_rx) = mpsc::channel(8);
        let (parsed_tx, mut parsed_rx) = mpsc::channel(8);

        let outputs = UserMessageOutputChannels { raw_tx, parsed_tx };
        let (_join, handle) = spawn(outputs);

        handle.process_input(OutputText::from("/command"));

        let raw_msg = timeout(Duration::from_secs(2), raw_rx.recv())
            .await
            .expect("must receive within timeout on raw channel")
            .expect("raw channel must have a message");

        let parsed_msg = timeout(Duration::from_secs(2), parsed_rx.recv())
            .await
            .expect("must receive within timeout on parsed channel")
            .expect("parsed channel must have a message");

        assert_eq!(raw_msg.tag, UserInputTag::ParsedCommand);
        assert_eq!(parsed_msg.tag, UserInputTag::ParsedCommand);
        assert_eq!(raw_msg.text, "/command");
        assert_eq!(parsed_msg.text, "/command");
        handle.shutdown();
    }

    /// Verifies that a non-slash input does NOT appear on the parsed channel.
    #[tokio::test]
    async fn run_does_not_send_raw_to_parsed_channel() {
        let (raw_tx, mut raw_rx) = mpsc::channel(8);
        let (parsed_tx, mut parsed_rx) = mpsc::channel(8);

        let outputs = UserMessageOutputChannels { raw_tx, parsed_tx };
        let (_join, handle) = spawn(outputs);

        handle.process_input(OutputText::from("not a command"));

        let _raw_msg = timeout(Duration::from_secs(2), raw_rx.recv())
            .await
            .expect("must receive on raw channel")
            .expect("raw channel must have a message");

        let result = timeout(Duration::from_millis(100), parsed_rx.recv()).await;
        assert!(
            result.is_err(),
            "parsed channel must be empty for non-slash input"
        );
        handle.shutdown();
    }

    /// Verifies that calling shutdown causes the actor task to exit cleanly.
    #[tokio::test]
    async fn shutdown_stops_actor() {
        let (raw_tx, _raw_rx) = mpsc::channel(8);
        let (parsed_tx, _parsed_rx) = mpsc::channel(8);

        let outputs = UserMessageOutputChannels { raw_tx, parsed_tx };
        let (join, handle) = spawn(outputs);

        handle.shutdown();

        let result = timeout(Duration::from_secs(2), join).await;
        assert!(
            result.is_ok(),
            "actor must finish within 2 seconds of shutdown"
        );
    }
}
