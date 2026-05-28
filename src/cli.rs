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

/// Optional help-time capability exposed by strategies that can route to child strategies.
pub trait SubcommandCatalog {
    /// Returns direct subcommands owned by this strategy.
    fn subcommands(&self) -> Vec<Command>;
}

/// Strategy contract for CLI command implementations.
pub trait CommandStrategy: Send + Sync {
    /// Executes the strategy with command-specific arguments.
    /// Strategy implementations should validate argument viability internally.
    fn execute(&self, args: Vec<String>) -> Result<(), StrategyError>;

    /// Optional catalog exposure used by help renderers to discover nested command trees.
    fn subcommand_catalog(&self) -> Option<&dyn SubcommandCatalog> {
        None
    }
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

/// Strategy helper implementing chain-of-responsibility style subcommand dispatch.
///
/// The router handles the first token of `args` as the subcommand name and forwards
/// trailing arguments recursively to the selected child strategy.
#[derive(Default)]
pub struct SubcommandRouter {
    children: BTreeMap<String, Command>,
    aliases: BTreeMap<String, String>,
}

impl SubcommandRouter {
    /// Creates an empty router.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a subcommand and returns `self` for chaining.
    pub fn register(mut self, command: Command) -> Self {
        self.register_mut(command);
        self
    }

    /// Registers a subcommand in-place.
    pub fn register_mut(&mut self, command: Command) -> &mut Self {
        for alias in &command.metadata.aliases {
            self.aliases
                .insert(alias.clone(), command.metadata.name.clone());
        }
        self.children.insert(command.metadata.name.clone(), command);
        self
    }

    fn resolve(&self, token: &str) -> Option<Command> {
        if let Some(command) = self.children.get(token) {
            return Some(command.clone());
        }

        self.aliases
            .get(token)
            .and_then(|canonical| self.children.get(canonical))
            .cloned()
    }

    fn available_subcommands(&self) -> String {
        self.children
            .keys()
            .cloned()
            .collect::<Vec<String>>()
            .join(", ")
    }
}

impl SubcommandCatalog for SubcommandRouter {
    fn subcommands(&self) -> Vec<Command> {
        self.children.values().cloned().collect()
    }
}

impl CommandStrategy for SubcommandRouter {
    fn execute(&self, args: Vec<String>) -> Result<(), StrategyError> {
        let Some(subcommand_name) = args.first() else {
            return Err(StrategyError::invalid_arguments(format!(
                "missing subcommand. available: {}",
                self.available_subcommands()
            )));
        };

        let command = self.resolve(subcommand_name).ok_or_else(|| {
            StrategyError::invalid_arguments(format!(
                "unknown subcommand '{subcommand_name}'. available: {}",
                self.available_subcommands()
            ))
        })?;

        command.execute(args[1..].to_vec())
    }

    fn subcommand_catalog(&self) -> Option<&dyn SubcommandCatalog> {
        Some(self)
    }
}

struct FallbackSubcommandStrategy {
    fallback: Arc<dyn CommandStrategy>,
    router: SubcommandRouter,
}

impl CommandStrategy for FallbackSubcommandStrategy {
    fn execute(&self, args: Vec<String>) -> Result<(), StrategyError> {
        if args.is_empty() {
            return self.fallback.execute(args);
        }
        self.router.execute(args)
    }

    fn subcommand_catalog(&self) -> Option<&dyn SubcommandCatalog> {
        Some(&self.router)
    }
}

/// User-facing metadata for a single CLI command.
#[derive(Clone)]
pub struct CommandMetaData {
    /// Command name used for lookup (for example: "help", "new", "build").
    pub name: String,
    /// Short description shown in generated help output.
    pub description: String,
    /// Optional explicit usage text for this command.
    pub usage: Option<String>,
    /// Optional detailed long-form help text.
    pub long_description: Option<String>,
    /// Optional command examples shown in help output.
    pub examples: Vec<String>,
    /// Optional option/flag descriptions shown in help output.
    pub options: Vec<String>,
    /// Optional aliases accepted by command discovery layers.
    pub aliases: Vec<String>,
}

impl CommandMetaData {
    /// Creates command metadata with required fields and sensible defaults.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            usage: None,
            long_description: None,
            examples: Vec::new(),
            options: Vec::new(),
            aliases: Vec::new(),
        }
    }

    /// Adds explicit usage text for help rendering.
    pub fn with_usage(mut self, usage: impl Into<String>) -> Self {
        self.usage = Some(usage.into());
        self
    }

    /// Adds detailed long-form description for help rendering.
    pub fn with_long_description(mut self, long_description: impl Into<String>) -> Self {
        self.long_description = Some(long_description.into());
        self
    }

    /// Adds example entries for this command.
    pub fn with_examples(mut self, examples: Vec<String>) -> Self {
        self.examples = examples;
        self
    }

    /// Adds option/flag description entries for this command.
    pub fn with_options(mut self, options: Vec<String>) -> Self {
        self.options = options;
        self
    }

    /// Adds alias entries for this command.
    pub fn with_aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases = aliases;
        self
    }
}

