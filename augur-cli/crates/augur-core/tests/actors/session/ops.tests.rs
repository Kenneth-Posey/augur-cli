use augur_core::actors::session::session_ops::SessionCommand;
use augur_domain::domain::string_newtypes::{EndpointName, StringNewtype};

#[test]
fn set_endpoint_variant_holds_endpoint_name() {
    let command = SessionCommand::SetEndpoint(EndpointName::new("openai"));
    match command {
        SessionCommand::SetEndpoint(endpoint) => assert_eq!(endpoint.as_str(), "openai"),
        SessionCommand::Shutdown => panic!("expected SetEndpoint"),
    }
}

#[test]
fn shutdown_variant_is_available() {
    let command = SessionCommand::Shutdown;
    assert!(matches!(command, SessionCommand::Shutdown));
}
