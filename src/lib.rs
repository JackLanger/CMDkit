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
    CliCore, CliCoreError, CoreConfig, HelpRenderer,  PlainTextHelpRenderer,
};

/// Runs a fresh default [`CliCore`] instance with pre-built commands.
pub fn run_with_commands(commands: &[Command]) {
    core::CliCore::run_with_commands(commands)
}

/// Runs a fresh default [`CliCore`] instance with pre-built commands.
pub fn try_run_with_commands(commands: &[Command]) -> Result<(), CliCoreError> {
    core::CliCore::try_run_with_commands(commands)
}

