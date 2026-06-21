//! Utility for extracting `//!` doc comments from Rust source text.
//!
//! Uses `syn` to parse the file and extract the first inner doc comment.

use syn::Attribute;

/// Extract the text of the first `//!` (inner doc) attribute from source.
///
/// Returns `None` if no doc comment is found or the source cannot be parsed.
pub fn extract_first_doc_comment(source: &str) -> Option<String> {
    let syntax_tree: syn::File = syn::parse_file(source).ok()?;
    for attr in &syntax_tree.attrs {
        if !matches!(attr.style, syn::AttrStyle::Inner(_)) {
            continue;
        }
        if attr.path().get_ident().is_none_or(|id| id != "doc") {
            continue;
        }
        let text = extract_doc_text(attr);
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

/// Extract the text content from a doc attribute.
///
/// Doc comments are represented as `#[doc = "text"]` attributes by syn.
fn extract_doc_text(attr: &Attribute) -> String {
    if let syn::Meta::NameValue(nv) = &attr.meta {
        if let syn::Expr::Lit(lit) = &nv.value {
            if let syn::Lit::Str(s) = &lit.lit {
                return s.value();
            }
        }
    }
    String::new()
}
