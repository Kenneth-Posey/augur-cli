//! Private helper operations for the user-message consumer actor.

use super::user_message_consumer_actor::UserMessageOutputChannels;
use super::user_message_consumer_ops::{parse_user_input, UserMessageCmd};
use augur_domain::domain::feeds::UserInputTag;
use augur_domain::domain::string_newtypes::OutputText;
use tokio::sync::mpsc;

/// Actor receive loop: classifies each `ProcessInput` command and exits on `Shutdown`.
///
/// Inputs: `rx` - command receiver; `outputs` - output channel bundle.
/// Side effect: each `ProcessInput(text)` is classified via `parse_user_input`
/// and dispatched to the raw channel and, if a slash command, also to the
/// parsed channel.
pub(super) async fn run(
    mut rx: mpsc::Receiver<UserMessageCmd>,
    outputs: UserMessageOutputChannels,
) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            UserMessageCmd::ProcessInput(text) => {
                let msg = parse_user_input(&OutputText::from(text));
                let is_parsed = msg.tag == UserInputTag::ParsedCommand;
                let _ = outputs.raw_tx.try_send(msg.clone());
                if is_parsed {
                    let _ = outputs.parsed_tx.try_send(msg);
                }
            }
            UserMessageCmd::Shutdown => break,
        }
    }
}
