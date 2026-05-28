/// Public CLI primitives: strategy trait, command model, and strategy error types.
pub mod cli;

/// Instance-owned CLI runtime and routing error types.
pub mod core;

pub use cli::{
    Command, CommandBuilder, CommandMetaData, CommandStrategy, FunctionStrategy, StrategyError,
    StrategyErrorKind, SubcommandCatalog, SubcommandRouter, command,
};
pub use core::{
    CliCore, CliCoreError, CoreConfig, HelpRenderer, LockPoisonPolicy, PlainTextHelpRenderer,
};
pub use functionality_macro::cli;

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
