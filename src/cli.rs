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

/// Structured error returned by [`CommandStrategy::execute`].
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
pub trait CommandStrategy: Send + Sync {
    /// Executes the strategy with command-specific arguments.
    /// Strategy implementations should validate argument viability internally.
    fn execute(&self, args: Vec<String>) -> Result<(), StrategyError>;
}

/// Adapter that turns a function or closure into a [`CommandStrategy`].
pub struct FunctionStrategy<F>
where
    F: Fn(Vec<String>) -> Result<(), StrategyError> + Send + Sync,
{
    runner: F,
}

impl<F> FunctionStrategy<F>
where
    F: Fn(Vec<String>) -> Result<(), StrategyError> + Send + Sync,
{
    pub fn new(runner: F) -> Self {
        Self { runner }
    }
}

impl<F> CommandStrategy for FunctionStrategy<F>
where
    F: Fn(Vec<String>) -> Result<(), StrategyError> + Send + Sync,
{
    fn execute(&self, args: Vec<String>) -> Result<(), StrategyError> {
        (self.runner)(args)
    }
}

/// User-facing metadata for a single CLI command.
#[derive(Clone)]
pub struct CommandMetaData {
    /// Command name used for lookup (for example: "help", "new", "build").
    pub name: String,
    /// Short description shown in generated help output.
    pub description: String,
}

/// Metadata + handler pair for a single CLI command.
#[derive(Clone)]
pub struct Command {
    /// User-facing command metadata.
    pub metadata: CommandMetaData,
    /// Handler implementation executed when this command is selected.
    pub strategy: Arc<dyn CommandStrategy>,
    /// Optional nested subcommands under this command.
    pub children: Vec<Command>,
}

impl Command {
    /// Creates a command specification from any handler implementation type.
    pub fn new<S>(name: impl Into<String>, description: impl Into<String>, strategy: S) -> Self
    where
        S: CommandStrategy + 'static,
    {
        Self {
            metadata: CommandMetaData {
                name: name.into(),
                description: description.into(),
            },
            strategy: Arc::new(strategy),
            children: Vec::new(),
        }
    }

    /// Creates a command specification directly from a function or closure handler.
    pub fn from_fn<F>(name: impl Into<String>, description: impl Into<String>, runner: F) -> Self
    where
        F: Fn(Vec<String>) -> Result<(), StrategyError> + Send + Sync + 'static,
    {
        Self::new(name, description, FunctionStrategy::new(runner))
    }

    /// Appends a single child command.
    pub fn add_child(mut self, child: Command) -> Self {
        self.children.push(child);
        self
    }

    /// Attaches nested child commands.
    pub fn with_children(mut self, children: Vec<Command>) -> Self {
        self.children = children;
        self
    }
}

/// Internal registry storage for command-to-command mappings.
pub(crate) struct CommandRegistry {
    commands: BTreeMap<String, Command>,
}

impl CommandRegistry {
    pub(crate) fn new() -> Self {
        Self {
            commands: BTreeMap::new(),
        }
    }

    pub(crate) fn get(&self, name: &str) -> Option<Command> {
        self.commands.get(name).cloned()
    }

    pub(crate) fn register(&mut self, command: Command) -> &mut CommandRegistry {
        let rooted = self.materialize_subtree(&command, None);
        self.insert_subtree(&rooted);
        self
    }

    pub(crate) fn get_all(&self) -> Vec<Command> {
        self.commands.values().cloned().collect()
    }

    pub(crate) fn get_children(&self, name: &str) -> Option<Vec<Command>> {
        self.get(name).map(|f| f.children)
    }

    fn materialize_subtree(&self, command: &Command, prefix: Option<&str>) -> Command {
        let full_name = match prefix {
            Some(parent) => format!("{parent} {}", command.metadata.name),
            None => command.metadata.name.clone(),
        };

        let children = command
            .children
            .iter()
            .map(|child| self.materialize_subtree(child, Some(&full_name)))
            .collect();

        Command {
            metadata: CommandMetaData {
                name: full_name,
                description: command.metadata.description.clone(),
            },
            strategy: Arc::clone(&command.strategy),
            children,
        }
    }

    fn insert_subtree(&mut self, node: &Command) {
        self.commands
            .insert(node.metadata.name.clone(), node.clone());
        for child in &node.children {
            self.insert_subtree(child);
        }
    }
}
