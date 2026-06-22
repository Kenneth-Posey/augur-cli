use augur_provider_anthropic::stream_complete;

#[test]
fn exports_anthropic_stream_function() {
    let function_name = core::any::type_name_of_val(&stream_complete);

    assert!(function_name.contains("stream_complete"));
}
