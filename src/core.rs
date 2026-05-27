use std::{
    error::Error,
    fmt,
    sync::{Arc, OnceLock, RwLock},
};

use crate::{Functionality, StrategyError, cli::FunctionalityRegistry};

/// # Default Help Strategy
///  Help strategy implementation for the CLI.
/// This strategy is registered by default and provides a comprehensive help message that lists all available functionalities and their descriptions.
/// When the `help` command is executed, it displays usage information and a list of all registered functionalities along with their descriptions.
/// This strategy is designed to be simple and informative, making it easy for users to understand how to use the CLI and what commands are available.
/// The help message is dynamically generated based on the currently registered functionalities, ensuring that it always reflects the latest state of the CLI.
pub struct DisplayHelp;

impl DisplayHelp {
    pub fn show(caller: &str, registered_commands: &Vec<Functionality>) -> String {
        format!(
            r#"Usage: {} <command> [args...]
    Registered commands are listed below.

    supported commands:
            - help : Display help information
    {}

        "#,
            caller,
            registered_commands
                .iter()
                .map(|e| format!("    - {}: {}", e.name, e.description))
                .collect::<Vec<String>>()
                .join("\n")
        )
    }
}

/// Error returned by CLI-Core during command routing and strategy execution.
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
    registry: OnceLock<Arc<RwLock<FunctionalityRegistry>>>,
}

impl Default for CliCore {
    fn default() -> Self {
        Self::new()
    }
}

impl CliCore {
    /// Creates a new CLI runtime with lazy registry initialization.
    pub fn new() -> Self {
        Self {
            registry: OnceLock::new(),
        }
    }

    /// Registers a command functionality into this runtime instance.
    pub fn register(&self, functionality: Functionality) -> &Self {
        let mut guard = match self.registry().write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.register(functionality);
        self
    }

    /// Retrieves a registered functionality by command name.
    pub fn get(&self, name: &str) -> Option<Functionality> {
        let guard = match self.registry().read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.get(name)
    }

    /// Returns all currently registered functionalities.
    pub fn get_all(&self) -> Vec<Functionality> {
        let guard = match self.registry().read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.get_all()
    }

    #[allow(dead_code)]
    fn register_function(&mut self, result: Result<Functionality, StrategyError>) -> &mut Self {
        if let Ok(functionality) = result {
            self.register(functionality);
        }
        // should we panic on initializer errors instead of silently skipping them?
        self
    }

    /// Runs the CLI with project-provided initializers and prints user-facing errors.
    pub fn run_with_initializers<F>(&self, initializers: &[F])
    where
        F: Fn() -> Functionality,
    {
        if let Err(e) = self.try_run_with_initializers(initializers) {
            eprintln!("{e}");
        }
    }

    /// Runs the CLI with project-provided initializers and recoverable errors.
    ///
    /// # Errors
    ///
    /// Returns [`CliCoreError::MissingCommand`] when argv does not contain a command,
    /// [`CliCoreError::UnknownCommand`] when the command is not registered, and
    /// [`CliCoreError::StrategyExecution`] when the selected strategy returns an error.
    pub fn try_run_with_initializers<F>(&self, initializers: &[F]) -> Result<(), CliCoreError>
    where
        F: Fn() -> Functionality,
    {
        for init in initializers {
            self.register(init());
        }
        self.try_run_from_env()
    }

    /// Runs command dispatch against an explicit argv slice.
    ///
    /// This is useful for tests and embedding scenarios where argument sources
    /// are not read from process environment.
    pub fn try_run_from_args(&self, args: &[String]) -> Result<(), CliCoreError> {
        let binary = args.get(0).cloned().unwrap_or_else(|| "cli".to_string());

        // check if help was requested if yes display help and exit
        if args.iter().any(|arg| arg == "help") {
            println!("{}", DisplayHelp::show(&binary, &self.get_all()));
            return Ok(());
        }

        let strategy_string = args
            .get(1)
            .cloned()
            .ok_or_else(|| CliCoreError::MissingCommand {
                help: DisplayHelp::show(&binary, &self.get_all()),
            })?;

        let functionality =
            self.get(&strategy_string)
                .ok_or_else(|| CliCoreError::UnknownCommand {
                    command: strategy_string.clone(),
                    help: DisplayHelp::show(&binary, &self.get_all()),
                })?;

        let command_args = args.get(2..).unwrap_or(&[]).to_vec();
        functionality
            .strategy
            .execute(command_args)
            .map_err(|source| CliCoreError::StrategyExecution {
                command: strategy_string,
                source,
            })
    }

    fn try_run_from_env(&self) -> Result<(), CliCoreError> {
        let argv = std::env::args().collect::<Vec<String>>();
        self.try_run_from_args(&argv)
    }

    fn registry(&self) -> &Arc<RwLock<FunctionalityRegistry>> {
        self.registry
            .get_or_init(|| Arc::new(RwLock::new(FunctionalityRegistry::new())))
    }
}

/// Runs the default global [`CliCore`] instance with project initializers.
///
/// This is a convenience wrapper kept for backward compatibility.
pub fn run_with_initializers(initializers: &[fn() -> Functionality]) {
    CliCore::new().run_with_initializers(initializers)
}

/// Runs the default global [`CliCore`] instance with recoverable errors.
///
/// This is a convenience wrapper kept for backward compatibility.
pub fn try_run_with_initializers(
    initializers: &[fn() -> Functionality],
) -> Result<(), CliCoreError> {
    CliCore::new().try_run_with_initializers(initializers)
}
