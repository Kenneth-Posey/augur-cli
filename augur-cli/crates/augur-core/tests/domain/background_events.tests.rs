use augur_domain::domain::background_events::BackgroundEventPriority;

#[test]
fn is_critical_only_for_critical() {
    assert!(BackgroundEventPriority::Critical.is_critical().0);
    assert!(!BackgroundEventPriority::Informational.is_critical().0);
    assert!(!BackgroundEventPriority::Debug.is_critical().0);
}

#[test]
fn is_informational_only_for_informational() {
    assert!(!BackgroundEventPriority::Critical.is_informational().0);
    assert!(BackgroundEventPriority::Informational.is_informational().0);
    assert!(!BackgroundEventPriority::Debug.is_informational().0);
}

#[test]
fn is_debug_only_for_debug() {
    assert!(!BackgroundEventPriority::Critical.is_debug().0);
    assert!(!BackgroundEventPriority::Informational.is_debug().0);
    assert!(BackgroundEventPriority::Debug.is_debug().0);
}
