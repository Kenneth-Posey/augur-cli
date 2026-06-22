use augur_core::actors::command::command_actor::build;
use augur_core::actors::command::types::CommandOutcome;
use augur_domain::domain::string_newtypes::{PromptText, StringNewtype};

/// Verifies that completions_for returns empty vec when buffer does not start with /.
///
/// Plain text in the buffer must produce no completions; the hint area
/// should remain hidden during normal conversation.
#[test]
fn completions_for_empty_for_plain_text() {
    let handle = build(&[]);
    assert!(handle
        .completions_for(&PromptText::from("hello"))
        .is_empty());
    assert!(handle.completions_for(&PromptText::from("")).is_empty());
}

/// Verifies that completions_for returns all commands when buffer is exactly "/".
///
/// Typing "/" alone should show every registered command as a completion,
/// alpha-sorted and ready for keyboard navigation. Commands past MAX_COMPLETIONS
/// are verified via all_commands() instead of the truncated completions list.
#[test]
fn completions_for_all_commands_for_bare_slash() {
    let handle = build(&[]);
    let cmds = handle.completions_for(&PromptText::from("/"));
    assert!(!cmds.is_empty(), "completions_for('/') must return results");
    let names: Vec<&str> = cmds.iter().map(|c| c.name).collect();
    assert!(names.contains(&"clear"));
    assert!(names.contains(&"help"));
    // "quit" and "switch" sort past MAX_COMPLETIONS with the current command count;
    // verify via all_commands() which is uncapped.
    let all_names: Vec<&str> = handle.all_commands().iter().map(|c| c.name).collect();
    assert!(all_names.contains(&"quit"));
    assert!(all_names.contains(&"switch"));
}

/// Verifies that completions_for filters by the typed partial command name.
///
/// "/q" should only return completions for commands whose name starts with "q",
/// confirming the prefix filter is applied after stripping the leading "/".
#[test]
fn completions_for_filtered_by_partial_name() {
    let handle = build(&[]);
    let cmds = handle.completions_for(&PromptText::from("/q"));
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].name, "quit");
}

/// Verifies that completions_for for a non-matching prefix returns an empty vec.
///
/// No completions should be shown when the typed prefix matches no command,
/// keeping the hint area clean rather than showing a "no match" placeholder.
#[test]
fn completions_for_empty_for_no_match() {
    let handle = build(&[]);
    assert!(handle.completions_for(&PromptText::from("/xyz")).is_empty());
}

/// Verifies that completions_for results are alpha-sorted by command name.
///
/// The completion list must be deterministically ordered so that keyboard
/// navigation produces the same sequence every time the same prefix is typed.
#[test]
fn completions_for_sorted_alphabetically() {
    let handle = build(&[]);
    let cmds = handle.completions_for(&PromptText::from("/"));
    let names: Vec<&str> = cmds.iter().map(|c| c.name).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "completions must be alpha-sorted");
}

/// Verifies that execute correctly dispatches /quit through the handle.
///
/// The handle must delegate to the registry without losing information.
#[test]
fn execute_quit_through_handle() {
    let handle = build(&[]);
    assert!(matches!(
        handle.execute(&PromptText::from("/quit")),
        CommandOutcome::Quit
    ));
}

/// Verifies that execute delegates /switch correctly through the handle.
///
/// The endpoint name must survive the handle boundary unchanged.
#[test]
fn execute_switch_through_handle() {
    let handle = build(&[]);
    match handle.execute(&PromptText::from("/switch claude")) {
        CommandOutcome::SwitchEndpoint(ep) => assert_eq!(ep.as_str(), "claude"),
        _ => panic!("expected SwitchEndpoint"),
    }
}

/// Verifies that execute returns NotACommand for plain text through the handle.
///
/// The handle must not intercept ordinary messages intended for the agent.
#[test]
fn execute_not_a_command_through_handle() {
    let handle = build(&[]);
    assert!(matches!(
        handle.execute(&PromptText::from("just a message")),
        CommandOutcome::NotACommand
    ));
}

/// Verifies that all_commands through the handle returns the full built-in set.
///
/// Callers that need the complete command list (e.g. for a help panel) must
/// be able to obtain it without going through the completions_for path.
#[test]
fn all_commands_through_handle_returns_builtins() {
    let handle = build(&[]);
    let cmds = handle.all_commands();
    let names: Vec<&str> = cmds.iter().map(|c| c.name).collect();
    assert!(names.contains(&"quit"));
    assert!(names.contains(&"switch"));
    assert!(names.contains(&"help"));
}
