use crate::core::CoreConfig;

#[test]
fn core_config_defaults_to_plain_text_help_renderer() {
    let config = CoreConfig::new();
    let text = config.help_renderer.render("app", &[]);
    assert!(text.contains("Usage:"));
}
