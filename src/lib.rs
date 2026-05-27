/// Public CLI primitives: strategy trait, functionality model, and strategy error types.
pub mod cli;

/// Instance-owned CLI runtime and routing error types.
pub mod core;

pub use cli::{CLIStrategy, Functionality, StrategyError, StrategyErrorKind};
pub use core::{CliCore, CliCoreError};
pub use functionality_macro::functionality;

/// Runs the default global [`CliCore`] instance with project initializers.
///
/// This root-level convenience wrapper is kept for backward compatibility.
pub fn run_with_initializers(initializers: &[fn() -> Functionality]) {
    core::run_with_initializers(initializers)
}

/// Runs the default global [`CliCore`] instance with recoverable errors.
///
/// This root-level convenience wrapper is kept for backward compatibility.
pub fn try_run_with_initializers(
    initializers: &[fn() -> Functionality],
) -> Result<(), CliCoreError> {
    core::try_run_with_initializers(initializers)
}

pub fn init() -> CliCore {
    CliCore::new()
}
