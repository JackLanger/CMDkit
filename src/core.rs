use std::{
    error::Error,
    fmt,
    sync::{Arc, OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use crate::{Argument, Command, StrategyError, Switch, cli::CommandRegistry};

/// Controls how [`CliCore`] responds when the registry lock is poisoned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LockPoisonPolicy {
    /// Panic immediately when a poisoned lock is encountered.
    ///
    /// This is the default for CLI applications where lock poisoning indicates a
    /// serious bug and silent recovery would hide inconsistent state.
    FailFast = 0,
    /// Recover by taking the poisoned inner value.
    Recover = 1,
}

/// Renders user-facing help output from registered command metadata.
pub trait HelpRenderer: Send + Sync {
    fn render(&self, caller: &str, registered_commands: &[Command]) -> String;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvocationElement {
    Argument(Argument),
    Switch(Switch),
    Param(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvocationArgs {
    pub name: String,
    pub args: Vec<Argument>,
    pub switches: Vec<Switch>,
    pub params: Vec<String>,
    pub order: Vec<InvocationElement>,
    pub subcommand: Option<Box<InvocationArgs>>,
}

impl InvocationArgs {
    pub fn leaf_name(&self) -> &str {
        self.subcommand
            .as_deref()
            .map_or(self.name.as_str(), InvocationArgs::leaf_name)
    }
}

pub trait ArgumentInterpreter: Send + Sync {
    fn interpret(
        &self,
        arg: &[String],
        registered_commands: &[Command],
    ) -> Result<InvocationArgs, CliCoreError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PlainTextArgumentInterpreter;

impl PlainTextArgumentInterpreter {
    fn find_declared_argument<'a>(command: &'a Command, flag: &str) -> Option<&'a Argument> {
        command.metadata.arguments.iter().find(|argument| {
            argument.name == flag || argument.aliases.iter().any(|alias| alias == flag)
        })
    }

    fn find_declared_switch<'a>(command: &'a Command, flag: &str) -> Option<&'a Switch> {
        command.metadata.options.iter().find(|option| {
            option.name == flag || option.aliases.iter().any(|alias| alias == flag)
        })
    }

    fn resolve_command(commands: &[Command], token: &str) -> Option<Command> {
        commands.iter().find_map(|command| {
            ((command.metadata.name == token)
                || command.metadata.aliases.iter().any(|alias| alias == token))
            .then(|| command.clone())
        })
    }

    fn upsert_argument(arguments: &mut Vec<Argument>, argument: Argument) {
        if let Some(index) = arguments
            .iter()
            .position(|existing| existing.name == argument.name)
        {
            arguments.remove(index);
        }

        arguments.push(argument);
    }

    fn validate_required_arguments(
        command: &Command,
        arguments: &[Argument],
    ) -> Result<(), CliCoreError> {
        for required in command
            .metadata
            .arguments
            .iter()
            .filter(|argument| argument.required)
        {
            let value = arguments
                .iter()
                .find(|argument| argument.name == required.name)
                .and_then(|argument| argument.value.as_deref());

            if value.is_none_or(|value| value.trim().is_empty()) {
                return Err(CliCoreError::StrategyExecution {
                    command: command.metadata.name.clone(),
                    source: StrategyError::invalid_arguments(format!(
                        "missing value for required argument '--{}'",
                        required.name
                    )),
                });
            }
        }

        Ok(())
    }

    fn invalid_arguments(command: &Command, message: impl Into<String>) -> CliCoreError {
        CliCoreError::StrategyExecution {
            command: command.metadata.name.clone(),
            source: StrategyError::invalid_arguments(message),
        }
    }

    fn parse_command(
        &self,
        command: &Command,
        args: &[String],
    ) -> Result<InvocationArgs, CliCoreError> {
        let mut switches = Vec::new();
        let mut arguments = Vec::new();
        let mut params = Vec::new();
        let mut order = Vec::new();
        let mut index = 0;

        while index < args.len() {
            let token = &args[index];

            if params.is_empty() && let Some(subcommand) = command.resolve_subcommand(token) {
                Self::validate_required_arguments(command, &arguments)?;

                return Ok(InvocationArgs {
                    name: command.metadata.name.clone(),
                    args: arguments,
                    switches,
                    params,
                    order,
                    subcommand: Some(Box::new(self.parse_command(&subcommand, &args[index + 1..])?)),
                });
            }

            let Some(flag) = token.strip_prefix("--") else {
                params.push(token.clone());
                order.push(InvocationElement::Param(token.clone()));
                index += 1;
                continue;
            };

            if let Some((flag_name, inline_value)) = flag.split_once('=') {
                if let Some(argument_decl) = Self::find_declared_argument(command, flag_name) {
                    let argument = argument_decl.clone().set_value(inline_value.to_string());
                    Self::upsert_argument(&mut arguments, argument.clone());
                    order.push(InvocationElement::Argument(argument));
                    index += 1;
                    continue;
                }

                if let Some(option_decl) = Self::find_declared_switch(command, flag_name) {
                    return Err(Self::invalid_arguments(
                        command,
                        format!("switch '--{}' does not take a value", option_decl.name),
                    ));
                }

                return Err(Self::invalid_arguments(
                    command,
                    format!("unknown flag '--{}'", flag_name),
                ));
            }

            if let Some(argument_decl) = Self::find_declared_argument(command, flag) {
                let Some(next) = args.get(index + 1) else {
                    return Err(Self::invalid_arguments(
                        command,
                        format!("missing value for argument '--{}'", argument_decl.name),
                    ));
                };

                if next.starts_with("--") || command.resolve_subcommand(next).is_some() {
                    return Err(Self::invalid_arguments(
                        command,
                        format!("missing value for argument '--{}'", argument_decl.name),
                    ));
                }

                let argument = argument_decl.clone().set_value(next.clone());
                Self::upsert_argument(&mut arguments, argument.clone());
                order.push(InvocationElement::Argument(argument));
                index += 2;
                continue;
            }

            if let Some(option_decl) = Self::find_declared_switch(command, flag) {
                let switch = option_decl.clone();
                switches.push(switch.clone());
                order.push(InvocationElement::Switch(switch));
                index += 1;
                continue;
            }

            return Err(Self::invalid_arguments(
                command,
                format!("unknown flag '--{}'", flag),
            ));
        }

        Self::validate_required_arguments(command, &arguments)?;

        Ok(InvocationArgs {
            name: command.metadata.name.clone(),
            args: arguments,
            switches,
            params,
            order,
            subcommand: None,
        })
    }
}

impl ArgumentInterpreter for PlainTextArgumentInterpreter {
    fn interpret(
        &self,
        arg: &[String],
        registered_commands: &[Command],
    ) -> Result<InvocationArgs, CliCoreError> {
        let Some(command_name) = arg.first() else {
            return Err(CliCoreError::MissingCommand {
                help: String::new(),
            });
        };

        let command = Self::resolve_command(registered_commands, command_name).ok_or_else(|| {
            CliCoreError::UnknownCommand {
                command: command_name.clone(),
                help: String::new(),
            }
        })?;

        self.parse_command(&command, &arg[1..])
    }
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

pub struct CoreConfig {
    pub lock_poison_policy: LockPoisonPolicy,
    pub help_renderer: Arc<dyn HelpRenderer>,
    pub argument_interpreter: Arc<dyn ArgumentInterpreter>,
}

impl CoreConfig {
    // creates a default config object
    pub fn new() -> Self {
        Self {
            lock_poison_policy: LockPoisonPolicy::FailFast,
            help_renderer: Arc::new(PlainTextHelpRenderer),
            argument_interpreter: Arc::new(PlainTextArgumentInterpreter),
        }
    }

    /// Sets the lock-poison policy in a builder-friendly way.
    pub fn with_lock_poison_policy(mut self, policy: LockPoisonPolicy) -> Self {
        self.lock_poison_policy = policy;
        self
    }

    /// Replaces the help renderer in a builder-friendly way.
    pub fn with_help_renderer<R>(mut self, renderer: R) -> Self
    where
        R: HelpRenderer + 'static,
    {
        self.help_renderer = Arc::new(renderer);
        self
    }

    /// Replaces the argument interpreter in a builder-friendly way.
    pub fn with_argument_interpreter<I>(mut self, interpreter: I) -> Self
    where
        I: ArgumentInterpreter + 'static,
    {
        self.argument_interpreter = Arc::new(interpreter);
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
    registry: OnceLock<Arc<RwLock<CommandRegistry>>>,
    config: CoreConfig,
}

impl Default for CliCore {
    fn default() -> Self {
        Self::new()
    }
}

impl CliCore {
    /// Creates a [`CliCore`] instance from a [`CoreConfig`].
    pub fn create(config: CoreConfig) -> Self {
        Self {
            registry: OnceLock::new(),
            config,
        }
    }

    /// Creates a new CLI runtime with lazy registry initialization.
    pub fn new() -> Self {
        Self {
            registry: OnceLock::new(),
            config: CoreConfig::new(),
        }
    }

    /// Returns the current lock-poison handling policy for this runtime.
    pub fn lock_poison_policy(&self) -> LockPoisonPolicy {
        self.config.lock_poison_policy
    }

    /// Registers a command into this runtime instance.
    pub fn register(&self, command: Command) -> &Self {
        let mut guard = self.write_registry();
        guard.register(command);
        self
    }

    /// Retrieves a registered command by name.
    pub fn get(&self, name: &str) -> Option<Command> {
        let guard = self.read_registry();
        guard.get(name)
    }

    /// Returns all currently registered commands.
    pub fn get_all(&self) -> Vec<Command> {
        let guard = self.read_registry();
        guard.get_all()
    }

    /// Runs the CLI with pre-built commands and prints user-facing errors.
    pub fn run_with_commands(&self, commands: &[Command]) {
        if let Err(e) = self.try_run_with_commands(commands) {
            eprintln!("{e}");
        }
    }

    /// Runs the CLI with pre-built commands and recoverable errors.
    pub fn try_run_with_commands(&self, commands: &[Command]) -> Result<(), CliCoreError> {
        for command in commands {
            self.register(command.clone());
        }
        self.try_run_from_env()
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

        let registered_commands = self.get_all();
        let invocation = self
            .config
            .argument_interpreter
            .interpret(args.get(1..).unwrap_or(&[]), &registered_commands)
            .map_err(|error| self.attach_help(error, &binary))?;

        let command = self
            .resolve_registered_command(&registered_commands, &invocation.name)
            .ok_or_else(|| CliCoreError::UnknownCommand {
                command: invocation.name.clone(),
                help: self.render_help(&binary),
            })?;

        command
            .execute(&invocation)
            .map_err(|source| CliCoreError::StrategyExecution {
                command: invocation.leaf_name().to_string(),
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

    fn attach_help(&self, error: CliCoreError, caller: &str) -> CliCoreError {
        match error {
            CliCoreError::MissingCommand { .. } => CliCoreError::MissingCommand {
                help: self.render_help(caller),
            },
            CliCoreError::UnknownCommand { command, .. } => CliCoreError::UnknownCommand {
                command,
                help: self.render_help(caller),
            },
            other => other,
        }
    }

    fn resolve_registered_command(&self, commands: &[Command], name: &str) -> Option<Command> {
        commands.iter().find_map(|command| {
            ((command.metadata.name == name) || command.metadata.aliases.iter().any(|alias| alias == name))
                .then(|| command.clone())
        })
    }

    fn registry(&self) -> &Arc<RwLock<CommandRegistry>> {
        self.registry
            .get_or_init(|| Arc::new(RwLock::new(CommandRegistry::new())))
    }

    fn read_registry(&self) -> RwLockReadGuard<'_, CommandRegistry> {
        match self.registry().read() {
            Ok(guard) => guard,
            Err(poisoned) => self.handle_poison(poisoned, "read"),
        }
    }

    fn write_registry(&self) -> RwLockWriteGuard<'_, CommandRegistry> {
        match self.registry().write() {
            Ok(guard) => guard,
            Err(poisoned) => self.handle_poison(poisoned, "write"),
        }
    }

    fn handle_poison<T>(&self, poisoned: PoisonError<T>, operation: &str) -> T {
        match self.lock_poison_policy() {
            LockPoisonPolicy::FailFast => {
                panic!("CMDkit registry lock poisoned during {operation} operation")
            }
            LockPoisonPolicy::Recover => poisoned.into_inner(),
        }
    }
}

/// Runs a fresh default [`CliCore`] instance with pre-built commands.
pub fn run_with_commands(commands: &[Command]) {
    CliCore::new().run_with_commands(commands)
}

/// Runs a fresh default [`CliCore`] instance with pre-built commands.
pub fn try_run_with_commands(commands: &[Command]) -> Result<(), CliCoreError> {
    CliCore::new().try_run_with_commands(commands)
}

#[cfg(test)]
mod tests;
