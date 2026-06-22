use augur_provider_ollama::stream_complete;

#[test]
fn exports_ollama_stream_function() {
    let function_name = core::any::type_name_of_val(&stream_complete);

    assert!(function_name.contains("stream_complete"));
}
