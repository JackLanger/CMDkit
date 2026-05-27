use cli_core::{CliCore, StrategyError, functionality};

#[functionality(name = "macro_new", description = "macro-defined strategy")]
fn macro_new(args: Vec<String>) -> Result<(), StrategyError> {
    if args.is_empty() {
        return Err(StrategyError::invalid_arguments("missing argument"));
    }
    Ok(())
}

#[test]
fn attribute_macro_generates_functionality_factory() {
    let core = CliCore::new();
    core.register(macro_new_functionality());

    let registered = core
        .get("macro_new")
        .expect("macro-generated functionality should be registered");

    assert_eq!(registered.name, "macro_new");
    assert_eq!(registered.description, "macro-defined strategy");
}

#[test]
fn attribute_macro_generated_strategy_executes_function_body() {
    let core = CliCore::new();
    core.register(macro_new_functionality());

    let ok_args = vec![
        "app".to_string(),
        "macro_new".to_string(),
        "value".to_string(),
    ];
    assert!(core.try_run_from_args(&ok_args).is_ok());

    let err_args = vec!["app".to_string(), "macro_new".to_string()];
    assert!(core.try_run_from_args(&err_args).is_err());
}
