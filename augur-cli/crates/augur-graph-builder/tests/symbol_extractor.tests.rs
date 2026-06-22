use augur_graph_builder::symbol_extractor;

#[test]
fn test_extract_fn() {
    let symbols = symbol_extractor::extract_symbols("fn foo() {}");
    assert_eq!(symbols, vec!["foo"]);
}

#[test]
fn test_extract_struct() {
    let symbols = symbol_extractor::extract_symbols("struct Foo { x: i32 }");
    assert_eq!(symbols, vec!["Foo"]);
}

#[test]
fn test_extract_enum() {
    let symbols = symbol_extractor::extract_symbols("enum Color { Red, Blue }");
    assert_eq!(symbols, vec!["Color"]);
}

#[test]
fn test_extract_trait() {
    let symbols = symbol_extractor::extract_symbols("trait Foo { fn bar(); }");
    assert_eq!(symbols, vec!["Foo"]);
}

#[test]
fn test_extract_type_alias() {
    let symbols = symbol_extractor::extract_symbols("type Foo = i32;");
    assert_eq!(symbols, vec!["Foo"]);
}

#[test]
fn test_extract_multiple_symbols() {
    let src = r#"
        fn bar() {}
        struct Baz;
        enum Qux { A, B }
    "#;
    let symbols = symbol_extractor::extract_symbols(src);
    assert_eq!(symbols, vec!["bar", "Baz", "Qux"]);
}

#[test]
fn test_extract_empty() {
    let symbols = symbol_extractor::extract_symbols("");
    assert!(symbols.is_empty());
}

#[test]
fn test_extract_uses_and_impls_ignored() {
    let src = r#"
        use std::collections::HashMap;
        struct Foo;
        impl Foo {}
    "#;
    let symbols = symbol_extractor::extract_symbols(src);
    assert_eq!(symbols, vec!["Foo"]);
}

#[test]
fn test_extract_const() {
    let symbols = symbol_extractor::extract_symbols("const MAX: usize = 100;");
    assert_eq!(symbols, vec!["MAX"]);
}

#[test]
fn test_extract_static() {
    let symbols = symbol_extractor::extract_symbols("static NAME: &str = \"hello\";");
    assert_eq!(symbols, vec!["NAME"]);
}

#[test]
fn test_extract_macro() {
    let symbols = symbol_extractor::extract_symbols("macro_rules! my_macro { () => {} }");
    assert_eq!(symbols, vec!["my_macro"]);
}
