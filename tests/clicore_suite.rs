use std::{
    process::Command,
    sync::{Arc, Mutex},
};

use cli_core::{
    CLIStrategy, CliCore, CliCoreError, Functionality, LockPoisonPolicy, StrategyError,
    StrategyErrorKind,
};

struct RecorderStrategy {
    calls: Arc<Mutex<Vec<Vec<String>>>>,
    error: Option<StrategyError>,
}

impl CLIStrategy for RecorderStrategy {
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
) -> Functionality {
    Functionality {
        name: name.to_string(),
        description: description.to_string(),
        strategy: Arc::new(RecorderStrategy { calls, error }),
        children: Vec::new(),
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
    assert_eq!(got.name, "echo");
    assert_eq!(got.description, "echo arguments");
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
    assert_eq!(got.description, "second");
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
fn run_from_args_supports_nested_command_paths() {
    let core = CliCore::new();
    let calls = Arc::new(Mutex::new(Vec::new()));

    core.register(build_recorder_functionality(
        "test all",
        "run all tests",
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
    assert_eq!(guard[0], vec!["--fast".to_string()]);
}

#[test]
fn nested_dispatch_prefers_longest_matching_command_path() {
    let core = CliCore::new();
    let parent_calls = Arc::new(Mutex::new(Vec::new()));
    let child_calls = Arc::new(Mutex::new(Vec::new()));

    core.register(build_recorder_functionality(
        "test",
        "run default test command",
        Arc::clone(&parent_calls),
        None,
    ));
    core.register(build_recorder_functionality(
        "test all",
        "run all tests",
        Arc::clone(&child_calls),
        None,
    ));

    let args = vec![
        "cli-core".to_string(),
        "test".to_string(),
        "all".to_string(),
        "target-a".to_string(),
    ];

    let result = core.try_run_from_args(&args);
    assert!(result.is_ok());

    let parent_guard = parent_calls.lock().expect("call log lock poisoned");
    let child_guard = child_calls.lock().expect("call log lock poisoned");
    assert!(parent_guard.is_empty());
    assert_eq!(child_guard.len(), 1);
    assert_eq!(child_guard[0], vec!["target-a".to_string()]);
}

#[test]
fn nested_tree_registration_routes_child_without_name_duplication() {
    let core = CliCore::new();
    let calls = Arc::new(Mutex::new(Vec::new()));

    core.register(Functionality {
        name: "test".to_string(),
        description: "test root".to_string(),
        strategy: Arc::new(RecorderStrategy {
            calls: Arc::new(Mutex::new(Vec::new())),
            error: None,
        }),
        children: vec![Functionality {
            name: "all".to_string(),
            description: "run all tests".to_string(),
            strategy: Arc::new(RecorderStrategy {
                calls: Arc::clone(&calls),
                error: None,
            }),
            children: Vec::new(),
        }],
    });

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
    assert_eq!(guard[0], vec!["--fast".to_string()]);
}

#[test]
fn get_children_returns_direct_nested_commands_with_full_paths() {
    let core = CliCore::new();

    core.register(Functionality {
        name: "test".to_string(),
        description: "test root".to_string(),
        strategy: Arc::new(RecorderStrategy {
            calls: Arc::new(Mutex::new(Vec::new())),
            error: None,
        }),
        children: vec![Functionality {
            name: "all".to_string(),
            description: "run all tests".to_string(),
            strategy: Arc::new(RecorderStrategy {
                calls: Arc::new(Mutex::new(Vec::new())),
                error: None,
            }),
            children: Vec::new(),
        }],
    });

    let children = core
        .get_children("test")
        .expect("test should have child commands");
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name, "test all");
    assert_eq!(children[0].description, "run all tests");
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

    let output = Command::new(binary)
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
fn lock_poison_policy_is_configurable() {
    let core = CliCore::new();
    assert_eq!(core.lock_poison_policy(), LockPoisonPolicy::FailFast);

    core.set_lock_poison_policy(LockPoisonPolicy::Recover);
    assert_eq!(core.lock_poison_policy(), LockPoisonPolicy::Recover);
}
