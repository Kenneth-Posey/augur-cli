//! Tests for `CacheSnapshot` and `CachedTier` domain types.

use augur_core::actors::cache::cache_ops::{CacheSnapshot, CachedFile, CachedTier};
use augur_domain::domain::string_newtypes::{StatusLabel, StringNewtype};
use std::path::PathBuf;

/// `CacheSnapshot` with an empty tiers vec has no files.
#[test]
fn cache_snapshot_with_no_tiers_is_empty() {
    let snap = CacheSnapshot { tiers: vec![] };
    assert!(snap.tiers.is_empty());
}

/// `CachedTier` label and files are accessible after construction.
#[test]
fn cached_tier_label_and_files_roundtrip() {
    let file = CachedFile {
        path: PathBuf::from("src/main.rs"),
        content: "fn main() {}".to_owned().into(),
    };
    let tier = CachedTier {
        label: StatusLabel::new("Foundation (tier 1)"),
        files: vec![file],
    };
    assert_eq!(tier.label.as_str(), "Foundation (tier 1)");
    assert_eq!(tier.files.len(), 1);
    assert_eq!(tier.files[0].path, PathBuf::from("src/main.rs"));
}
