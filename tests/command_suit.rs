use std::sync::{Arc, Mutex};

use cmdkit::{Argument, ArgumentValue, CMDKit, argument, command, switch};

fn format_switches(switches: &[String]) -> String {
    switches.to_vec().join(",")
}

fn format_arguments(arguments: &[Argument]) -> String {
    arguments
        .iter()
        .map(|argument| match argument.value {
            ArgumentValue::String(ref value) => format!("{}={value}", argument.name),
            ArgumentValue::Int(value) => format!("{}={value}", argument.name),
            ArgumentValue::Float(value) => format!("{}={value}", argument.name),
            _ => argument.name.to_string(),
        })
        .collect::<Vec<String>>()
        .join(",")
}

#[test]
fn strategy_chain_handles_subtask_tokens() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_for_strategy = Arc::clone(&captured);
    let core = CMDKit::builder()
        .register(
            command("parent", "Parent command")
                .handler_fn(move |_, invocation| {
                    let options = invocation.switches;
                    let arguments = invocation.args;
                    let params = invocation.params;
                    let mut guard = captured_for_strategy
                        .lock()
                        .expect("capture lock should not be poisoned");
                    guard.push(vec![
                        format!("options={}", format_switches(&options)),
                        format!("arguments={}", format_arguments(&arguments)),
                        format!("params={params:?}"),
                    ]);
                    Ok(())
                })
                .with_options(vec![switch("opt", "option")])
                .with_arguments(vec![argument("flag", "flag")])
                .build(),
        )
        .build();

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
    assert!(guard[0][0].contains("options=opt"));
    assert!(guard[0][1].contains("flag=value"));
    assert!(guard[0][2].contains("[]"));
}

#[test]
fn command_builder_registers_leaf_command_without_exposing_strategy_types() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_for_handler = Arc::clone(&captured);

    let core = CMDKit::builder()
        .register(
            command("echo", "Echo command")
                .handler_fn(move |_, invocation| {
                    let options = invocation.switches;
                    let arguments = invocation.args;
                    let params = invocation.params;
                    captured_for_handler
                        .lock()
                        .expect("capture lock should not be poisoned")
                        .push(vec![
                            format!("options={}", format_switches(&options)),
                            format!("arguments={}", format_arguments(&arguments)),
                            format!("params={params:?}"),
                        ]);
                    Ok(())
                })
                .with_arguments(vec![argument("message", "message")])
                .build(),
        )
        .build();

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
    assert!(guard[0][0].contains("options="));
    assert!(guard[0][1].contains("message=hello"));
    assert!(guard[0][2].contains("[]"));
}

#[test]
fn command_builder_registers_recursive_subcommands_without_router_exposure() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_for_handler = Arc::clone(&captured);

    let core = CMDKit::builder()
        .register(
            command("tool", "tool root")
                .subcommand(
                    command("run", "run tasks").subcommand(
                        command("one", "run one task")
                            .handler_fn(move |_, invocation| {
                                let options = invocation.switches;
                                let arguments = invocation.args;
                                let params = invocation.params;
                                captured_for_handler
                                    .lock()
                                    .expect("capture lock should not be poisoned")
                                    .push(vec![
                                        format!("options={}", format_switches(&options)),
                                        format!("arguments={}", format_arguments(&arguments)),
                                        format!("params={params:?}"),
                                    ]);
                                Ok(())
                            })
                            .with_arguments(vec![argument("value", "value")]),
                    ),
                )
                .build(),
        )
        .build();

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
    assert!(guard[0][0].contains("options="));
    assert!(guard[0][1].contains("value=alpha"));
    assert!(guard[0][2].contains("[]"));
}
