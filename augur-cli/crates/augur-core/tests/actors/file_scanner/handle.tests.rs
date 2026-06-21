use augur_core::actors::file_scanner::spawn;

/// Verifies FileScannerHandle::latest returns empty vec before any scan.
#[test]
fn handle_latest_empty_before_scan() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let (_join, handle) = spawn();
        assert!(handle.latest().is_empty());
        handle.shutdown();
    });
}

/// Verifies FileScannerHandle::scan triggers results visible via latest().
#[test]
fn handle_scan_produces_results() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let (_join, handle) = spawn();
        handle.scan("Cargo");
        // Give the actor time to process the scan command.
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let results = handle.latest();
        let names: Vec<String> = results.iter().map(|c| c.display_name.to_string()).collect();
        assert!(
            names
                .iter()
                .any(|n| *n == "Cargo.toml" || *n == "Cargo.lock"),
            "expected Cargo files in results, got: {:?}",
            names
        );
        handle.shutdown();
    });
}

/// Verifies FileScannerHandle::latest is non-blocking (returns immediately).
#[test]
fn handle_latest_is_nonblocking() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let (_join, handle) = spawn();
        // latest() must not block - calling it in a tight loop is safe.
        for _ in 0..100 {
            let _ = handle.latest();
        }
        handle.shutdown();
    });
}

/// Verifies FileScannerHandle::scan with an unknown prefix results in empty latest().
#[test]
fn handle_scan_unknown_prefix_empty_results() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let (_join, handle) = spawn();
        handle.scan("zzz_no_match_xyz");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        assert!(handle.latest().is_empty());
        handle.shutdown();
    });
}
