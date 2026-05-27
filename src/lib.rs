/// Public CLI primitives: strategy trait, functionality model, and strategy error types.
pub mod cli;

/// Instance-owned CLI runtime and routing error types.
pub mod core;

pub use cli::{CLIStrategy, FunctionStrategy, Functionality, StrategyError, StrategyErrorKind};
pub use core::{CliCore, CliCoreError, LockPoisonPolicy};
pub use functionality_macro::cli;

/// Runs the default global [`CliCore`] instance with pre-built functionalities.
pub fn run_with_functionalities(functionalities: &[Functionality]) {
    core::run_with_functionalities(functionalities)
}

/// Runs the default global [`CliCore`] instance with pre-built functionalities.
pub fn try_run_with_functionalities(functionalities: &[Functionality]) -> Result<(), CliCoreError> {
    core::try_run_with_functionalities(functionalities)
}

pub fn init() -> CliCore {
    CliCore::new()
}
