use augur_cli::wiring::{
    build_registry, spawn_core_runtime, BuildRegistryArgs, OptionalToolArgs, RegistryDirectoryScope,
};
use augur_domain::config::types::ProgramSettings;
use augur_domain::domain::{StringNewtype, ToolName};

#[test]
fn mirrored_surface_smoke_infrastructure() {
    let type_name = core::any::type_name::<BuildRegistryArgs>();
    assert!(type_name.contains("BuildRegistryArgs"));
    let function_name = core::any::type_name_of_val(&build_registry);
    assert!(function_name.contains("build_registry"));
    let function_name = core::any::type_name_of_val(&spawn_core_runtime);
    assert!(function_name.contains("spawn_core_runtime"));
}

#[tokio::test]
async fn s01_build_registry_with_some_lsp_handle_includes_lsp_query_tool() {
    let (query_tx, _rx) = tokio::sync::mpsc::channel(1);
    let (_fr_join, file_read) = augur_core::actors::file_read::file_read_actor::spawn(vec![]);
    let (_lsp_join, lsp_handle) = augur_core::actors::lsp::lsp_actor::spawn(
        augur_core::actors::lsp::lsp_actor::LspActorConfig {
            root_uri: "file:///tmp".to_string().into(),
        },
    );
    let registry = build_registry(BuildRegistryArgs {
        query_tx,
        file_read,
        cache: None,
        dirs: RegistryDirectoryScope {
            allowed_dirs: vec![],
            excluded_dirs: vec![],
        },
        optional: OptionalToolArgs {
            spawn_agent: None,
            lsp: Some(lsp_handle),
        },
    });
    let names: Vec<&str> = registry
        .definitions()
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    assert!(
        names.contains(&"lsp_query"),
        "expected lsp_query in registry when lsp handle is Some; got: {names:?}"
    );
}

#[tokio::test]
async fn s02_build_registry_with_none_lsp_excludes_lsp_query_tool() {
    let (query_tx, _rx) = tokio::sync::mpsc::channel(1);
    let (_fr_join, file_read) = augur_core::actors::file_read::file_read_actor::spawn(vec![]);
    let registry = build_registry(BuildRegistryArgs {
        query_tx,
        file_read,
        cache: None,
        dirs: RegistryDirectoryScope {
            allowed_dirs: vec![],
            excluded_dirs: vec![],
        },
        optional: OptionalToolArgs {
            spawn_agent: None,
            lsp: None,
        },
    });
    let names: Vec<&str> = registry
        .definitions()
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    assert!(
        !names.contains(&"lsp_query"),
        "expected lsp_query absent from registry when lsp handle is None; got: {names:?}"
    );
}

#[tokio::test]
async fn build_registry_uses_program_settings_exclusions_for_list_directory() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let root = temp_dir.path();
    std::fs::create_dir(root.join("changelogs")).expect("mkdir changelogs");
    std::fs::create_dir(root.join("visible")).expect("mkdir visible");
    std::fs::write(root.join("changelogs").join("hidden.txt"), "hidden").expect("write hidden");
    std::fs::write(root.join("visible").join("shown.txt"), "shown").expect("write shown");

    let (query_tx, _rx) = tokio::sync::mpsc::channel(1);
    let (_fr_join, file_read) = augur_core::actors::file_read::file_read_actor::spawn(vec![]);
    let settings = ProgramSettings::default();
    let registry = build_registry(BuildRegistryArgs {
        query_tx,
        file_read,
        cache: None,
        dirs: RegistryDirectoryScope {
            allowed_dirs: vec![root.to_path_buf()],
            excluded_dirs: settings.excluded_directory_paths(),
        },
        optional: OptionalToolArgs {
            spawn_agent: None,
            lsp: None,
        },
    });

    let tool = registry
        .find(&ToolName::new("list_directory"))
        .expect("list_directory tool");
    let result = tool
        .execute(serde_json::json!({
            "path": root.to_str().expect("root path str"),
            "recursive": true
        }))
        .await;

    assert!(!result.is_error);
    let output = result.output.as_str();
    assert!(output.contains("visible/"));
    assert!(output.contains("shown.txt"));
    assert!(!output.contains("changelogs/"));
    assert!(!output.contains("hidden.txt"));
}
