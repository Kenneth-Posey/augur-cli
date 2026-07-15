//! Diagnostic test: end-to-end session-discovery flow replicated from
//! `load_startup_state` in `crates/augur-app/src/wiring/infrastructure.rs`.
//!
//! Each link in the chain is verified independently with `println!` output
//! so we can identify exactly where sessions get dropped between
//! `load_startup_state` and the TUI state initialization.

use augur_domain::domain::string_newtypes::{FilePath, StringNewtype};
use augur_domain::persistence::handle::PersistenceHandle;
use augur_domain::persistence::store::{apply_repo_subdir, list_sessions, resolve_sessions_dir};

/// Replicate the EXACT startup session-discovery flow from
/// `crates/augur-app/src/wiring/infrastructure.rs` `load_startup_state()`.
///
/// Prints every intermediate value so we can see where the chain breaks.
#[test]
fn startup_session_discovery_diagnostic() {
    // ── Step 1: Simulate load_startup_state() ────────────────────────────

    println!("\n===== STEP 1: load_startup_state simulation =====");

    // Replicate: resolve_sessions_dir(config.persistence.sessions_dir.as_ref())
    let configured = Some(FilePath::new("~/.augur-cli/sessions"));
    let base_sessions_dir = resolve_sessions_dir(configured.as_ref());
    println!("base_sessions_dir: {}", base_sessions_dir.display());

    // Replicate: std::env::current_dir()
    let cwd = std::env::current_dir().expect("cwd");
    println!("cwd: {}", cwd.display());

    // Replicate: apply_repo_subdir(base_sessions_dir, &cwd)
    let sessions_dir = apply_repo_subdir(base_sessions_dir, &cwd);
    println!("sessions_dir: {}", sessions_dir.display());

    // Replicate: list_sessions(&sessions_dir)
    let summaries = list_sessions(&sessions_dir).expect("list_sessions");
    println!("summaries.len(): {}", summaries.len());

    // Print first 3 summaries for inspection
    for (i, s) in summaries.iter().take(3).enumerate() {
        println!(
            "  summary[{}]: id={}, endpoint={}, preview={}, message_count={}",
            i,
            s.identity.id.as_str(),
            s.identity.endpoint_name.as_str(),
            s.preview.as_str(),
            s.message_count
        );
    }

    // ── Step 2: Simulate build_initial_mode() ────────────────────────────

    println!("\n===== STEP 2: build_initial_mode simulation =====");

    // Replicate the logic from state.rs build_initial_mode()
    let initial_screen = if summaries.is_empty() {
        "AppScreen::Conversation (NO sessions)"
    } else {
        "AppScreen::SessionSelector (sessions found)"
    };
    println!("build_initial_mode result: {initial_screen}");

    // ── Step 3: Simulate build_initial_state() ───────────────────────────

    println!("\n===== STEP 3: build_initial_state context =====");

    // We cannot fully replicate build_initial_state without the TUI actor
    // dependencies, but we can verify the critical routing condition:
    let session_selector_expected = !summaries.is_empty();
    println!(
        "SessionSelector expected (summaries non-empty): {}",
        session_selector_expected
    );

    // ── Step 4: Direct read_dir check ────────────────────────────────────

    println!("\n===== STEP 4: Direct read_dir check =====");

    match std::fs::read_dir(&sessions_dir) {
        Ok(entries) => {
            let count = entries.flatten().count();
            println!(
                "read_dir({}) succeeded, entry count: {}",
                sessions_dir.display(),
                count
            );

            // Count JSON files specifically
            if let Ok(entries2) = std::fs::read_dir(&sessions_dir) {
                let json_count = entries2
                    .flatten()
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                    .count();
                println!("JSON files in sessions_dir: {json_count}");
            }
        }
        Err(e) => {
            println!("read_dir({}) FAILED: {}", sessions_dir.display(), e);
        }
    }

    // Also check the base_sessions_dir (without repo subdir)
    println!("\n----- check base_sessions_dir (no repo subdir) -----");
    match std::fs::read_dir(resolve_sessions_dir(Some(&FilePath::new(
        "~/.augur-cli/sessions",
    )))) {
        Ok(entries) => {
            let count = entries.flatten().count();
            println!("read_dir(base) succeeded, entry count: {}", count);
        }
        Err(e) => {
            println!("read_dir(base) FAILED: {e}");
        }
    }

    // ── Step 5: PersistenceHandle check ──────────────────────────────────

    println!("\n===== STEP 5: PersistenceHandle check =====");

    let handle = PersistenceHandle::new(sessions_dir.clone());
    let handle_dir = handle.sessions_dir();
    println!("PersistenceHandle.sessions_dir(): {}", handle_dir.display());
    assert_eq!(
        handle_dir, sessions_dir,
        "PersistenceHandle sessions_dir should match the computed sessions_dir"
    );
    println!("PersistenceHandle sessions_dir MATCHES computed sessions_dir: OK");

    // ── Summary ──────────────────────────────────────────────────────────

    println!("\n===== DIAGNOSTIC SUMMARY =====");
    println!(
        "base_sessions_dir:     {}",
        resolve_sessions_dir(configured.as_ref()).display()
    );
    println!("cwd:                  {}", cwd.display());
    println!("sessions_dir:         {}", sessions_dir.display());
    println!("summaries.len():      {}", summaries.len());
    println!("initial_screen:       {initial_screen}");
    println!(
        "HOME:                  {}",
        std::env::var("HOME").unwrap_or_else(|_| "UNSET".to_string())
    );
    println!("===== END DIAGNOSTIC =====\n");
}
