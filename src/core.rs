use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex, mpsc},
    thread,
};

use futures_channel::oneshot;

use crate::{Argument, Command, StrategyError, Switch, cli::CommandRegistry};

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
    ) -> Result<InvocationArgs, CMDKitError>;
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
        command
            .metadata
            .options
            .iter()
            .find(|option| option.name == flag || option.aliases.iter().any(|alias| alias == flag))
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
    ) -> Result<(), CMDKitError> {
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
                return Err(CMDKitError::StrategyExecution {
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

    fn invalid_arguments(command: &Command, message: impl Into<String>) -> CMDKitError {
        CMDKitError::StrategyExecution {
            command: command.metadata.name.clone(),
            source: StrategyError::invalid_arguments(message),
        }
    }

    fn parse_command(
        &self,
        command: &Command,
        args: &[String],
    ) -> Result<InvocationArgs, CMDKitError> {
        let mut switches = Vec::new();
        let mut arguments = Vec::new();
        let mut params = Vec::new();
        let mut order = Vec::new();
        let mut index = 0;

        while index < args.len() {
            let token = &args[index];

            if params.is_empty()
                && let Some(subcommand) = command.resolve_subcommand(token)
            {
                Self::validate_required_arguments(command, &arguments)?;

                return Ok(InvocationArgs {
                    name: command.metadata.name.clone(),
                    args: arguments,
                    switches,
                    params,
                    order,
                    subcommand: Some(Box::new(
                        self.parse_command(&subcommand, &args[index + 1..])?,
                    )),
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
    ) -> Result<InvocationArgs, CMDKitError> {
        let Some(command_name) = arg.first() else {
            return Err(CMDKitError::MissingCommand {
                help: String::new(),
            });
        };

        let command =
            Self::resolve_command(registered_commands, command_name).ok_or_else(|| {
                CMDKitError::UnknownCommand {
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

#[derive(Clone)]
pub struct CoreConfig {
    pub help_renderer: Arc<dyn HelpRenderer>,
    pub argument_interpreter: Arc<dyn ArgumentInterpreter>,
}

impl CoreConfig {
    // creates a default config object
    pub fn new() -> Self {
        Self {
            help_renderer: Arc::new(PlainTextHelpRenderer),
            argument_interpreter: Arc::new(PlainTextArgumentInterpreter),
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
pub enum CMDKitError {
    /// Command registration failed due to invalid or conflicting metadata.
    Registration { message: String },
    /// Master executor cannot accept or complete jobs.
    ExecutorUnavailable { message: String },
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

impl fmt::Display for CMDKitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Registration { message } => {
                write!(f, "Command registration failed: {message}")
            }
            Self::ExecutorUnavailable { message } => {
                write!(f, "Executor unavailable: {message}")
            }
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

impl Error for CMDKitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::StrategyExecution { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Instance-owned CLI runtime.
///
/// Each [`CMDKit`] owns a lazily initialized command registry and can be reused
/// across multiple invocations without relying on process-global mutable state.
pub struct CMDKit {
    registry: CommandRegistry,
    config: CoreConfig,
}

impl CMDKit {
    /// Creates a [`CMDKit`] instance from a [`CoreConfig`].
    pub fn builder() -> CMDKitBuilder {
        CMDKitBuilder::new()
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
    ///
    /// This is a convenience wrapper that does not surface registration failures
    /// to the caller. Prefer [`CMDKit::try_run_with_commands`] when callers need
    /// to handle command-registration collisions programmatically.
    pub fn run_with_commands(commands: &[Command]) {
        if let Err(e) = Self::try_run_with_commands(commands) {
            eprintln!("{e}");
        }
    }

    /// Runs the CLI with pre-built commands and recoverable errors.
    ///
    /// This is the preferred entrypoint for embedding and library use because it
    /// returns structured registration errors (for example alias/name collisions)
    /// instead of panicking.
    pub fn try_run_with_commands(commands: &[Command]) -> Result<(), CMDKitError> {
        Self::builder()
            .try_with_commands(commands)?
            .build()
            .try_run_from_env()
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
    pub fn try_run_from_args(&self, args: &[String]) -> Result<(), CMDKitError> {
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
            .ok_or_else(|| CMDKitError::UnknownCommand {
                command: invocation.name.clone(),
                help: self.render_help(&binary),
            })?;

        command
            .execute(&invocation)
            .map_err(|source| CMDKitError::StrategyExecution {
                command: invocation.leaf_name().to_string(),
                source,
            })
    }

    fn render_help(&self, caller: &str) -> String {
        self.config.help_renderer.render(caller, &self.get_all())
    }

    fn try_run_from_env(&self) -> Result<(), CMDKitError> {
        let argv = std::env::args().collect::<Vec<String>>();
        self.try_run_from_args(&argv)
    }

    fn attach_help(&self, error: CMDKitError, caller: &str) -> CMDKitError {
        match error {
            CMDKitError::MissingCommand { .. } => CMDKitError::MissingCommand {
                help: self.render_help(caller),
            },
            CMDKitError::UnknownCommand { command, .. } => CMDKitError::UnknownCommand {
                command,
                help: self.render_help(caller),
            },
            other => other,
        }
    }

    fn resolve_registered_command(&self, commands: &[Command], name: &str) -> Option<Command> {
        commands.iter().find_map(|command| {
            ((command.metadata.name == name)
                || command.metadata.aliases.iter().any(|alias| alias == name))
            .then(|| command.clone())
        })
    }
}

pub struct CMDKitBuilder {
    config: CoreConfig,
    registry: CommandRegistry,
}

impl CMDKitBuilder {
    pub fn with_config(mut self, config: CoreConfig) -> Self {
        self.config = config;
        self
    }

    /// Registers a command into this runtime instance.
    ///
    /// Prefer [`CMDKitBuilder::try_register`] when command metadata can come from
    /// external or dynamic sources.
    ///
    /// # Panics
    /// Panics when registration fails (for example, alias/name collisions).
    pub fn register(self, command: Command) -> Self {
        self.try_register(command)
            .expect("command registration should succeed")
    }

    /// Registers a command and returns a structured error on failure.
    ///
    /// This is the preferred registration API because command registration is
    /// fallible: aliases and command names are validated for collisions.
    pub fn try_register(mut self, command: Command) -> Result<Self, CMDKitError> {
        self.registry
            .register(command)
            .map_err(|message| CMDKitError::Registration { message })?;
        Ok(self)
    }

    fn new() -> CMDKitBuilder {
        Self {
            config: Default::default(),
            registry: Default::default(),
        }
    }

    /// Registers multiple commands into this runtime instance.
    ///
    /// Prefer [`CMDKitBuilder::try_with_commands`] in library and embedding
    /// scenarios so registration failures can be handled by the caller.
    ///
    /// # Panics
    /// Panics when any command registration fails (for example, alias/name
    /// collisions).
    pub fn with_commands(self, commands: &[Command]) -> Self {
        self.try_with_commands(commands)
            .expect("bulk command registration should succeed")
    }

    /// Registers multiple commands and returns a structured error on failure.
    ///
    /// This is the preferred bulk-registration API because registration is
    /// fallible and should generally be handled by the caller.
    pub fn try_with_commands(mut self, commands: &[Command]) -> Result<Self, CMDKitError> {
        for cmd in commands {
            self.registry
                .register(cmd.clone())
                .map_err(|message| CMDKitError::Registration { message })?;
        }
        Ok(self)
    }

    /// Replaces the argument interpreter for all commands built by this builder.
    pub fn with_argument_interpreter<I>(mut self, interpreter: I) -> Self
    where
        I: ArgumentInterpreter + 'static,
    {
        self.config.argument_interpreter = Arc::new(interpreter);
        self
    }

    /// Finalizes this builder into a reusable [`CMDKit`] runtime.
    pub fn build(&self) -> CMDKit {
        CMDKit {
            registry: self.registry.clone(),
            config: self.config.clone(),
        }
    }

    /// Creates a [`CMDKitMaster`] that submits invocations to worker threads.
    ///
    /// The returned master exposes async completion handles for submitted jobs.
    /// `worker_count` values of `0` are normalized to `1` worker.
    pub fn as_master_executor(&self, config: CoreConfig, worker_count: usize) -> CMDKitMaster {
        CMDKitMaster::new(self.registry.clone(), config, worker_count)
    }
}

type WorkerResult = Result<(), CMDKitError>;

/// A future handle that resolves when a submitted invocation completes.
///
/// Awaiting this handle yields `Result<Result<(), CMDKitError>, Canceled>` where:
/// - outer `Err(Canceled)` means the executor dropped the completion channel.
/// - outer `Ok(inner)` carries the command execution result.
pub type ExecutionHandle = oneshot::Receiver<WorkerResult>;

struct QueuedInvocation {
    args: Vec<String>,
    completion_tx: oneshot::Sender<WorkerResult>,
}

/// Multi-worker command dispatcher that returns awaitable completion handles.
///
/// `CMDKitMaster` accepts invocations immediately and executes them on background
/// worker threads using the registry snapshot from the originating builder.
pub struct CMDKitMaster {
    submit_tx: mpsc::Sender<QueuedInvocation>,
    config: CoreConfig,
}

impl CMDKitMaster {
    fn new(registry: CommandRegistry, config: CoreConfig, worker_count: usize) -> Self {
        let (submit_tx, submit_rx) = mpsc::channel::<QueuedInvocation>();
        let shared_rx = Arc::new(Mutex::new(submit_rx));

        for _ in 0..worker_count.max(1) {
            let rx = Arc::clone(&shared_rx);
            let worker_registry = registry.clone();
            let worker_config = config.clone();

            thread::spawn(move || {
                let cmdkit = CMDKit {
                    registry: worker_registry,
                    config: worker_config,
                };

                loop {
                    let next_job = {
                        let guard = rx
                            .lock()
                            .expect("executor queue mutex should not be poisoned");
                        guard.recv()
                    };

                    let Ok(job) = next_job else {
                        break;
                    };

                    let result = cmdkit.try_run_from_args(&job.args);
                    let _ = job.completion_tx.send(result);
                }
            });
        }

        Self { submit_tx, config }
    }

    fn resolved_handle(result: WorkerResult) -> ExecutionHandle {
        let (tx, rx) = oneshot::channel();
        let _ = tx.send(result);
        rx
    }

    fn dispatch(&self, args: &[String]) -> Result<ExecutionHandle, CMDKitError> {
        let (completion_tx, completion_rx) = oneshot::channel();
        self.submit_tx
            .send(QueuedInvocation {
                args: args.to_vec(),
                completion_tx,
            })
            .map_err(|_| CMDKitError::ExecutorUnavailable {
                message: "worker queue is closed".to_string(),
            })?;

        Ok(completion_rx)
    }

    /// Submits an invocation for worker execution and returns its completion handle.
    ///
    /// This method is non-blocking with respect to command execution. Callers can
    /// await the returned [`ExecutionHandle`] to observe completion.
    pub fn try_run_from_args(&self, args: &[String]) -> Result<ExecutionHandle, CMDKitError> {
        let binary = args
            .iter()
            .next()
            .cloned()
            .unwrap_or_else(|| "cli".to_string());

        if args.get(1).is_some_and(|arg| arg == "help") {
            println!("{}", self.config.help_renderer.render(&binary, &[]));
            return Ok(Self::resolved_handle(Ok(())));
        }

        self.dispatch(args)
    }

    /// Submits process argv for worker execution and returns its completion handle.
    pub fn try_run_from_env(&self) -> Result<ExecutionHandle, CMDKitError> {
        let argv = std::env::args().collect::<Vec<String>>();
        self.try_run_from_args(&argv)
    }
}

#[cfg(test)]
mod tests;
