use std::sync::{Arc, Mutex};

use cmdkit::{
    Argument, CMDKit, CMDKitError, Command, CommandStrategy, StrategyError, StrategyErrorKind,
    SubcommandRouter, Switch, argument, command, switch,
};
type CallLog = Arc<Mutex<Vec<(Vec<Switch>, Vec<Argument>, Vec<String>)>>>;

struct RecorderStrategy {
    calls: CallLog,
    error: Option<StrategyError>,
}

impl CommandStrategy for RecorderStrategy {
    fn execute(
        &self,
        options: Vec<Switch>,
        arguments: Vec<Argument>,
        params: Vec<String>,
    ) -> Result<(), StrategyError> {
        let mut guard = self.calls.lock().expect("call log lock poisoned");
        guard.push((options, arguments, params));

        if let Some(err) = &self.error {
            return Err(err.clone());
        }

        Ok(())
    }
}

fn build_recorder_functionality(
    name: &str,
    description: &str,
    calls: CallLog,
    error: Option<StrategyError>,
) -> Command {
    Command::new(name, description, RecorderStrategy { calls, error })
}

fn has_switch(switches: &[Switch], name: &str) -> bool {
    switches.iter().any(|switch| switch.name == name)
}

fn argument_value<'a>(arguments: &'a [Argument], name: &str) -> Option<&'a str> {
    arguments
        .iter()
        .find(|argument| argument.name == name)
        .and_then(|argument| argument.value.as_deref())
}

#[test]
fn register_and_get_by_name_works() {
    let calls = Arc::new(Mutex::new(Vec::new()));

    let core = CMDKit::builder()
        .register(build_recorder_functionality(
            "echo",
            "echo arguments",
            Arc::clone(&calls),
            None,
        ))
        .build();

    let got = core
        .get("echo")
        .expect("echo functionality should be found");
    assert_eq!(got.metadata.name, "echo");
    assert_eq!(got.metadata.description, "echo arguments");
}

#[test]
fn duplicate_registration_overwrites_previous_entry() {
    let calls = Arc::new(Mutex::new(Vec::new()));

    let core = CMDKit::builder()
        .register(build_recorder_functionality(
            "dup",
            "first",
            Arc::clone(&calls),
            None,
        ))
        .register(build_recorder_functionality(
            "dup",
            "second",
            Arc::clone(&calls),
            None,
        ))
        .build();

    let got = core.get("dup").expect("dup functionality should exist");
    assert_eq!(got.metadata.description, "second");
}

#[test]
fn run_from_args_routes_trailing_arguments_to_strategy() {
    let calls = Arc::new(Mutex::new(Vec::new()));

    let core = CMDKit::builder()
        .register(
            command("echo", "echo arguments")
                .handler(RecorderStrategy {
                    calls: Arc::clone(&calls),
                    error: None,
                })
                .with_options(vec![switch("toggle", "toggle option")])
                .with_arguments(vec![argument("one", "one value")])
                .build(),
        )
        .build();

    let args = vec![
        "app".to_string(),
        "echo".to_string(),
        "--one".to_string(),
        "two".to_string(),
        "--toggle".to_string(),
    ];

    let result = core.try_run_from_args(&args);
    assert!(result.is_ok());

    let guard = calls.lock().expect("call log lock poisoned");
    assert_eq!(guard.len(), 1);
    assert!(has_switch(&guard[0].0, "toggle"));
    assert_eq!(argument_value(&guard[0].1, "one"), Some("two"));
    assert!(guard[0].2.is_empty());
}

#[test]
fn strategy_receives_subtask_tokens_for_chain_of_responsibility() {
    let calls = Arc::new(Mutex::new(Vec::new()));

    let core = CMDKit::builder()
        .register(
            command("test", "test root")
                .handler(RecorderStrategy {
                    calls: Arc::clone(&calls),
                    error: None,
                })
                .with_options(vec![switch("toggle", "toggle option")])
                .with_arguments(vec![argument("all", "all value")])
                .build(),
        )
        .build();

    let args = vec![
        "CMDkit".to_string(),
        "test".to_string(),
        "--all".to_string(),
        "fast".to_string(),
        "--toggle".to_string(),
    ];

    let result = core.try_run_from_args(&args);
    assert!(result.is_ok());

    let guard = calls.lock().expect("call log lock poisoned");
    assert_eq!(guard.len(), 1);
    assert!(has_switch(&guard[0].0, "toggle"));
    assert_eq!(argument_value(&guard[0].1, "all"), Some("fast"));
    assert!(guard[0].2.is_empty());
}

