use std::sync::{Arc, Mutex};

use cli_core::{
    CLIStrategy, CliCore, CliCoreError, Functionality, StrategyError, StrategyErrorKind,
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

    fn help(&self) -> String {
        "recorder strategy".to_string()
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
    }
}

#[test]
fn help_is_registered_by_default() {
    let core = CliCore::new();
    let help = core.get("help").expect("help functionality should exist");

    assert_eq!(help.name, "help");
    assert_eq!(help.description, "Display help information");
    assert!(help.strategy.help().contains("supported commands:"));
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
