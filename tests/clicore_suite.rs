use std::{
    process::Command as ProcessCommand,
    sync::{Arc, Mutex},
};

use cli_core::{
    CliCore, CliCoreError, Command, CommandMetaData, CommandStrategy, StrategyError,
    StrategyErrorKind, SubcommandRouter,
};

struct RecorderStrategy {
    calls: Arc<Mutex<Vec<Vec<String>>>>,
    error: Option<StrategyError>,
}

impl CommandStrategy for RecorderStrategy {
    fn execute(&self, args: Vec<String>) -> Result<(), StrategyError> {
        let mut guard = self.calls.lock().expect("call log lock poisoned");
        guard.push(args);

        if let Some(err) = &self.error {
            return Err(err.clone());
        }

        Ok(())
    }
}

fn build_recorder_functionality(
    name: &str,
    description: &str,
    calls: Arc<Mutex<Vec<Vec<String>>>>,
    error: Option<StrategyError>,
) -> Command {
    Command {
        metadata: CommandMetaData::new(name, description),
        strategy: Arc::new(RecorderStrategy { calls, error }),
    }
}

#[test]
fn register_and_get_by_name_works() {
    let core = CliCore::new();
    let calls = Arc::new(Mutex::new(Vec::new()));

    core.register(build_recorder_functionality(
        "echo",
        "echo arguments",
        Arc::clone(&calls),
        None,
    ));

    let got = core
        .get("echo")
        .expect("echo functionality should be found");
    assert_eq!(got.metadata.name, "echo");
    assert_eq!(got.metadata.description, "echo arguments");
}

#[test]
fn duplicate_registration_overwrites_previous_entry() {
    let core = CliCore::new();
    let calls = Arc::new(Mutex::new(Vec::new()));

    core.register(build_recorder_functionality(
        "dup",
        "first",
        Arc::clone(&calls),
        None,
    ));
    core.register(build_recorder_functionality(
        "dup",
        "second",
        Arc::clone(&calls),
        None,
    ));

    let got = core.get("dup").expect("dup functionality should exist");
    assert_eq!(got.metadata.description, "second");
}

#[test]
fn run_from_args_routes_trailing_arguments_to_strategy() {
    let core = CliCore::new();
    let calls = Arc::new(Mutex::new(Vec::new()));

    core.register(build_recorder_functionality(
        "echo",
        "echo arguments",
        Arc::clone(&calls),
        None,
    ));

    let args = vec![
        "app".to_string(),
        "echo".to_string(),
        "one".to_string(),
        "two".to_string(),
    ];

    let result = core.try_run_from_args(&args);
    assert!(result.is_ok());

    let guard = calls.lock().expect("call log lock poisoned");
    assert_eq!(guard.len(), 1);
    assert_eq!(guard[0], vec!["one".to_string(), "two".to_string()]);
}

#[test]
fn strategy_receives_subtask_tokens_for_chain_of_responsibility() {
    let core = CliCore::new();
    let calls = Arc::new(Mutex::new(Vec::new()));

    core.register(build_recorder_functionality(
        "test",
        "test root",
        Arc::clone(&calls),
        None,
    ));

    let args = vec![
        "cli-core".to_string(),
        "test".to_string(),
        "all".to_string(),
        "--fast".to_string(),
    ];

    let result = core.try_run_from_args(&args);
    assert!(result.is_ok());

    let guard = calls.lock().expect("call log lock poisoned");
    assert_eq!(guard.len(), 1);
    assert_eq!(guard[0], vec!["all".to_string(), "--fast".to_string()]);
}

#[test]
fn functionality_from_fn_supports_function_based_strategy_registration() {
    let core = CliCore::new();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let calls_for_strategy = Arc::clone(&calls);

    core.register(Command::from_fn(
        "fncmd",
        "defined from a function",
        move |args: Vec<String>| {
            let mut guard = calls_for_strategy.lock().expect("call log lock poisoned");
            guard.push(args);
            Ok(())
        },
    ));

    let args = vec![
        "cli-core".to_string(),
        "fncmd".to_string(),
        "alpha".to_string(),
    ];
    let result = core.try_run_from_args(&args);
    assert!(result.is_ok());

    let guard = calls.lock().expect("call log lock poisoned");
    assert_eq!(guard.len(), 1);
    assert_eq!(guard[0], vec!["alpha".to_string()]);
}

#[test]
fn run_from_args_returns_missing_command_error() {
    let core = CliCore::new();
    let args = vec!["app".to_string()];

    let result = core.try_run_from_args(&args);

    match result {
        Err(CliCoreError::MissingCommand { help }) => {
            assert!(help.contains("Usage:"));
        }
        _ => panic!("expected missing command error"),
    }
}

#[test]
fn help_text_uses_explicit_binary_name_for_missing_command() {
    let core = CliCore::new();
    let args = vec!["custom-cli".to_string()];

    let result = core.try_run_from_args(&args);

    match result {
        Err(CliCoreError::MissingCommand { help }) => {
            assert!(help.contains("Usage: custom-cli <command> [args...]"));
        }
        _ => panic!("expected missing command error"),
    }
}

