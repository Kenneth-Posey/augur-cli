//! Tests for the guided plan file loader.

use augur_core::actors::guided_plan::loader::{load_guided_plan, LoadError};
use augur_domain::domain::string_newtypes::StringNewtype;
use std::io::Write;
use tempfile::NamedTempFile;

/// Write `content` to a temp file and return its path handle.
fn temp_plan_file(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("create temp file");
    f.write_all(content.as_bytes()).expect("write temp file");
    f
}

/// Verifies that a valid plan file with `guided: true` frontmatter parses correctly.
#[test]
fn load_valid_plan_returns_config() {
    let content = r#"---
guided: true
name: "My Plan"
phases:
  - id: "phase-1"
    name: "Step One"
---
# My Plan

Some markdown body.
"#;
    let f = temp_plan_file(content);
    let config = load_guided_plan(f.path()).expect("should succeed");
    assert_eq!(config.name.as_str(), "My Plan");
    assert_eq!(config.phases.len(), 1);
    assert_eq!(config.phases[0].id.as_str(), "phase-1");
    assert_eq!(config.phases[0].name.as_str(), "Step One");
}

/// Verifies that a file with no `---` frontmatter returns `MissingFrontmatter`.
#[test]
fn load_missing_frontmatter_returns_error() {
    let content = "# Just Markdown\n\nNo frontmatter here.\n";
    let f = temp_plan_file(content);
    let err = load_guided_plan(f.path()).expect_err("should fail");
    assert!(
        matches!(err, LoadError::MissingFrontmatter),
        "expected MissingFrontmatter, got: {err}"
    );
}

/// Verifies that a file with `guided: false` returns `MissingFrontmatter`.
#[test]
fn load_guided_false_returns_missing_frontmatter() {
    let content = r#"---
guided: false
name: "Not A Guided Plan"
phases: []
---
# Body
"#;
    let f = temp_plan_file(content);
    let err = load_guided_plan(f.path()).expect_err("should fail");
    assert!(
        matches!(err, LoadError::MissingFrontmatter),
        "expected MissingFrontmatter, got: {err}"
    );
}

/// Verifies that a file without the `guided` key returns `MissingFrontmatter`.
#[test]
fn load_no_guided_key_returns_missing_frontmatter() {
    let content = r#"---
name: "Missing Guided Key"
phases: []
---
"#;
    let f = temp_plan_file(content);
    let err = load_guided_plan(f.path()).expect_err("should fail");
    assert!(matches!(err, LoadError::MissingFrontmatter));
}

/// Verifies that malformed YAML in the frontmatter returns a `Parse` error.
#[test]
fn load_malformed_yaml_returns_parse_error() {
    let content = "---\nguided: true\nname: [broken yaml\n---\n";
    let f = temp_plan_file(content);
    let err = load_guided_plan(f.path()).expect_err("should fail");
    assert!(
        matches!(err, LoadError::Parse(_)),
        "expected Parse, got: {err}"
    );
}

/// Verifies that a nonexistent file returns an `Io` error.
#[test]
fn load_nonexistent_file_returns_io_error() {
    let path = std::path::Path::new("/nonexistent/plan/file.md");
    let err = load_guided_plan(path).expect_err("should fail");
    assert!(matches!(err, LoadError::Io(_)), "expected Io, got: {err}");
}

/// Verifies the loader ignores the markdown body after the second `---` delimiter.
#[test]
fn load_ignores_markdown_body() {
    let content = r#"---
guided: true
name: "Body Plan"
phases:
  - id: "p1"
    name: "Phase 1"
---
# This is the markdown body

It should be ignored. Even if it has `yaml: content` it does not matter.
"#;
    let f = temp_plan_file(content);
    let config = load_guided_plan(f.path()).expect("should succeed");
    assert_eq!(config.name.as_str(), "Body Plan");
    assert_eq!(config.phases.len(), 1);
}
