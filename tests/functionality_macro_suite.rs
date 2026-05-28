use std::sync::Arc;

use cli_core::{CliCore, Command, CommandMetaData, cli};

#[cli]
fn simple_cli_strategy(&self, _args: Vec<String>) -> Result<(), cli_core::StrategyError> {
    Ok(())
}

#[test]
fn cli_attribute_generates_execute_shaped_strategy_wrapper() {
    let core = CliCore::new();
    core.register(Command {
        metadata: CommandMetaData::new("simple", "simple cli strategy"),
        strategy: Arc::new(SimpleCliStrategy::new()),
    });

    let args = vec!["app".to_string(), "simple".to_string(), "extra".to_string()];
    assert!(core.try_run_from_args(&args).is_ok());
}

#[test]
fn cli_attribute_generated_type_uses_upper_camel_name() {
    let _instance = SimpleCliStrategy::new();
}
