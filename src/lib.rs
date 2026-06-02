/// Public CLI primitives for implementation-first command trees, parsed strategy dispatch, and strategy error types.
pub mod cli;

/// Instance-owned CLI runtime, routing error types, and help rendering.
pub mod core;

pub use cli::{
    Argument, Command, CommandBuilder, CommandMetaData, CommandStrategy, FunctionStrategy,
    StrategyError, StrategyErrorKind, SubcommandCatalog, SubcommandRouter, Switch, argument,
    command, switch,
};
pub use core::{
    ArgumentInterpreter, CMDKit, CMDKitBuilder, CMDKitError, CoreConfig, HelpRenderer,
    InvocationArgs, InvocationElement, PlainTextArgumentInterpreter, PlainTextHelpRenderer,
};

/// Runs a fresh default [`CMDKit`] instance with pre-built commands.
///
/// This is a convenience wrapper that prints errors. Prefer
/// [`try_run_with_commands`] when callers should handle registration failures
/// (such as alias/name collisions) programmatically.
pub fn run_with_commands(commands: &[Command]) {
    core::CMDKit::run_with_commands(commands)
}

/// Runs a fresh default [`CMDKit`] instance with pre-built commands.
///
/// This is the preferred wrapper for library use because it returns
/// [`CMDKitError`] instead of hiding failure paths.
pub fn try_run_with_commands(commands: &[Command]) -> Result<(), CMDKitError> {
    core::CMDKit::try_run_with_commands(commands)
}

#[cfg(test)]
mod tests {
    use super::{CMDKitError, run_with_commands, try_run_with_commands};

    #[test]
    fn wrapper_try_run_with_commands_propagates_missing_command_error() {
        let result = try_run_with_commands(&[]);

        match result {
            Err(CMDKitError::MissingCommand { help }) => {
                assert!(help.contains("Usage:"));
            }
            _ => panic!("expected missing command error"),
        }
    }

    #[test]
    fn wrapper_run_with_commands_handles_errors_without_panicking() {
        run_with_commands(&[]);
    }
}
