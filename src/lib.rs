/// Public CLI primitives for implementation-first command trees, parsed strategy dispatch, and strategy error types.
pub mod cli;

/// Instance-owned CLI runtime, routing error types, and help rendering.
pub mod core;

pub use cli::{
    Command, CommandBuilder, CommandMetaData, CommandStrategy, FunctionStrategy, StrategyError,
    StrategyErrorKind, SubcommandCatalog, SubcommandRouter, Switch, command,
};
pub use core::{
    CliCore, CliCoreError, CoreConfig, HelpRenderer, LockPoisonPolicy, PlainTextHelpRenderer,
};

/// Runs a fresh default [`CliCore`] instance with pre-built commands.
pub fn run_with_commands(commands: &[Command]) {
    core::run_with_commands(commands)
}

/// Runs a fresh default [`CliCore`] instance with pre-built commands.
pub fn try_run_with_commands(commands: &[Command]) -> Result<(), CliCoreError> {
    core::try_run_with_commands(commands)
}

pub fn init() -> CliCore {
    CliCore::new()
}