#[test]
fn functionality_from_fn_supports_function_based_strategy_registration() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let calls_for_strategy = Arc::clone(&calls);

    let core = CMDKit::builder()
        .register(
            command("fncmd", "defined from a function")
                .handler_fn(move |options, arguments, params| {
                    let mut guard = calls_for_strategy.lock().expect("call log lock poisoned");
                    guard.push((options, arguments, params));
                    Ok(())
                })
                .with_options(vec![switch("alpha", "alpha switch")])
                .build(),
        )
        .build();

    let args = vec![
        "CMDkit".to_string(),
        "fncmd".to_string(),
        "--alpha".to_string(),
    ];
    let result = core.try_run_from_args(&args);
    assert!(result.is_ok());

    let guard = calls.lock().expect("call log lock poisoned");
    assert_eq!(guard.len(), 1);
    assert!(has_switch(&guard[0].0, "alpha"));
    assert!(guard[0].1.is_empty());
    assert!(guard[0].2.is_empty());
}

#[test]
fn run_from_args_returns_missing_command_error() {
    let core = CMDKit::builder();
    let args = vec!["app".to_string()];

    let result = core.build().try_run_from_args(&args);

    match result {
        Err(CMDKitError::MissingCommand { help }) => {
            assert!(help.contains("Usage:"));
        }
        _ => panic!("expected missing command error"),
    }
}

#[test]
fn help_text_uses_explicit_binary_name_for_missing_command() {
    let core = CMDKit::builder();
    let args = vec!["custom-cli".to_string()];

    let result = core.build().try_run_from_args(&args);

    match result {
        Err(CMDKitError::MissingCommand { help }) => {
            assert!(help.contains("Usage: custom-cli <command> [args...]"));
        }
        _ => panic!("expected missing command error"),
    }
}

#[test]
fn help_text_uses_explicit_binary_name_for_unknown_command() {
    let core = CMDKit::builder();
    let args = vec!["custom-cli".to_string(), "not-a-command".to_string()];

    let result = core.build().try_run_from_args(&args);

    match result {
        Err(CMDKitError::UnknownCommand { help, .. }) => {
            assert!(help.contains("Usage: custom-cli <command> [args...]"));
        }
        _ => panic!("expected unknown command error"),
    }
}

#[test]
fn run_from_args_returns_unknown_command_error() {
    let core = CMDKit::builder().build();
    let args = vec!["app".to_string(), "unknown".to_string()];

    let result = core.try_run_from_args(&args);

    match result {
        Err(CMDKitError::UnknownCommand { command, help }) => {
            assert_eq!(command, "unknown");
            assert!(help.contains("supported commands:"));
        }
        _ => panic!("expected unknown command error"),
    }
}

#[test]
fn strategy_errors_bubble_with_kind_and_message() {
    let calls = Arc::new(Mutex::new(Vec::new()));

    let core = CMDKit::builder()
        .register(build_recorder_functionality(
            "validate",
            "fails validation",
            Arc::clone(&calls),
            Some(StrategyError::new(
                StrategyErrorKind::InvalidArguments,
                "missing value",
            )),
        ))
        .build();

    let args = vec!["app".to_string(), "validate".to_string()];
    let result = core.try_run_from_args(&args);

    match result {
        Err(CMDKitError::StrategyExecution { command, source }) => {
            assert_eq!(command, "validate");
            assert_eq!(source.kind, StrategyErrorKind::InvalidArguments);
            assert_eq!(source.message, "missing value");
        }
        _ => panic!("expected strategy execution error"),
    }
}

#[test]
fn parser_rejects_unknown_flags() {
    let calls = Arc::new(Mutex::new(Vec::new()));

    let core = CMDKit::builder()
        .register(
            command("strict", "strict command")
                .handler(RecorderStrategy {
                    calls: Arc::clone(&calls),
                    error: None,
                })
                .with_options(vec![switch("known", "known switch")])
                .with_arguments(vec![argument("path", "path argument")])
                .build(),
        )
        .build();

    let args = vec![
        "app".to_string(),
        "strict".to_string(),
        "--unknown".to_string(),
    ];

    let result = core.try_run_from_args(&args);
    match result {
        Err(CMDKitError::StrategyExecution { command, source }) => {
            assert_eq!(command, "strict");
            assert_eq!(source.kind, StrategyErrorKind::InvalidArguments);
            assert!(source.message.contains("unknown flag '--unknown'"));
        }
        _ => panic!("expected strategy execution error for unknown flag"),
    }
}

