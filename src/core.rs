use std::{
    error::Error,
    fmt,
    sync::{Arc},
};

use crate::{Command, StrategyError, cli::CommandRegistry};



/// Renders user-facing help output from registered command metadata.
pub trait HelpRenderer: Send + Sync {
    fn render(&self, caller: &str, registered_commands: &[Command]) -> String;
}

/// Default plain-text help renderer.
pub struct PlainTextHelpRenderer;

impl HelpRenderer for PlainTextHelpRenderer {
    fn render(&self, caller: &str, registered_commands: &[Command]) -> String {
        fn render_recursive(command: &Command, depth: usize, path: String, out: &mut Vec<String>) {
            let indent = "  ".repeat(depth);
            out.push(format!(
                "    {indent}- {path}: {}",
                command.metadata.description
            ));

            if let Some(usage) = &command.metadata.usage {
                out.push(format!("    {indent}  usage: {usage}"));
            }

            if let Some(long_description) = &command.metadata.long_description {
                out.push(format!("    {indent}  details: {long_description}"));
            }

            if !command.metadata.aliases.is_empty() {
                out.push(format!(
                    "    {indent}  aliases: {}",
                    command.metadata.aliases.join(", ")
                ));
            }

            if !command.metadata.examples.is_empty() {
                out.push(format!("    {indent}  examples:"));
                for example in &command.metadata.examples {
                    out.push(format!("    {indent}    - {example}"));
                }
            }

            if !command.metadata.options.is_empty() {
                out.push(format!("    {indent}  switches:"));
                for option in &command.metadata.options {
                    if option.aliases.is_empty() {
                        out.push(format!(
                            "    {indent}    - --{}: {}",
                            option.name, option.description
                        ));
                    } else {
                        out.push(format!(
                            "    {indent}    - --{} (aliases: {}): {}",
                            option.name,
                            option.aliases.join(", "),
                            option.description
                        ));
                    }
                }
            }

            if !command.metadata.arguments.is_empty() {
                out.push(format!("    {indent}  arguments:"));
                for argument in &command.metadata.arguments {
                    let required_suffix = if argument.required { " [required]" } else { "" };
                    if argument.aliases.is_empty() {
                        out.push(format!(
                            "    {indent}    - --{}{}: {}",
                            argument.name, required_suffix, argument.description
                        ));
                    } else {
                        out.push(format!(
                            "    {indent}    - --{}{} (aliases: {}): {}",
                            argument.name,
                            required_suffix,
                            argument.aliases.join(", "),
                            argument.description
                        ));
                    }
                }
            }

            if let Some(catalog) = command.subcommand_catalog() {
                for child in catalog.subcommands() {
                    let child_path = format!("{path} {}", child.metadata.name);
                    render_recursive(&child, depth + 1, child_path, out);
                }
            }
        }

        let mut lines = Vec::new();
        for command in registered_commands {
            render_recursive(command, 0, command.metadata.name.clone(), &mut lines);
        }
        let command_lines = lines.join("\n");

        format!(
            r#"Usage: {} <command> [args...]
    Registered commands are listed below.

    supported commands:
            - help : Display help information
    {}

        "#,
            caller, command_lines
        )
    }
}

#[derive(Clone)]
pub struct CoreConfig {
    pub help_renderer: Arc<dyn HelpRenderer>,
}

impl CoreConfig {
    // creates a default config object
    pub fn new() -> Self {
        Self {
            help_renderer: Arc::new(PlainTextHelpRenderer),
        }
    }

    /// Replaces the help renderer in a builder-friendly way.
    pub fn with_help_renderer<R>(mut self, renderer: R) -> Self
    where
        R: HelpRenderer + 'static,
    {
        self.help_renderer = Arc::new(renderer);
        self
    }
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Error returned by CMDkit during command routing and strategy execution.
#[derive(Debug)]
pub enum CliCoreError {
    /// No command name was provided in argv.
    MissingCommand { help: String },
    /// The command name does not exist in the registry.
    UnknownCommand { command: String, help: String },
    /// A registered strategy failed while executing.
    StrategyExecution {
        /// The command selected by the user.
        command: String,
        /// Original error emitted by the strategy implementation.
        source: StrategyError,
    },
}

impl fmt::Display for CliCoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommand { help } => {
                write!(f, "No command provided.\n\n{help}")
            }
            Self::UnknownCommand { command, help } => {
                write!(f, "Unknown command: {command}\n\n{help}")
            }
            Self::StrategyExecution { command, source } => {
                write!(f, "Strategy execution failed for '{command}': {source}")
            }
        }
    }
}

