//! Extract top-level symbol names (functions, types, traits, constants, statics, macros)
//! from a Rust source file.

use syn::Item;
pub fn extract_symbols(source: &str) -> Vec<String> {
    let syntax_tree: syn::File = match syn::parse_file(source) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let mut symbols: Vec<String> = Vec::new();

    for item in &syntax_tree.items {
        if let Some(name) = extract_item_name(item) {
            symbols.push(name);
        }
    }

    symbols
}

fn extract_item_name(item: &Item) -> Option<String> {
    match item {
        Item::Fn(f) => Some(f.sig.ident.to_string()),
        Item::Struct(s) => Some(s.ident.to_string()),
        Item::Enum(e) => Some(e.ident.to_string()),
        Item::Trait(t) => Some(t.ident.to_string()),
        Item::Type(t) => Some(t.ident.to_string()),
        Item::Const(c) => Some(c.ident.to_string()),
        Item::Static(s) => Some(s.ident.to_string()),
        Item::Macro(m) => {
            // Macros declared with `macro_rules!` or `macro`
            m.ident.as_ref().map(|i| i.to_string())
        }
        // `mod` with content (inline module) - skip, they're separate nodes
        // `use` - skip, they're edges
        // `impl` - skip, they're not top-level items
        _ => None,
    }
}
