/// Public CLI primitives: strategy trait, functionality model, and strategy error types.
pub mod cli;

/// Instance-owned CLI runtime and routing error types.
pub mod core;

pub use cli::{
    Command, CommandMetaData, CommandStrategy, FunctionStrategy, StrategyError, StrategyErrorKind,
};
pub use core::{CliCore, CliCoreError, LockPoisonPolicy};
pub use functionality_macro::cli;

/// Runs the default global [`CliCore`] instance with pre-built commands.
pub fn run_with_commands(commands: &[Command]) {
    core::run_with_commands(commands)
}

/// Runs the default global [`CliCore`] instance with pre-built commands.
pub fn try_run_with_commands(commands: &[Command]) -> Result<(), CliCoreError> {
    core::try_run_with_commands(commands)
}

pub fn init() -> CliCore {
    CliCore::new()
}