#[test]
fn parser_enforces_required_argument_presence_and_non_empty_values() {
    let calls = Arc::new(Mutex::new(Vec::new()));

    let core = CMDKit::builder()
        .register(
            command("strict", "strict command")
                .handler(RecorderStrategy {
                    calls: Arc::clone(&calls),
                    error: None,
                })
                .with_arguments(vec![argument("path", "path argument").set_required()])
                .build(),
        )
        .build();

    let missing_value = vec!["app".to_string(), "strict".to_string()];
    let missing_result = core.try_run_from_args(&missing_value);
    match missing_result {
        Err(CMDKitError::StrategyExecution { source, .. }) => {
            assert_eq!(source.kind, StrategyErrorKind::InvalidArguments);
            assert!(
                source
                    .message
                    .contains("missing value for required argument '--path'")
            );
        }
        _ => panic!("expected missing required argument error"),
    }

    let inline_empty = vec![
        "app".to_string(),
        "strict".to_string(),
        "--path=".to_string(),
    ];
    let inline_empty_result = core.try_run_from_args(&inline_empty);
    match inline_empty_result {
        Err(CMDKitError::StrategyExecution { source, .. }) => {
            assert_eq!(source.kind, StrategyErrorKind::InvalidArguments);
            assert!(
                source
                    .message
                    .contains("missing value for required argument '--path'")
            );
        }
        _ => panic!("expected required argument empty-value error"),
    }

    let next_empty = vec![
        "app".to_string(),
        "strict".to_string(),
        "--path".to_string(),
        "".to_string(),
    ];
    let next_empty_result = core.try_run_from_args(&next_empty);
    match next_empty_result {
        Err(CMDKitError::StrategyExecution { source, .. }) => {
            assert_eq!(source.kind, StrategyErrorKind::InvalidArguments);
            assert!(
                source
                    .message
                    .contains("missing value for required argument '--path'")
            );
        }
        _ => panic!("expected required argument empty-value error"),
    }
}

#[test]
fn parser_accepts_argument_aliases_and_uses_last_value_wins() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let calls_for_strategy = Arc::clone(&calls);

    let core = CMDKit::builder()
        .register(
            command("alias", "alias command")
                .handler_fn(move |options, arguments, params| {
                    let mut guard = calls_for_strategy.lock().expect("call log lock poisoned");
                    guard.push((options, arguments, params));
                    Ok(())
                })
                .with_arguments(vec![
                    argument("path", "path argument").with_aliases(vec!["p"]),
                ])
                .build(),
        )
        .build();

    let args = vec![
        "app".to_string(),
        "alias".to_string(),
        "--p".to_string(),
        "first".to_string(),
        "--path".to_string(),
        "second".to_string(),
    ];

    assert!(core.try_run_from_args(&args).is_ok());

    let guard = calls.lock().expect("call log lock poisoned");
    assert_eq!(guard.len(), 1);
    assert_eq!(guard[0].1.len(), 1);
    assert_eq!(guard[0].1[0].name, "path");
    assert_eq!(guard[0].1[0].value.as_deref(), Some("second"));
}

#[test]
fn independent_instances_do_not_share_registry_entries() {
    let core_b = CMDKit::builder().build();
    let calls = Arc::new(Mutex::new(Vec::new()));

    let core_a = CMDKit::builder()
        .register(build_recorder_functionality(
            "isolated",
            "only in core_a",
            Arc::clone(&calls),
            None,
        ))
        .build();

    assert!(core_a.get("isolated").is_some());
    assert!(core_b.get("isolated").is_none());
}

#[test]
fn subcommand_router_dispatches_recursively_to_deep_children() {
    let deep_calls = Arc::new(Mutex::new(Vec::new()));
    let deep_calls_for_leaf = Arc::clone(&deep_calls);

    let deep_leaf = command("leaf", "deep leaf")
        .handler_fn(move |options, arguments, params| {
            deep_calls_for_leaf
                .lock()
                .expect("call log lock poisoned")
                .push((options, arguments, params));
            Ok(())
        })
        .with_arguments(vec![argument("flag", "flag value")])
        .build();

    let level_two = Command::new(
        "level2",
        "second level",
        SubcommandRouter::new().register(deep_leaf),
    );

    let root = Command::new(
        "tool",
        "tool root",
        SubcommandRouter::new().register(Command::new(
            "level1",
            "first level",
            SubcommandRouter::new().register(level_two),
        )),
    );

    let core = CMDKit::builder().register(root).build();

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
    assert!(guard[0].0.is_empty());
    assert_eq!(argument_value(&guard[0].1, "flag"), Some("value"));
    assert!(guard[0].2.is_empty());
}

#[test]
fn subcommand_boundary_defers_flag_parsing_to_child_command() {
    let child_calls = Arc::new(Mutex::new(Vec::new()));
    let child_calls_for_strategy = Arc::clone(&child_calls);

    let run = command("run", "run command")
        .handler_fn(move |options, arguments, params| {
            child_calls_for_strategy
                .lock()
                .expect("call log lock poisoned")
                .push((options, arguments, params));
            Ok(())
        })
        .with_arguments(vec![argument("mode", "execution mode")])
        .build();

    let root = Command::new("tool", "tool root", SubcommandRouter::new().register(run));
    let core = CMDKit::builder().register(root).build();

    let args = vec![
        "app".to_string(),
        "tool".to_string(),
        "run".to_string(),
        "--mode".to_string(),
        "fast".to_string(),
    ];

    assert!(core.try_run_from_args(&args).is_ok());

    let guard = child_calls.lock().expect("call log lock poisoned");
    assert_eq!(guard.len(), 1);
    assert!(guard[0].0.is_empty());
    assert_eq!(argument_value(&guard[0].1, "mode"), Some("fast"));
    assert!(guard[0].2.is_empty());
}

