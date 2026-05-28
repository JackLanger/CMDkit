use std::{
    error::Error,
    fmt,
    sync::{
        Arc, OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard,
        atomic::{AtomicU8, Ordering},
    },
};

use crate::{Command, StrategyError, cli::CommandRegistry};

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

pub struct CoreConfig {
    pub lock_poison_policy: LockPoisonPolicy,
}

impl CoreConfig {
    // creates a default config object
    pub fn new() -> Self {
        Self {
            lock_poison_policy: LockPoisonPolicy::FailFast,
        }
    }
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// # Default Help Strategy
///  Help strategy implementation for the CLI.
/// This strategy is registered by default and provides a comprehensive help message that lists all available functionalities and their descriptions.
/// When the `help` command is executed, it displays usage information and a list of all registered functionalities along with their descriptions.
/// This strategy is designed to be simple and informative, making it easy for users to understand how to use the CLI and what commands are available.
/// The help message is dynamically generated based on the currently registered functionalities, ensuring that it always reflects the latest state of the CLI.
pub struct DisplayHelp;

impl DisplayHelp {
    pub fn show(caller: &str, registered_commands: &Vec<Command>) -> String {
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
                .map(|e| format!("    - {}: {}", e.metadata.name, e.metadata.description))
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
    registry: OnceLock<Arc<RwLock<CommandRegistry>>>,
    config: CoreConfig,
}

impl Default for CliCore {
    fn default() -> Self {
        Self::new()
    }
}

impl CliCore {
    /// creates a CliCore instance form a [CoreConfig].
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

    /// Registers a command functionality into this runtime instance.
    pub fn register(&self, command: Command) -> &Self {
        let mut guard = self.write_registry();
        guard.register(command);
        self
    }

    /// Retrieves a registered functionality by command name.
    pub fn get(&self, name: &str) -> Option<Command> {
        let guard = self.read_registry();
        guard.get(name)
    }

    /// Returns all currently registered functionalities.
    pub fn get_all(&self) -> Vec<Command> {
        let guard = self.read_registry();
        guard.get_all()
    }

    /// Returns direct child commands for a registered command path.
    pub fn get_children(&self, name: &str) -> Option<Vec<Command>> {
        let guard = self.read_registry();
        guard.get_children(name)
    }

    /// Runs the CLI with pre-built functionalities and prints user-facing errors.
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
    /// This is useful for tests and embedding scenarios where argument sources
    /// are not read from process environment.
    pub fn try_run_from_args(&self, args: &[String]) -> Result<(), CliCoreError> {
        let binary = args.get(0).cloned().unwrap_or_else(|| "cli".to_string());

        // check if help was requested if yes display help and exit
        if args.iter().any(|arg| arg == "help") {
            println!("{}", DisplayHelp::show(&binary, &self.get_all()));
            return Ok(());
        }

        let command_tokens = args.get(1..).ok_or_else(|| CliCoreError::MissingCommand {
            help: DisplayHelp::show(&binary, &self.get_all()),
        })?;
        if command_tokens.is_empty() {
            return Err(CliCoreError::MissingCommand {
                help: DisplayHelp::show(&binary, &self.get_all()),
            });
        }

        let (strategy_string, command, command_args) = self
            .resolve_command_path(command_tokens)
            .ok_or_else(|| CliCoreError::UnknownCommand {
                command: command_tokens.join(" "),
                help: DisplayHelp::show(&binary, &self.get_all()),
            })?;

        command
            .strategy
            .execute(command_args)
            .map_err(|source| CliCoreError::StrategyExecution {
                command: strategy_string,
                source,
            })
    }

    fn resolve_command_path(&self, tokens: &[String]) -> Option<(String, Command, Vec<String>)> {
        // Match the longest registered command path first so nested commands can
        // coexist with parent commands (for example: "test" and "test all").
        for end in (1..=tokens.len()).rev() {
            let candidate = tokens[..end].join(" ");
            if let Some(command) = self.get(&candidate) {
                return Some((candidate, command, tokens[end..].to_vec()));
            }
        }
        None
    }

    fn try_run_from_env(&self) -> Result<(), CliCoreError> {
        let argv = std::env::args().collect::<Vec<String>>();
        self.try_run_from_args(&argv)
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
                panic!("cli-core registry lock poisoned during {operation} operation")
            }
            LockPoisonPolicy::Recover => poisoned.into_inner(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        panic,
        sync::{Arc, RwLock},
    };

    use super::{CliCore, LockPoisonPolicy};
    use crate::{cli::CommandRegistry, core::CoreConfig};

    #[test]
    fn lock_poison_policy_defaults_to_fail_fast() {
        let core = CliCore::new();
        assert_eq!(core.lock_poison_policy(), LockPoisonPolicy::FailFast);
    }

    #[test]
    fn fail_fast_policy_panics_on_poisoned_read_lock() {
        let core = CliCore::new();
        let lock = Arc::new(RwLock::new(CommandRegistry::new()));

        let lock_for_thread = Arc::clone(&lock);
        let _ = std::thread::spawn(move || {
            let _guard = lock_for_thread
                .write()
                .expect("write lock should be acquired");
            panic!("poison lock");
        })
        .join();

        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            let poisoned = match lock.read() {
                Ok(_) => panic!("lock should be poisoned"),
                Err(poisoned) => poisoned,
            };
            drop(core.handle_poison(poisoned, "read"));
        }));
        assert!(result.is_err());
    }

    #[test]
    fn recover_policy_returns_inner_guard_on_poisoned_read_lock() {
        let config = CoreConfig {
            lock_poison_policy: LockPoisonPolicy::Recover,
        };

        let core = CliCore::create(config);

        let lock = Arc::new(RwLock::new(CommandRegistry::new()));
        let lock_for_thread = Arc::clone(&lock);
        let _ = std::thread::spawn(move || {
            let _guard = lock_for_thread
                .write()
                .expect("write lock should be acquired");
            panic!("poison lock");
        })
        .join();

        let poisoned = match lock.read() {
            Ok(_) => panic!("lock should be poisoned"),
            Err(poisoned) => poisoned,
        };
        let _guard = core.handle_poison(poisoned, "read");
    }
}

/// Runs the default global [`CliCore`] instance with pre-built functionalities.
pub fn run_with_commands(commands: &[Command]) {
    CliCore::new().run_with_commands(commands)
}

/// Runs the default global [`CliCore`] instance with pre-built commands.
pub fn try_run_with_commands(commands: &[Command]) -> Result<(), CliCoreError> {
    CliCore::new().try_run_with_commands(commands)
}