#[test]
fn help_text_uses_explicit_binary_name_for_unknown_command() {
    let core = CliCore::new();
    let args = vec!["custom-cli".to_string(), "not-a-command".to_string()];

    let result = core.try_run_from_args(&args);

    match result {
        Err(CliCoreError::UnknownCommand { help, .. }) => {
            assert!(help.contains("Usage: custom-cli <command> [args...]"));
        }
        _ => panic!("expected unknown command error"),
    }
}

#[test]
fn run_from_args_returns_unknown_command_error() {
    let core = CliCore::new();
    let args = vec!["app".to_string(), "unknown".to_string()];

    let result = core.try_run_from_args(&args);

    match result {
        Err(CliCoreError::UnknownCommand { command, help }) => {
            assert_eq!(command, "unknown");
            assert!(help.contains("supported commands:"));
        }
        _ => panic!("expected unknown command error"),
    }
}

#[test]
fn strategy_errors_bubble_with_kind_and_message() {
    let core = CliCore::new();
    let calls = Arc::new(Mutex::new(Vec::new()));

    core.register(build_recorder_functionality(
        "validate",
        "fails validation",
        Arc::clone(&calls),
        Some(StrategyError::new(
            StrategyErrorKind::InvalidArguments,
            "missing value",
        )),
    ));

    let args = vec!["app".to_string(), "validate".to_string()];
    let result = core.try_run_from_args(&args);

    match result {
        Err(CliCoreError::StrategyExecution { command, source }) => {
            assert_eq!(command, "validate");
            assert_eq!(source.kind, StrategyErrorKind::InvalidArguments);
            assert_eq!(source.message, "missing value");
        }
        _ => panic!("expected strategy execution error"),
    }
}

#[test]
fn independent_instances_do_not_share_registry_entries() {
    let core_a = CliCore::new();
    let core_b = CliCore::new();
    let calls = Arc::new(Mutex::new(Vec::new()));

    core_a.register(build_recorder_functionality(
        "isolated",
        "only in core_a",
        Arc::clone(&calls),
        None,
    ));

    assert!(core_a.get("isolated").is_some());
    assert!(core_b.get("isolated").is_none());
}

#[test]
fn wrapper_calls_do_not_share_runtime_state() {
    let binary = std::env::var("CARGO_BIN_EXE_wrapper_probe")
        .expect("wrapper_probe binary should be built by cargo test");

    let output = ProcessCommand::new(binary)
        .arg("help")
        .output()
        .expect("wrapper_probe should run successfully");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let first_section = stdout
        .split("--SECOND--")
        .next()
        .expect("first section should exist");
    let second_section = stdout
        .split("--SECOND--")
        .nth(1)
        .expect("second section should exist");

    assert!(first_section.contains("alpha command"));
    assert!(!first_section.contains("beta command"));
    assert!(second_section.contains("beta command"));
    assert!(!second_section.contains("alpha command"));
}

#[test]
fn subcommand_router_dispatches_recursively_to_deep_children() {
    let core = CliCore::new();
    let deep_calls = Arc::new(Mutex::new(Vec::new()));
    let deep_calls_for_leaf = Arc::clone(&deep_calls);

    let deep_leaf = Command::from_fn("leaf", "deep leaf", move |args| {
        deep_calls_for_leaf
            .lock()
            .expect("call log lock poisoned")
            .push(args);
        Ok(())
    });

    let level_two = SubcommandRouter::new().register(Command {
        metadata: CommandMetaData::new("level2", "second level"),
        strategy: Arc::new(SubcommandRouter::new().register(deep_leaf)),
    });

    let root = Command {
        metadata: CommandMetaData::new("tool", "tool root"),
        strategy: Arc::new(SubcommandRouter::new().register(Command {
            metadata: CommandMetaData::new("level1", "first level"),
            strategy: Arc::new(level_two),
        })),
    };

    core.register(root);

    let args = vec![
        "app".to_string(),
        "tool".to_string(),
        "level1".to_string(),
        "level2".to_string(),
        "leaf".to_string(),
        "--flag".to_string(),
        "value".to_string(),
    ];

    assert!(core.try_run_from_args(&args).is_ok());

    let guard = deep_calls.lock().expect("call log lock poisoned");
    assert_eq!(guard.len(), 1);
    assert_eq!(guard[0], vec!["--flag".to_string(), "value".to_string()]);
}

#[test]
fn help_renderer_includes_recursive_subcommands_from_router_catalog() {
    let core = CliCore::new();

    let root = Command {
        metadata: CommandMetaData::new("tool", "tool root"),
        strategy: Arc::new(SubcommandRouter::new().register(Command {
            metadata: CommandMetaData::new("child", "child command"),
            strategy: Arc::new(SubcommandRouter::new().register(Command::from_fn(
                "leaf",
                "leaf command",
                |_| Ok(()),
            ))),
        })),
    };

    core.register(root);

    let args = vec!["app".to_string()];
    let result = core.try_run_from_args(&args);

    match result {
        Err(CliCoreError::MissingCommand { help }) => {
            assert!(help.contains("tool: tool root"));
            assert!(help.contains("tool child: child command"));
            assert!(help.contains("tool child leaf: leaf command"));
        }
        _ => panic!("expected missing command error"),
    }
}
