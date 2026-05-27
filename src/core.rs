use std::{
    error::Error,
    fmt,
    sync::{Arc, OnceLock, RwLock},
};

use crate::{
    Functionality, StrategyError,
    cli::{FunctionalityRegistry, help::build_help_functionality},
};

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
    pub fn register(&self, functionality: Functionality) {
        let mut guard = match self.registry().write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.register(functionality);
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

    /// Runs the CLI with project-provided initializers and prints user-facing errors.
    pub fn run_with_initializers(&self, initializers: &[fn() -> Functionality]) {
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
    pub fn try_run_with_initializers(
        &self,
        initializers: &[fn() -> Functionality],
    ) -> Result<(), CliCoreError> {
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
        let help_str = self
            .get("help")
            .expect("Help strategy should be registered")
            .strategy
            .help();

        let strategy_string = args
            .get(1)
            .cloned()
            .ok_or_else(|| CliCoreError::MissingCommand {
                help: help_str.clone(),
            })?;

        let functionality =
            self.get(&strategy_string)
                .ok_or_else(|| CliCoreError::UnknownCommand {
                    command: strategy_string.clone(),
                    help: help_str,
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
        self.registry.get_or_init(|| {
            let registry = Arc::new(RwLock::new(FunctionalityRegistry::new()));
            let help_functionality = build_help_functionality(Arc::clone(&registry));
            {
                let mut guard = match registry.write() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                guard.register(help_functionality);
            }
            registry
        })
    }
}

fn global_core() -> &'static CliCore {
    static CORE: OnceLock<CliCore> = OnceLock::new();
    CORE.get_or_init(CliCore::new)
}

/// Runs the default global [`CliCore`] instance with project initializers.
///
/// This is a convenience wrapper kept for backward compatibility.
pub fn run_with_initializers(initializers: &[fn() -> Functionality]) {
    global_core().run_with_initializers(initializers)
}

/// Runs the default global [`CliCore`] instance with recoverable errors.
///
/// This is a convenience wrapper kept for backward compatibility.
pub fn try_run_with_initializers(
    initializers: &[fn() -> Functionality],
) -> Result<(), CliCoreError> {
    global_core().try_run_with_initializers(initializers)
}
