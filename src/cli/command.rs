use crate::core::InvocationArgs;
use std::fmt::Debug;
use std::{option::Option, sync::Arc};

use super::strategy::FallbackSubcommandStrategy;
use super::{
    CommandStrategy, FunctionStrategy, StrategyError, SubcommandCatalog, SubcommandRouter,
};

/// Declarative value-taking option metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchDefinition {
    /// Canonical option name, for example: "path".
    pub name: String,
    /// Human-readable description for help output.
    pub description: String,
    /// Alternative spellings accepted during parsing.
    pub aliases: Vec<String>,
}

impl SwitchDefinition {
    /// Creates a value-taking option declaration.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            aliases: Vec::new(),
        }
    }

    /// Adds alias spellings for this option.
    pub fn with_aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases = aliases;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArgumentDefinition {
    /// Canonical
    /// Argument name, for example: "verbose".
    pub name: String,
    /// Human-readable description for help output.
    pub description: String,
    /// Alternative spellings accepted during parsing.
    pub aliases: Vec<String>,
    /// Whether this argument is required or optional.
    pub required: bool,

    pub value_type: ValueType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueType {
    String,
    Bool,
    Float,
    Int,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ArgumentValue {
    String(String),
    Int(i64),
    Bool(bool),
    Float(f64),
}

impl Eq for ArgumentValue {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Argument {
    pub name: String,
    pub value: ArgumentValue,
}

impl ArgumentDefinition {
    /// Creates a
    /// Argument declaration with the given numeric payload.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        value_type: ValueType,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            aliases: Vec::new(),
            required: false,
            value_type,
        }
    }

    pub fn set_value(&self, value: ArgumentValue) -> Argument {
        Argument {
            name: self.name.clone(),
            value,
        }
    }

    /// Adds alias spellings for this
    /// Argument.
    pub fn with_aliases(mut self, aliases: Vec<impl Into<String>>) -> Self {
        self.aliases = aliases.into_iter().map(|s| s.into()).collect();
        self
    }

    /// Sets required to true, indicating this argument is required.
    pub fn set_required(mut self) -> Self {
        self.required = true;
        self
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
    pub options: Vec<SwitchDefinition>,
    /// Optional
    /// Argument/flag descriptions shown in help output.
    pub arguments: Vec<ArgumentDefinition>,
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
            arguments: Vec::new(),
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

    /// Adds value-taking option definitions for this command.
    pub fn with_options(mut self, options: Vec<SwitchDefinition>) -> Self {
        self.options = options;
        self
    }

    /// Adds
    /// Argument/flag definitions for this command.
    pub fn with_arguments(mut self, arguments: Vec<ArgumentDefinition>) -> Self {
        self.arguments = arguments;
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
    strategy: Arc<dyn CommandStrategy>,
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

    pub(crate) fn execute(&self, mut invocation: InvocationArgs) -> Result<(), StrategyError> {
        match invocation.subcommand.take() {
            Some(subcommand) => self
                .resolve_subcommand(&subcommand.name)
                .ok_or_else(|| {
                    StrategyError::invalid_arguments(format!(
                        "unknown subcommand '{}'",
                        subcommand.name
                    ))
                })?
                .execute(*subcommand),
            None => self.strategy.execute(invocation),
        }
    }

    /// Returns the optional subcommand catalog exposed by the underlying strategy.
    pub fn subcommand_catalog(&self) -> Option<&dyn SubcommandCatalog> {
        self.strategy.subcommand_catalog()
    }

    /// Creates a command specification directly from a function or closure handler.
    pub fn from_fn<F>(name: impl Into<String>, description: impl Into<String>, runner: F) -> Self
    where
        F: Fn(Vec<String>, Vec<Argument>, Vec<String>) -> Result<(), StrategyError>
            + Send
            + Sync
            + 'static,
    {
        Self::new(name, description, FunctionStrategy::new(runner))
    }

    /// Creates a fluent command builder.
    pub fn builder(name: impl Into<String>, description: impl Into<String>) -> CommandBuilder {
        CommandBuilder::new(name, description)
    }

    pub(crate) fn resolve_subcommand(&self, token: &str) -> Option<Command> {
        self.subcommand_catalog().and_then(|catalog| {
            catalog.subcommands().into_iter().find(|command| {
                command.metadata.name == token
                    || command.metadata.aliases.iter().any(|alias| alias == token)
            })
        })
    }
}

/// Creates a fluent command builder.
pub fn command(name: impl Into<String>, description: impl Into<String>) -> CommandBuilder {
    CommandBuilder::new(name, description)
}

/// creates a value-taking option declaration.
pub fn argument(name: impl Into<String>, description: impl Into<String>) -> ArgumentDefinition {
    ArgumentDefinition::new(name, description, ValueType::String)
}

pub fn argument_of_type(
    name: impl Into<String>,
    description: impl Into<String>,
    value_type: ValueType,
) -> ArgumentDefinition {
    ArgumentDefinition::new(name, description, value_type)
}

/// creates a value-taking option declaration with the required flag set to true.
pub fn switch(name: impl Into<String>, description: impl Into<String>) -> SwitchDefinition {
    SwitchDefinition::new(name, description)
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
        F: Fn(Vec<String>, Vec<Argument>, Vec<String>) -> Result<(), StrategyError>
            + Send
            + Sync
            + 'static,
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
    pub fn with_options(mut self, options: Vec<SwitchDefinition>) -> Self {
        self.metadata = self.metadata.with_options(options);
        self
    }

    /// Adds
    /// Argument/flag description entries for this command.
    pub fn with_arguments(mut self, arguments: Vec<ArgumentDefinition>) -> Self {
        self.metadata = self.metadata.with_arguments(arguments);
        self
    }

    /// Adds alias entries for this command.
    pub fn with_aliases(mut self, aliases: Vec<impl Into<String>>) -> Self {
        self.metadata = self
            .metadata
            .with_aliases(aliases.into_iter().map(|s| s.into()).collect());
        self
    }

    /// Builds the final command.
    pub fn build(self) -> Command {
        let strategy: Arc<dyn CommandStrategy> = if self.subcommands.is_empty() {
            self.strategy.unwrap_or_else(|| {
                Arc::new(FunctionStrategy::new(|_, _, _| {
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
                Some(fallback) => Arc::new(FallbackSubcommandStrategy::new(fallback, router)),
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