#[test]
fn positional_tokens_before_subcommand_boundary_remain_current_level_params() {
    let run = command("run", "run command")
        .handler_fn(|_, _, _| Ok(()))
        .build();
    let root = Command::new("tool", "tool root", SubcommandRouter::new().register(run));
    let core = CMDKit::builder().register(root).build();

    let args = vec![
        "app".to_string(),
        "tool".to_string(),
        "pre".to_string(),
        "run".to_string(),
    ];

    let result = core.try_run_from_args(&args);
    match result {
        Err(CMDKitError::StrategyExecution { source, .. }) => {
            assert_eq!(source.kind, StrategyErrorKind::InvalidArguments);
            assert!(source.message.contains("unknown subcommand 'pre'"));
        }
        _ => panic!("expected unknown subcommand error"),
    }
}

#[test]
fn help_renderer_includes_recursive_subcommands_from_router_catalog() {
    let root = Command::new(
        "tool",
        "tool root",
        SubcommandRouter::new().register(Command::new(
            "child",
            "child command",
            SubcommandRouter::new().register(Command::from_fn(
                "leaf",
                "leaf command",
                |_, _, _| Ok(()),
            )),
        )),
    );

    let core = CMDKit::builder().register(root).build();

    let args = vec!["app".to_string()];
    let result = core.try_run_from_args(&args);

    match result {
        Err(CMDKitError::MissingCommand { help }) => {
            assert!(help.contains("tool: tool root"));
            assert!(help.contains("tool child: child command"));
            assert!(help.contains("tool child leaf: leaf command"));
        }
        _ => panic!("expected missing command error"),
    }
}

#[test]
fn help_renderer_includes_nested_catalogs_hidden_by_fallback_wrappers() {
    let root = command("tool", "tool root")
        .handler(SubcommandRouter::new().register(Command::new(
            "inner",
            "inner branch",
            SubcommandRouter::new().register(Command::from_fn(
                "leaf",
                "leaf command",
                |_, _, _| Ok(()),
            )),
        )))
        .subcommand(command("outer", "outer command").handler_fn(|_, _, _| Ok(())))
        .build();

    let core = CMDKit::builder().register(root).build();

    let args = vec!["app".to_string()];
    let result = core.try_run_from_args(&args);

    match result {
        Err(CMDKitError::MissingCommand { help }) => {
            assert!(help.contains("tool outer: outer command"));
            assert!(help.contains("tool inner: inner branch"));
            assert!(help.contains("tool inner leaf: leaf command"));
        }
        _ => panic!("expected missing command error"),
    }
}

#[test]
fn help_renderer_includes_optional_metadata_fields() {
    let cmd = command("build", "Build a project")
        .handler_fn(|_, _, _| Ok(()))
        .with_aliases(vec!["b", "compile"])
        .with_usage("build --path <dir> [--release]")
        .with_long_description("Builds the project artifacts for distribution")
        .with_examples(vec![
            "app build --path ./demo".to_string(),
            "app build --path ./demo --release".to_string(),
        ])
        .with_options(vec![
            switch("release", "Build in release mode").with_aliases(vec!["r".to_string()]),
        ])
        .with_arguments(vec![
            argument("path", "Project directory")
                .with_aliases(vec!["p"])
                .set_required(),
        ])
        .build();

    let core = CMDKit::builder().register(cmd).build();

    let args = vec!["app".to_string()];
    let result = core.try_run_from_args(&args);

    match result {
        Err(CMDKitError::MissingCommand { help }) => {
            assert!(help.contains("- build: Build a project"));
            assert!(help.contains("usage: build --path <dir> [--release]"));
            assert!(help.contains("details: Builds the project artifacts for distribution"));
            assert!(help.contains("aliases: b, compile"));
            assert!(help.contains("examples:"));
            assert!(help.contains("- app build --path ./demo"));
            assert!(help.contains("- app build --path ./demo --release"));
            assert!(help.contains("switches:"));
            assert!(help.contains("- --release (aliases: r): Build in release mode"));
            assert!(help.contains("arguments:"));
            assert!(help.contains("- --path [required] (aliases: p): Project directory"));
        }
        _ => panic!("expected missing command error"),
    }
}
