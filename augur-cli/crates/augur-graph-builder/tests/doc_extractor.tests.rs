use augur_graph_builder::doc_extractor;

#[test]
fn test_extract_simple_inner_doc_comment() {
    let source = r#"
//! This is a crate-level doc comment.

fn main() {}
"#;
    let result = doc_extractor::extract_first_doc_comment(source);
    assert_eq!(
        result,
        Some(" This is a crate-level doc comment.".to_string())
    );
}

#[test]
fn test_extract_no_doc_comment() {
    let source = r#"
fn main() {
    let x = 42;
}
"#;
    let result = doc_extractor::extract_first_doc_comment(source);
    assert_eq!(result, None);
}

#[test]
fn test_extract_empty_source() {
    let source = "";
    let result = doc_extractor::extract_first_doc_comment(source);
    assert_eq!(result, None);
}

#[test]
fn test_extract_only_outer_doc() {
    let source = r#"
/// This is an outer doc comment.
fn foo() {}
"#;
    let result = doc_extractor::extract_first_doc_comment(source);
    assert_eq!(result, None);
}

#[test]
fn test_extract_mixed_doc_comments() {
    let source = r#"
//! Crate doc.
/// Item doc.
fn bar() {}
"#;
    let result = doc_extractor::extract_first_doc_comment(source);
    assert_eq!(result, Some(" Crate doc.".to_string()));
}

#[test]
fn test_extract_block_doc_comment() {
    let source = r#"/*! Block crate doc. */

fn main() {}
"#;
    let result = doc_extractor::extract_first_doc_comment(source);
    assert_eq!(result, Some(" Block crate doc. ".to_string()));
}

#[test]
fn test_extract_multiline_inner_doc() {
    let source = r#"
//! Line one.
//! Line two.

fn f() {}
"#;
    // Only the first `//!` is captured by `extract_first_doc_comment`.
    let result = doc_extractor::extract_first_doc_comment(source);
    assert!(result.is_some());
    let text = result.unwrap();
    assert_eq!(text, " Line one.");
}
