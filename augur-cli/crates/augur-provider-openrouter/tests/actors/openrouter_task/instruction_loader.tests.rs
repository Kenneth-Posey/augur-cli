use augur_domain::task_types::{InstructionFilePath, RepoRoot};
use augur_domain::types::Role;
use augur_domain::StringNewtype;
use augur_provider_openrouter::actors::openrouter_task::instruction_loader::load_instruction_prefix;
use std::fs;

#[tokio::test]
async fn load_instruction_prefix_reads_existing_files_in_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("first.md");
    let second = dir.path().join("second.md");
    fs::write(&first, "alpha").expect("write first");
    fs::write(&second, "beta").expect("write second");

    let prefix = load_instruction_prefix(
        &[
            InstructionFilePath::new("first.md"),
            InstructionFilePath::new("second.md"),
        ],
        &RepoRoot::new(dir.path().display().to_string()),
    )
    .await
    .expect("loader should succeed");

    assert_eq!(prefix.0.len(), 2);
    assert_eq!(prefix.0[0].role, Role::User);
    assert!(prefix.0[0].content.as_str().contains("[FILE: first.md]"));
    assert!(prefix.0[0].content.as_str().contains("alpha"));
    assert!(prefix.0[1].content.as_str().contains("[FILE: second.md]"));
    assert!(prefix.0[1].content.as_str().contains("beta"));
}

#[tokio::test]
async fn load_instruction_prefix_skips_missing_files_without_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let existing = dir.path().join("exists.md");
    fs::write(&existing, "present").expect("write existing");

    let prefix = load_instruction_prefix(
        &[
            InstructionFilePath::new("missing.md"),
            InstructionFilePath::new("exists.md"),
        ],
        &RepoRoot::new(dir.path().display().to_string()),
    )
    .await
    .expect("missing files are skipped, not fatal");

    assert_eq!(prefix.0.len(), 1);
    assert!(prefix.0[0].content.as_str().contains("[FILE: exists.md]"));
    assert!(prefix.0[0].content.as_str().contains("present"));
}
