use augur_domain::domain::events::{SessionInfo, SessionResumed, SessionStarted};

/// Verifies this integration test can reach exported event surface symbols.
#[test]
fn mirrored_surface_smoke_mod() {
    let type_name = core::any::type_name::<SessionInfo>();
    assert!(type_name.contains("SessionInfo"));
    let type_name = core::any::type_name::<SessionStarted>();
    assert!(type_name.contains("SessionStarted"));
    let type_name = core::any::type_name::<SessionResumed>();
    assert!(type_name.contains("SessionResumed"));
}
