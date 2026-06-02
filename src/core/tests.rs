use std::sync::{Arc, Mutex};

use crate::{
    CMDKit, CMDKitError, Command, CoreConfig, InvocationArgs, StrategyError, argument, command,
};

struct MarkerHelpRenderer;

impl crate::core::HelpRenderer for MarkerHelpRenderer {
    fn render(&self, caller: &str, _registered_commands: &[Command]) -> String {
        format!("marker-help:{caller}")
    }
}

struct FixedInterpreter {
    command_name: String,
}

impl crate::core::InvocationInterpreter for FixedInterpreter {
    fn interpret(
        &self,
        _arg: &[String],
        _registered_commands: &[Command],
    ) -> Result<InvocationArgs, CMDKitError> {
        Ok(InvocationArgs {
            name: self.command_name.clone(),
            args: vec![argument("path", "path arg").set_value("from-fixed")],
            switches: Vec::new(),
            params: Vec::new(),
            order: Vec::new(),
            subcommand: None,
        })
    }
}

#[test]
fn core_config_defaults_to_plain_text_help_renderer() {
    let config = CoreConfig::new();
    let text = config.help_renderer.render("app", &[]);
    assert!(text.contains("Usage:"));
}

#[test]
fn core_config_can_replace_help_renderer() {
    let config = CoreConfig::new().with_help_renderer(MarkerHelpRenderer);
    let text = config.help_renderer.render("app", &[]);
    assert_eq!(text, "marker-help:app");
}

#[test]
fn invocation_leaf_name_returns_deepest_nested_name() {
    let invocation = InvocationArgs {
        name: "root".to_string(),
        args: Vec::new(),
        switches: Vec::new(),
        params: Vec::new(),
        order: Vec::new(),
        subcommand: Some(Box::new(InvocationArgs {
            name: "child".to_string(),
            args: Vec::new(),
            switches: Vec::new(),
            params: Vec::new(),
            order: Vec::new(),
            subcommand: Some(Box::new(InvocationArgs {
                name: "leaf".to_string(),
                args: Vec::new(),
                switches: Vec::new(),
                params: Vec::new(),
                order: Vec::new(),
                subcommand: None,
            })),
        })),
    };

    assert_eq!(invocation.leaf_name(), "leaf");
}

#[test]
fn cmdkit_error_display_and_source_are_structured() {
    let missing = CMDKitError::MissingCommand {
        help: "help text".to_string(),
    };
    assert!(missing.to_string().contains("No command provided"));
    assert!(std::error::Error::source(&missing).is_none());

    let unknown = CMDKitError::UnknownCommand {
        command: "ghost".to_string(),
        help: "help text".to_string(),
    };
    assert!(unknown.to_string().contains("Unknown command: ghost"));
    assert!(std::error::Error::source(&unknown).is_none());

    let strategy = StrategyError::execution("failed to execute");
    let strategy_execution = CMDKitError::StrategyExecution {
        command: "run".to_string(),
        source: strategy,
    };
    assert!(
        strategy_execution
            .to_string()
            .contains("Strategy execution failed for 'run': execution: failed to execute")
    );
    assert!(std::error::Error::source(&strategy_execution).is_some());
}

#[test]
fn builder_argument_interpreter_can_drive_with_commands_registration() {
    let calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let calls_for_handler = Arc::clone(&calls);

    let cmd = command("echo", "echo command")
        .handler_fn(move |_switches, arguments, _params| {
            let value = arguments
                .iter()
                .find(|arg| arg.name == "path")
                .and_then(|arg| arg.value.clone())
                .unwrap_or_default();
            calls_for_handler
                .lock()
                .expect("calls lock should not be poisoned")
                .push(value);
            Ok(())
        })
        .with_arguments(vec![argument("path", "path arg")])
        .build();

    let core = CMDKit::builder()
        .with_argument_interpreter(FixedInterpreter {
            command_name: "echo".to_string(),
        })
        .with_commands(&[cmd])
        .build();

    assert!(core.try_run_from_args(&["app".to_string()]).is_ok());

    let guard = calls.lock().expect("calls lock should not be poisoned");
    assert_eq!(guard.as_slice(), ["from-fixed"]);
}

#[test]
fn builder_with_config_uses_custom_renderer_for_missing_command_help() {
    let core = CMDKit::builder()
        .with_config(CoreConfig::new().with_help_renderer(MarkerHelpRenderer))
        .build();

    let result = core.try_run_from_args(&["custom".to_string()]);

    match result {
        Err(CMDKitError::MissingCommand { help }) => {
            assert_eq!(help, "marker-help:custom");
        }
        _ => panic!("expected missing command error"),
    }
}