impl Error for CliCoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::StrategyExecution { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Instance-owned CLI runtime.
///
/// Each [`CliCore`] owns a lazily initialized command registry and can be reused
/// across multiple invocations without relying on process-global mutable state.
pub struct CliCore {
    registry: CommandRegistry,
    config: CoreConfig,
}


impl CliCore {
    /// Creates a [`CliCore`] instance from a [`CoreConfig`].
    pub fn builder() -> CliCoreBuilder {
        CliCoreBuilder::new()
    }


    /// Retrieves a registered command by name.
    pub fn get(&self, name: &str) -> Option<Command> {
        self.registry.get(name)
    }

    /// Returns all currently registered commands.
    pub fn get_all(&self) -> Vec<Command> {
        self.registry.get_all()
    }

    /// Runs the CLI with pre-built commands and prints user-facing errors.
    pub fn run_with_commands(commands: &[Command]) {
        if let Err(e) = Self::try_run_with_commands(commands) {
            eprintln!("{e}");
        }
    }

    /// Runs the CLI with pre-built commands and recoverable errors.
    pub fn try_run_with_commands(commands: &[Command]) -> Result<(), CliCoreError> {

        Self::builder().with_commands(commands).build().try_run_from_env()
    }

    /// Runs command dispatch against an explicit argv slice.
    ///
    /// Token semantics:
    /// - `args[1]` is always treated as the top-level command selector.
    /// - `args[2..]` is forwarded to the selected command for command-level parsing.
    /// - Subcommand boundaries are resolved by the selected command strategy layer.
    ///
    /// This is useful for tests and embedding scenarios where argument sources
    /// are not read from process environment.
    pub fn try_run_from_args(&self, args: &[String]) -> Result<(), CliCoreError> {
        let binary = args
            .iter()
            .next()
            .cloned()
            .unwrap_or_else(|| "cli".to_string());

        if args.get(1).is_some_and(|arg| arg == "help") {
            println!("{}", self.render_help(&binary));
            return Ok(());
        }

        if args.len() < 2 {
            return Err(CliCoreError::MissingCommand {
                help: self.render_help(&binary),
            });
        }

        let command_name = args[1].clone();
        let command_args = args.get(2..).unwrap_or(&[]).to_vec();

        let command = self
            .get(&command_name)
            .ok_or_else(|| CliCoreError::UnknownCommand {
                command: command_name.clone(),
                help: self.render_help(&binary),
            })?;

        command
            .execute(command_args)
            .map_err(|source| CliCoreError::StrategyExecution {
                command: command_name,
                source,
            })
    }

    fn render_help(&self, caller: &str) -> String {
        self.config.help_renderer.render(caller, &self.get_all())
    }

    fn try_run_from_env(&self) -> Result<(), CliCoreError> {
        let argv = std::env::args().collect::<Vec<String>>();
        self.try_run_from_args(&argv)
    }

}


pub struct CliCoreBuilder {
    config: CoreConfig,
    registry: CommandRegistry,
}


impl CliCoreBuilder {
    pub fn with_config(mut self ,config: CoreConfig) -> Self {
        self.config = config;
        self
    }

    /// Registers a command into this runtime instance.
    pub fn register(mut self, command: Command) -> Self {

        self.registry.register(command);
        self
    }

    fn new() -> CliCoreBuilder {
        Self {
            config : Default::default(),
            registry : Default::default(),
        }
    }
    pub fn with_commands(mut self, commands: &[Command]) -> Self {
        for cmd in commands {
            self.registry.register(cmd.clone());
        }
        self
    }

    pub fn build(&self) -> CliCore {
        CliCore {
            registry: self.registry.clone(),
            config: self.config.clone(),
        }
    }
}


#[cfg(test)]
mod tests;
