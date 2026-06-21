use augur_domain::domain::background_events::BackgroundEventPriority;

#[test]
fn critical_priority_is_critical() {
    let priority = BackgroundEventPriority::Critical;
    assert!(priority.is_critical().0);
    assert!(!priority.is_informational().0);
    assert!(!priority.is_debug().0);
}

#[test]
fn informational_priority_is_informational() {
    let priority = BackgroundEventPriority::Informational;
    assert!(!priority.is_critical().0);
    assert!(priority.is_informational().0);
    assert!(!priority.is_debug().0);
}

#[test]
fn debug_priority_is_debug() {
    let priority = BackgroundEventPriority::Debug;
    assert!(!priority.is_critical().0);
    assert!(!priority.is_informational().0);
    assert!(priority.is_debug().0);
}
