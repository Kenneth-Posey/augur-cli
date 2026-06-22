use augur_provider_openai::{stream_complete, stream_openai_compat};

#[test]
fn exports_openai_stream_functions() {
    let complete_name = core::any::type_name_of_val(&stream_complete);
    let compat_name = core::any::type_name_of_val(&stream_openai_compat);

    assert!(complete_name.contains("stream_complete"));
    assert!(compat_name.contains("stream_openai_compat"));
}
