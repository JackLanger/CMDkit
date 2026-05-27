use std::sync::Arc;

use cli_core::{CliCore, Command, CommandMetaData, StrategyError, cli};

#[cli]
fn simple_cli_strategy() -> Result<(), StrategyError> {
    Ok(())
}

#[test]
fn cli_attribute_generates_no_arg_strategy_wrapper() {
    let core = CliCore::new();
    core.register(Command {
        metadata: CommandMetaData {
            name: "simple".to_string(),
            description: "simple cli strategy".to_string(),
        },
        strategy: Arc::new(SimpleCliStrategy::new()),
        children: Vec::new(),
    });

    let args = vec!["app".to_string(), "simple".to_string(), "extra".to_string()];
    assert!(core.try_run_from_args(&args).is_ok());
}

#[test]
fn cli_attribute_generated_type_uses_upper_camel_name() {
    let _instance = SimpleCliStrategy::new();
}
