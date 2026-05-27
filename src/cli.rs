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
    /// Optional nested subcommands under this command node.
    pub children: Vec<Functionality>,
}

/// Internal registry storage for command-to-functionality mappings.
pub(crate) struct FunctionalityRegistry {
    functionalities: BTreeMap<String, Functionality>,
}

impl FunctionalityRegistry {
    pub(crate) fn new() -> Self {
        Self {
            functionalities: BTreeMap::new(),
        }
    }

    pub(crate) fn get(&self, name: &str) -> Option<Functionality> {
        self.functionalities.get(name).cloned()
    }

    pub(crate) fn register(&mut self, functionality: Functionality) -> &mut FunctionalityRegistry {
        let rooted = self.materialize_subtree(&functionality, None);
        self.insert_subtree(&rooted);
        self
    }

    pub(crate) fn get_all(&self) -> Vec<Functionality> {
        self.functionalities.values().cloned().collect()
    }

    pub(crate) fn get_children(&self, name: &str) -> Option<Vec<Functionality>> {
        self.get(name).map(|f| f.children)
    }

    fn materialize_subtree(
        &self,
        functionality: &Functionality,
        prefix: Option<&str>,
    ) -> Functionality {
        let full_name = match prefix {
            Some(parent) => format!("{parent} {}", functionality.name),
            None => functionality.name.clone(),
        };

        let children = functionality
            .children
            .iter()
            .map(|child| self.materialize_subtree(child, Some(&full_name)))
            .collect();

        Functionality {
            name: full_name,
            description: functionality.description.clone(),
            strategy: Arc::clone(&functionality.strategy),
            children,
        }
    }

    fn insert_subtree(&mut self, node: &Functionality) {
        self.functionalities.insert(node.name.clone(), node.clone());
        for child in &node.children {
            self.insert_subtree(child);
        }
    }
}
