use augur_provider_copilot_sdk::actors::copilot::background_agent::{
    BackgroundAgentArgs, BackgroundAgentConfig, run_background_agent,
};

#[test]
fn mirrored_surface_smoke_background_agent_symbols() {
    assert!(core::any::type_name::<BackgroundAgentConfig>().contains("BackgroundAgentConfig"));
    assert!(core::any::type_name::<BackgroundAgentArgs>().contains("BackgroundAgentArgs"));
    let _ = run_background_agent;
}
