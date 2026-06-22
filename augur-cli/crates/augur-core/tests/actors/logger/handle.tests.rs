use augur_core::actors::logger::handle::LoggerHandle;

#[test]
fn logger_handle_surface_is_reexported() {
    let type_name = core::any::type_name::<LoggerHandle>();
    assert!(type_name.contains("LoggerHandle"));
}
