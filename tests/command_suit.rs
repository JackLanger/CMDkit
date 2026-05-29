use std::sync::{Arc, Mutex};

use cmdkit::{CliCore, Command, command};

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
                command("one", "run one task").handler_fn(
                    move |options, arguments, subcommands| {
                        captured_for_handler
                            .lock()
                            .expect("capture lock should not be poisoned")
                            .push(vec![
                                format!("options={options:?}"),
                                format!("arguments={arguments:?}"),
                                format!("subcommands={subcommands:?}"),
                            ]);
                        Ok(())
                    },
                ),
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
