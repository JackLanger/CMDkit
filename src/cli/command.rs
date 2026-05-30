use std::{option::Option, sync::Arc};

use super::strategy::FallbackSubcommandStrategy;
use super::{
    CommandStrategy, FunctionStrategy, StrategyError, SubcommandCatalog, SubcommandRouter,
};

/// Declarative value-taking option metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Switch {
    /// Canonical option name, for example: "path".
    pub name: String,
    /// Human-readable description for help output.
    pub description: String,
    /// Alternative spellings accepted during parsing.
    pub aliases: Vec<String>,
}

impl Switch {
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
pub struct Argument {
    /// Canonical
    /// Argument name, for example: "verbose".
    pub name: String,
    /// Human-readable description for help output.
    pub description: String,
    /// Alternative spellings accepted during parsing.
    pub aliases: Vec<String>,
    /// Numeric payload that can be mapped to an enum or bit mask.
    pub value: Option<String>,
    /// Whether this argument is required or optional.
    pub required: bool,
}

impl Argument {
    /// Creates a
    /// Argument declaration with the given numeric payload.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            aliases: Vec::new(),
            value: None,
            required: false,
        }
    }

    /// Adds alias spellings for this
    /// Argument.
    pub fn with_aliases(mut self, aliases: Vec<impl Into<String>>) -> Self {
        self.aliases = aliases.into_iter().map(|s| s.into()).collect();
        self
    }

    /// Sets the value for this argument.
    pub fn set_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
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
    pub options: Vec<Switch>,
    /// Optional
    /// Argument/flag descriptions shown in help output.
    pub arguments: Vec<Argument>,
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
    pub fn with_options(mut self, options: Vec<Switch>) -> Self {
        self.options = options;
        self
    }

    /// Adds
    /// Argument/flag definitions for this command.
    pub fn with_arguments(mut self, arguments: Vec<Argument>) -> Self {
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

    /// Executes this command after parsing raw argv-style arguments into the strategy contract.
    pub fn execute(&self, args: Vec<String>) -> Result<(), StrategyError> {
        let invocation = parser::ArgumentParser::parse(args, &self.metadata, |token| {
            self.matches_subcommand(token)
        })?;
        self.strategy
            .execute(invocation.options, invocation.arguments, invocation.params)
    }

    /// Returns the optional subcommand catalog exposed by the underlying strategy.
    pub fn subcommand_catalog(&self) -> Option<&dyn SubcommandCatalog> {
        self.strategy.subcommand_catalog()
    }

    /// Creates a command specification directly from a function or closure handler.
    pub fn from_fn<F>(name: impl Into<String>, description: impl Into<String>, runner: F) -> Self
    where
        F: Fn(Vec<Switch>, Vec<Argument>, Vec<String>) -> Result<(), StrategyError>
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

    fn matches_subcommand(&self, token: &str) -> bool {
        self.subcommand_catalog().is_some_and(|catalog| {
            catalog.subcommands().into_iter().any(|command| {
                command.metadata.name == token
                    || command.metadata.aliases.iter().any(|alias| alias == token)
            })
        })
    }
}

struct ParsedInvocation {
    options: Vec<Switch>,
    arguments: Vec<Argument>,
    params: Vec<String>,
}

/// Creates a fluent command builder.
pub fn command(name: impl Into<String>, description: impl Into<String>) -> CommandBuilder {
    CommandBuilder::new(name, description)
}

/// creates a value-taking option declaration.
pub fn argument(name: impl Into<String>, description: impl Into<String>) -> Argument {
    Argument::new(name, description)
}

/// creates a value-taking option declaration with the required flag set to true.
pub fn switch(name: impl Into<String>, description: impl Into<String>) -> Switch {
    Switch::new(name, description)
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
        F: Fn(Vec<Switch>, Vec<Argument>, Vec<String>) -> Result<(), StrategyError>
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
    pub fn with_options(mut self, options: Vec<Switch>) -> Self {
        self.metadata = self.metadata.with_options(options);
        self
    }

    /// Adds
    /// Argument/flag description entries for this command.
    pub fn with_arguments(mut self, arguments: Vec<Argument>) -> Self {
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

mod parser {
    use super::{CommandMetaData, ParsedInvocation};
    use crate::StrategyError;

    pub(super) struct ArgumentParser;

    impl ArgumentParser {
        pub fn parse<F>(
            args: Vec<String>,
            metadata: &CommandMetaData,
            is_subcommand: F,
        ) -> Result<ParsedInvocation, StrategyError>
        where
            F: Fn(&str) -> bool,
        {
            let mut options = Vec::new();
            let mut arguments = Vec::new();
            let mut params = Vec::new();
            let mut index = 0;

            while index < args.len() {
                let token = &args[index];

                if is_subcommand(token) {
                    break;
                }

                let Some(flag) = token.strip_prefix("--") else {
                    params.push(token.clone());
                    index += 1;
                    continue;
                };

                if let Some(argument_decl) = metadata.arguments.iter().find(|argument| {
                    argument.name == flag || argument.aliases.iter().any(|alias| alias == flag)
                }) {
                    if let Some((_, value)) = flag.split_once('=') {
                        arguments.push(argument_decl.clone().set_value(value.to_string()));
                        index += 1;
                        continue;
                    }

                    let Some(next) = args.get(index + 1) else {
                        return Err(StrategyError::invalid_arguments(format!(
                            "missing value for argument '--{}'",
                            argument_decl.name
                        )));
                    };

                    if next.starts_with("--") || is_subcommand(next) {
                        return Err(StrategyError::invalid_arguments(format!(
                            "missing value for argument '--{}'",
                            argument_decl.name
                        )));
                    }

                    arguments.push(argument_decl.clone().set_value(next.clone()));
                    index += 2;
                    continue;
                }

                if let Some(option_decl) = metadata.options.iter().find(|option| {
                    option.name == flag || option.aliases.iter().any(|alias| alias == flag)
                }) {
                    if flag.contains('=') {
                        return Err(StrategyError::invalid_arguments(format!(
                            "switch '--{}' does not take a value",
                            option_decl.name
                        )));
                    }

                    options.push(option_decl.clone());
                    index += 1;
                    continue;
                }

                if let Some((key, value)) = flag.split_once('=') {
                    arguments.push(super::Argument::new(key, "").set_value(value.to_string()));
                    index += 1;
                    continue;
                }

                if let Some(next) = args.get(index + 1) {
                    if next.starts_with("--") || is_subcommand(next) {
                        options.push(super::Switch::new(flag, ""));
                        index += 1;
                    } else {
                        arguments.push(super::Argument::new(flag, "").set_value(next.clone()));
                        index += 2;
                    }
                } else {
                    options.push(super::Switch::new(flag, ""));
                    index += 1;
                }
            }

            params.extend_from_slice(&args[index..]);

            Ok(ParsedInvocation {
                options,
                arguments,
                params,
            })
        }
    }
}
