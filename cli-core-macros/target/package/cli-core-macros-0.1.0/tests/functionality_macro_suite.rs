use std::{
    env,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cli_core::{CliCore, Command, StrategyError};
use cli_core_macros::cli;

#[cli]
fn simple_cli_strategy(
    &self,
    _options: Vec<String>,
    _arguments: std::collections::HashMap<String, String>,
    _subcommands: Vec<String>,
) -> Result<(), StrategyError> {
    Ok(())
}

#[test]
fn cli_attribute_generates_execute_shaped_strategy_wrapper() {
    let core = CliCore::new();
    core.register(Command::new(
        "simple",
        "simple cli strategy",
        SimpleCliStrategy::new(),
    ));

    let args = vec![
        "app".to_string(),
        "simple".to_string(),
        "--extra".to_string(),
    ];
    assert!(core.try_run_from_args(&args).is_ok());
}

#[test]
fn cli_attribute_generated_type_uses_upper_camel_name() {
    let _instance = SimpleCliStrategy::new();
}

#[cli]
fn create_directory(
    &self,
    _options: Vec<String>,
    arguments: std::collections::HashMap<String, String>,
    _subcommands: Vec<String>,
) -> Result<(), StrategyError> {
    let path = arguments
        .get("path")
        .ok_or_else(|| StrategyError::invalid_arguments("missing path"))?;

    std::fs::create_dir(std::path::Path::new(path))
        .map_err(|e| StrategyError::execution(format!("Failed to create directory: {e}")))
}

#[test]
fn test_command_suit() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let dir_path: PathBuf = env::temp_dir().join(format!("cli-core-create-{unique}"));

    let core = CliCore::new();
    core.register(Command::new(
        "create",
        "Create a directory",
        CreateDirectory::new(),
    ));

    let args = vec![
        "app".to_string(),
        "create".to_string(),
        "--path".to_string(),
        dir_path.to_string_lossy().into_owned(),
    ];

    assert!(core.try_run_from_args(&args).is_ok());
    assert!(dir_path.exists());
    std::fs::remove_dir(&dir_path).expect("Failed to clean up test directory");
}
