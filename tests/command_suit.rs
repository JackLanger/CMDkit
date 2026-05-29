use std::{
    env,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use cli_core::{CliCore, Command, StrategyError, cli, command};

#[cli]
fn ok_strategy(
    &self,
    _options: Vec<String>,
    _arguments: std::collections::HashMap<String, String>,
    _subcommands: Vec<String>,
) -> Result<(), StrategyError> {
    Ok(())
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
fn strategy_chain_handles_subtask_tokens() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_for_strategy = Arc::clone(&captured);

    let core = CliCore::new();
    core.register(Command::from_fn(
        "parent",
        "Parent command",
        move |options, arguments, subcommands| {
            let mut guard = captured_for_strategy
                .lock()
                .expect("capture lock should not be poisoned");
            guard.push(vec![
                format!("options={options:?}"),
                format!("arguments={arguments:?}"),
                format!("subcommands={subcommands:?}"),
            ]);
            Ok(())
        },
    ));

    let args = vec![
        "app".to_string(),
        "parent".to_string(),
        "--opt".to_string(),
        "--flag".to_string(),
        "value".to_string(),
    ];
    assert!(core.try_run_from_args(&args).is_ok());

    let guard = captured
        .lock()
        .expect("capture lock should not be poisoned");
    assert_eq!(guard.len(), 1);
    assert!(guard[0][0].contains("options=[\"opt\"]"));
    assert!(guard[0][1].contains("flag"));
    assert!(guard[0][1].contains("value"));
    assert!(guard[0][2].contains("[]"));
}

#[test]
fn test_command_suit() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let dir_path: PathBuf = env::temp_dir().join(format!("cli-core-create-{unique}"));

    let core = CliCore::new();
    core.register(Command::new("create", "Create a directory", CreateDirectory::new()));

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

#[test]
fn command_builder_registers_leaf_command_without_exposing_strategy_types() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_for_handler = Arc::clone(&captured);

    let core = CliCore::new();
    core.register(
        command("echo", "Echo command")
            .handler_fn(move |options, arguments, subcommands| {
                captured_for_handler
                    .lock()
                    .expect("capture lock should not be poisoned")
                    .push(vec![
                        format!("options={options:?}"),
                        format!("arguments={arguments:?}"),
                        format!("subcommands={subcommands:?}"),
                    ]);
                Ok(())
            })
            .build(),
    );

    let args = vec![
        "app".to_string(),
        "echo".to_string(),
        "--message".to_string(),
        "hello".to_string(),
    ];

    assert!(core.try_run_from_args(&args).is_ok());
    let guard = captured
        .lock()
        .expect("capture lock should not be poisoned");
    assert!(guard[0][0].contains("options=[]"));
    assert!(guard[0][1].contains("message"));
    assert!(guard[0][1].contains("hello"));
    assert!(guard[0][2].contains("[]"));
}

#[test]
fn command_builder_registers_recursive_subcommands_without_router_exposure() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_for_handler = Arc::clone(&captured);

    let core = CliCore::new();
    core.register(
        command("tool", "tool root")
            .subcommand(command("run", "run tasks").subcommand(
                command("one", "run one task").handler_fn(move |options, arguments, subcommands| {
                    captured_for_handler
                        .lock()
                        .expect("capture lock should not be poisoned")
                        .push(vec![
                            format!("options={options:?}"),
                            format!("arguments={arguments:?}"),
                            format!("subcommands={subcommands:?}"),
                        ]);
                    Ok(())
                }),
            ))
            .build(),
    );

    let args = vec![
        "app".to_string(),
        "tool".to_string(),
        "run".to_string(),
        "one".to_string(),
        "--value".to_string(),
        "alpha".to_string(),
    ];

    assert!(core.try_run_from_args(&args).is_ok());
    let guard = captured
        .lock()
        .expect("capture lock should not be poisoned");
    assert!(guard[0][0].contains("options=[]"));
    assert!(guard[0][1].contains("value"));
    assert!(guard[0][1].contains("alpha"));
    assert!(guard[0][2].contains("[]"));
}
