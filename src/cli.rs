pub(crate) mod help;
use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};

/// Categorizes strategy failures for downstream error handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyErrorKind {
    /// Input arguments are syntactically valid for CLI routing but invalid for this strategy.
    InvalidArguments,
    /// Strategy business logic failed during normal execution.
    Execution,
    /// Unexpected internal failure in strategy implementation.
    Internal,
}

impl fmt::Display for StrategyErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::InvalidArguments => "invalid-arguments",
            Self::Execution => "execution",
            Self::Internal => "internal",
        };
        write!(f, "{label}")
    }
}

/// Structured error returned by [`CLIStrategy::execute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyError {
    /// Machine-friendly error category.
    pub kind: StrategyErrorKind,
    /// Human-readable explanation.
    pub message: String,
}

impl StrategyError {
    /// Creates a new strategy error from a kind and message.
    pub fn new(kind: StrategyErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Convenience constructor for argument validation failures.
    pub fn invalid_arguments(message: impl Into<String>) -> Self {
        Self::new(StrategyErrorKind::InvalidArguments, message)
    }

    /// Convenience constructor for runtime execution failures.
    pub fn execution(message: impl Into<String>) -> Self {
        Self::new(StrategyErrorKind::Execution, message)
    }

    /// Convenience constructor for internal or unexpected failures.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StrategyErrorKind::Internal, message)
    }
}

impl fmt::Display for StrategyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl Error for StrategyError {}

/// Strategy contract for CLI command implementations.
pub trait CLIStrategy: Send + Sync {
    /// Executes the strategy with command-specific arguments.
    /// Strategy implementations should validate argument viability internally.
    fn execute(&self, args: Vec<String>) -> Result<(), StrategyError>;
    /// Provides help information for the strategy.
    fn help(&self) -> String;
}

/// Metadata + behavior pair for a single command.
#[derive(Clone)]
pub struct Functionality {
    /// Command name used for lookup (for example: "help", "new", "build").
    pub name: String,
    /// Short description shown in generated help output.
    pub description: String,
    /// Strategy implementation executed when this functionality is selected.
    pub strategy: Arc<dyn CLIStrategy>,
}

/// Registry storage for command-to-functionality mappings.
///
/// This type is exposed for introspection and advanced integrations, while normal
/// crate usage should go through high-level entry points.
pub struct FunctionalityRegistry {
    functionalities: BTreeMap<String, Functionality>,
}

impl FunctionalityRegistry {
    /// Creates an empty functionality registry.
    pub fn new() -> Self {
        Self {
            functionalities: BTreeMap::new(),
        }
    }

    pub(crate) fn get(&self, name: &str) -> Option<Functionality> {
        self.functionalities.get(name).cloned()
    }

    pub(crate) fn register(&mut self, functionality: Functionality) -> &mut FunctionalityRegistry {
        self.functionalities
            .insert(functionality.name.clone(), functionality);
        self
    }

    pub(crate) fn get_all(&self) -> Vec<Functionality> {
        self.functionalities.values().cloned().collect()
    }
}
