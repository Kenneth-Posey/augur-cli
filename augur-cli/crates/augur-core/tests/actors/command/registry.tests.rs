use augur_core::actors::command::command_actor::build;
use augur_core::actors::command::types::CommandOutcome;
use augur_domain::domain::string_newtypes::PromptText;

#[test]
fn completions_for_prefix_are_capped_and_sorted() {
    let handle = build(&[]);
    let completions = handle.completions_for(&PromptText::from("/"));
    assert!(!completions.is_empty());
    assert!(completions.len() <= 12);

    let mut names: Vec<&str> = completions.iter().map(|c| c.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
}

#[test]
fn generate_catalog_command_parses_provider_flag() {
    let handle = build(&[]);
    match handle.execute(&PromptText::from("/generate-catalog --provider openai")) {
        CommandOutcome::GenerateCatalog { provider } => {
            assert_eq!(provider.as_deref(), Some("openai"));
        }
        _ => panic!("expected CommandOutcome::GenerateCatalog"),
    }
}