/// Metadata + handler pair for a single CLI command.
#[derive(Clone)]
pub struct Command {
    /// User-facing command metadata.
    pub metadata: CommandMetaData,
    /// Handler implementation executed when this command is selected.
    pub strategy: Arc<dyn CommandStrategy>,
}

impl Command {
    /// Creates a command specification from any handler implementation type.
    pub fn new<S>(name: impl Into<String>, description: impl Into<String>, strategy: S) -> Self
    where
        S: CommandStrategy + 'static,
    {
        Self {
            metadata: CommandMetaData::new(name, description),
            strategy: Arc::new(strategy),
        }
    }

    // execute this strategy with the provided arguments, returning any strategy error for downstream handling
    pub fn execute(&self, args: Vec<String>) -> Result<(), StrategyError> {
        self.strategy.execute(args)
    }

    /// Creates a command specification directly from a function or closure handler.
    pub fn from_fn<F>(name: impl Into<String>, description: impl Into<String>, runner: F) -> Self
    where
        F: Fn(Vec<String>) -> Result<(), StrategyError> + Send + Sync + 'static,
    {
        Self::new(name, description, FunctionStrategy::new(runner))
    }

    /// Creates a fluent command builder.
    pub fn builder(name: impl Into<String>, description: impl Into<String>) -> CommandBuilder {
        CommandBuilder::new(name, description)
    }
}

/// Creates a fluent command builder.
pub fn command(name: impl Into<String>, description: impl Into<String>) -> CommandBuilder {
    CommandBuilder::new(name, description)
}

/// Fluent command builder that hides strategy implementation details.
pub struct CommandBuilder {
    metadata: CommandMetaData,
    strategy: Option<Arc<dyn CommandStrategy>>,
    subcommands: Vec<Command>,
}

impl CommandBuilder {
    /// Creates a builder with required metadata.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            metadata: CommandMetaData::new(name, description),
            strategy: None,
            subcommands: Vec::new(),
        }
    }

    /// Sets command strategy implementation.
    pub fn handler<S>(mut self, strategy: S) -> Self
    where
        S: CommandStrategy + 'static,
    {
        self.strategy = Some(Arc::new(strategy));
        self
    }

    /// Sets command strategy using a function/closure.
    pub fn handler_fn<F>(mut self, runner: F) -> Self
    where
        F: Fn(Vec<String>) -> Result<(), StrategyError> + Send + Sync + 'static,
    {
        self.strategy = Some(Arc::new(FunctionStrategy::new(runner)));
        self
    }

    /// Adds a subcommand.
    pub fn subcommand<C>(mut self, subcommand: C) -> Self
    where
        C: Into<Command>,
    {
        self.subcommands.push(subcommand.into());
        self
    }

    /// Adds explicit usage text for help rendering.
    pub fn with_usage(mut self, usage: impl Into<String>) -> Self {
        self.metadata = self.metadata.with_usage(usage);
        self
    }

    /// Adds detailed long-form description for help rendering.
    pub fn with_long_description(mut self, long_description: impl Into<String>) -> Self {
        self.metadata = self.metadata.with_long_description(long_description);
        self
    }

    /// Adds example entries for this command.
    pub fn with_examples(mut self, examples: Vec<String>) -> Self {
        self.metadata = self.metadata.with_examples(examples);
        self
    }

    /// Adds option/flag description entries for this command.
    pub fn with_options(mut self, options: Vec<String>) -> Self {
        self.metadata = self.metadata.with_options(options);
        self
    }

    /// Adds alias entries for this command.
    pub fn with_aliases(mut self, aliases: Vec<String>) -> Self {
        self.metadata = self.metadata.with_aliases(aliases);
        self
    }

    /// Builds the final command.
    pub fn build(self) -> Command {
        let strategy: Arc<dyn CommandStrategy> = if self.subcommands.is_empty() {
            self.strategy.unwrap_or_else(|| {
                Arc::new(FunctionStrategy::new(|_| {
                    Err(StrategyError::internal(
                        "command has no handler; configure a handler or subcommand",
                    ))
                }))
            })
        } else {
            let mut router = SubcommandRouter::new();
            for subcommand in self.subcommands {
                router.register_mut(subcommand);
            }

            match self.strategy {
                Some(fallback) => Arc::new(FallbackSubcommandStrategy { fallback, router }),
                None => Arc::new(router),
            }
        };

        Command {
            metadata: self.metadata,
            strategy,
        }
    }
}

impl From<CommandBuilder> for Command {
    fn from(value: CommandBuilder) -> Self {
        value.build()
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
        self.commands.insert(command.metadata.name.clone(), command);
        self
    }

    pub(crate) fn get_all(&self) -> Vec<Command> {
        self.commands.values().cloned().collect()
    }
}
