use std::{collections::BTreeMap, sync::Arc};

use super::{Argument, Command, StrategyError, Switch};

/// Optional help-time capability exposed by strategies that can route to child strategies.
pub trait SubcommandCatalog {
    /// Returns direct subcommands owned by this strategy.
    fn subcommands(&self) -> Vec<Command>;
}

/// Strategy contract for CLI command implementations.
pub trait CommandStrategy: Send + Sync {
    /// Executes the strategy with parsed invocation data.
    /// Strategy implementations should validate argument viability internally.
    fn execute(
        &self,
        options: Vec<Switch>,
        arguments: Vec<Argument>,
        params: Vec<String>,
    ) -> Result<(), StrategyError>;

    /// Optional catalog exposure used by help renderers to discover nested command trees.
    fn subcommand_catalog(&self) -> Option<&dyn SubcommandCatalog> {
        None
    }
}

/// Adapter that turns a function or closure into a [`CommandStrategy`].
pub struct FunctionStrategy<F>
where
    F: Fn(Vec<Switch>, Vec<Argument>, Vec<String>) -> Result<(), StrategyError> + Send + Sync,
{
    runner: F,
}

impl<F> FunctionStrategy<F>
where
    F: Fn(Vec<Switch>, Vec<Argument>, Vec<String>) -> Result<(), StrategyError> + Send + Sync,
{
    pub fn new(runner: F) -> Self {
        Self { runner }
    }
}

impl<F> CommandStrategy for FunctionStrategy<F>
where
    F: Fn(Vec<Switch>, Vec<Argument>, Vec<String>) -> Result<(), StrategyError> + Send + Sync,
{
    fn execute(
        &self,
        options: Vec<Switch>,
        arguments: Vec<Argument>,
        params: Vec<String>,
    ) -> Result<(), StrategyError> {
        (self.runner)(options, arguments, params)
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
    fn execute(
        &self,
        _options: Vec<Switch>,
        _arguments: Vec<Argument>,
        params: Vec<String>,
    ) -> Result<(), StrategyError> {
        let Some(subcommand_name) = params.first() else {
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

        command.execute(params[1..].to_vec())
    }

    fn subcommand_catalog(&self) -> Option<&dyn SubcommandCatalog> {
        Some(self)
    }
}

pub(crate) struct FallbackSubcommandStrategy {
    strategy: Arc<dyn CommandStrategy>,
    router: SubcommandRouter,
}

impl FallbackSubcommandStrategy {
    pub(crate) fn new(strategy: Arc<dyn CommandStrategy>, router: SubcommandRouter) -> Self {
        Self { strategy, router }
    }
}

impl CommandStrategy for FallbackSubcommandStrategy {
    fn execute(
        &self,
        options: Vec<Switch>,
        arguments: Vec<Argument>,
        params: Vec<String>,
    ) -> Result<(), StrategyError> {
        if params.is_empty() {
            return self.strategy.execute(options, arguments, params);
        }
        self.router.execute(options, arguments, params)
    }

    fn subcommand_catalog(&self) -> Option<&dyn SubcommandCatalog> {
        Some(self)
    }
}

impl SubcommandCatalog for FallbackSubcommandStrategy {
    fn subcommands(&self) -> Vec<Command> {
        let mut subcommands = BTreeMap::new();

        for command in self.router.subcommands() {
            subcommands.insert(command.metadata.name.clone(), command);
        }

        if let Some(catalog) = self.strategy.subcommand_catalog() {
            for command in catalog.subcommands() {
                subcommands
                    .entry(command.metadata.name.clone())
                    .or_insert(command);
            }
        }

        subcommands.into_values().collect()
    }
}
