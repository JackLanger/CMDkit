use std::{collections::HashMap, option::Option, sync::Arc};

use super::strategy::FallbackSubcommandStrategy;
use super::{
    CommandStrategy, FunctionStrategy, StrategyError, SubcommandCatalog, SubcommandRouter,
};

/// Declarative value-taking option metadata.
#[derive(Clone)]
pub struct Opt {
    /// Canonical option name, for example: "path".
    pub name: String,
    /// Human-readable description for help output.
    pub description: String,
    /// Alternative spellings accepted during parsing.
    pub aliases: Vec<String>,
}

impl Opt {
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

/// Declarative flag metadata with a numeric payload for enum mapping.
#[derive(Clone)]
pub struct Switch {
    /// Canonical switch name, for example: "verbose".
    pub name: String,
    /// Human-readable description for help output.
    pub description: String,
    /// Alternative spellings accepted during parsing.
    pub aliases: Vec<String>,
    /// Numeric payload that can be mapped to an enum or bit mask.
    pub value: i32,
}

impl Switch {
    /// Creates a switch declaration with the given numeric payload.
    pub fn new(name: impl Into<String>, description: impl Into<String>, value: i32) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            aliases: Vec::new(),
            value,
        }
    }

    /// Adds alias spellings for this switch.
    pub fn with_aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases = aliases;
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
    pub options: Vec<Opt>,
    /// Optional switch/flag descriptions shown in help output.
    pub switches: Vec<Switch>,
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
            switches: Vec::new(),
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
    pub fn with_options(mut self, options: Vec<Opt>) -> Self {
        self.options = options;
        self
    }

    /// Adds switch/flag definitions for this command.
    pub fn with_switches(mut self, switches: Vec<Switch>) -> Self {
        self.switches = switches;
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
        let invocation = self.parse_invocation(args)?;
        self.strategy.execute(
            invocation.options,
            invocation.arguments,
            invocation.subcommands,
        )
    }

    /// Returns the optional subcommand catalog exposed by the underlying strategy.
    pub fn subcommand_catalog(&self) -> Option<&dyn SubcommandCatalog> {
        self.strategy.subcommand_catalog()
    }

    /// Creates a command specification directly from a function or closure handler.
    pub fn from_fn<F>(name: impl Into<String>, description: impl Into<String>, runner: F) -> Self
    where
        F: Fn(Vec<String>, HashMap<String, String>, Vec<String>) -> Result<(), StrategyError>
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

    fn parse_invocation(&self, args: Vec<String>) -> Result<ParsedInvocation, StrategyError> {
        let mut options = Vec::new();
        let mut arguments = HashMap::new();
        let mut index = 0;
        let has_declared_inputs =
            !self.metadata.options.is_empty() || !self.metadata.switches.is_empty();

        while index < args.len() {
            let token = &args[index];

            if self.matches_subcommand(token) {
                break;
            }

            let Option::Some(flag) = token.strip_prefix("--") else {
                if has_declared_inputs {
                    return Err(StrategyError::invalid_arguments(format!(
                        "unexpected argument '{token}'. positional arguments must use a flag before subcommands"
                    )));
                }

                return Err(StrategyError::invalid_arguments(format!(
                    "unexpected argument '{token}'. positional arguments must use a flag before subcommands"
                )));
            };

            if let Option::Some(name) = self.resolve_switch(flag) {
                options.push(name);
                index += 1;
                continue;
            }

            if let Option::Some(option) = self.resolve_option(flag) {
                if let Some((key, value)) = flag.split_once('=') {
                    arguments.insert(key.to_string(), value.to_string());
                    index += 1;
                    continue;
                }

                let Some(next) = args.get(index + 1) else {
                    return Err(StrategyError::invalid_arguments(format!(
                        "missing value for option '--{}'",
                        option.name
                    )));
                };

                if next.starts_with("--") || self.matches_subcommand(next) {
                    return Err(StrategyError::invalid_arguments(format!(
                        "missing value for option '--{}'",
                        option.name
                    )));
                }

                arguments.insert(option.name, next.clone());
                index += 2;
                continue;
            }

            if let Some((key, value)) = flag.split_once('=') {
                arguments.insert(key.to_string(), value.to_string());
                index += 1;
                continue;
            }

            if let Some(next) = args.get(index + 1) {
                if next.starts_with("--") || self.matches_subcommand(next) {
                    options.push(flag.to_string());
                    index += 1;
                } else {
                    arguments.insert(flag.to_string(), next.clone());
                    index += 2;
                }
            } else {
                options.push(flag.to_string());
                index += 1;
            }
        }

        Ok(ParsedInvocation {
            options,
            arguments,
            subcommands: args[index..].to_vec(),
        })
    }

    fn matches_subcommand(&self, token: &str) -> bool {
        self.subcommand_catalog().is_some_and(|catalog| {
            catalog.subcommands().into_iter().any(|command| {
                command.metadata.name == token
                    || command.metadata.aliases.iter().any(|alias| alias == token)
            })
        })
    }

    fn resolve_option(&self, token: &str) -> Option<Opt> {
        self.metadata
            .options
            .iter()
            .find(|option| {
                option.name == token || option.aliases.iter().any(|alias| alias == token)
            })
            .cloned()
    }

    fn resolve_switch(&self, token: &str) -> Option<String> {
        self.metadata
            .switches
            .iter()
            .find(|switch| {
                switch.name == token || switch.aliases.iter().any(|alias| alias == token)
            })
            .map(|switch| switch.name.clone())
    }
}

struct ParsedInvocation {
    options: Vec<String>,
    arguments: HashMap<String, String>,
    subcommands: Vec<String>,
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
        F: Fn(Vec<String>, HashMap<String, String>, Vec<String>) -> Result<(), StrategyError>
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
    pub fn with_options(mut self, options: Vec<Opt>) -> Self {
        self.metadata = self.metadata.with_options(options);
        self
    }

    /// Adds switch/flag description entries for this command.
    pub fn with_switches(mut self, switches: Vec<Switch>) -> Self {
        self.metadata = self.metadata.with_switches(switches);
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
