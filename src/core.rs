use std::{
    error::Error,
    fmt,
    sync::{
        Arc, OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard,
        atomic::{AtomicU8, Ordering},
    },
};

use crate::{Functionality, StrategyError, cli::FunctionalityRegistry};

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

impl LockPoisonPolicy {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Recover,
            _ => Self::FailFast,
        }
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
    lock_poison_policy: AtomicU8,
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
            lock_poison_policy: AtomicU8::new(LockPoisonPolicy::FailFast as u8),
        }
    }

    /// Sets how this runtime handles poisoned registry locks.
    pub fn set_lock_poison_policy(&self, policy: LockPoisonPolicy) -> &Self {
        self.lock_poison_policy
            .store(policy as u8, Ordering::SeqCst);
        self
    }

    /// Returns the current lock-poison handling policy for this runtime.
    pub fn lock_poison_policy(&self) -> LockPoisonPolicy {
        LockPoisonPolicy::from_u8(self.lock_poison_policy.load(Ordering::SeqCst))
    }

    /// Registers a command functionality into this runtime instance.
    pub fn register(&self, functionality: Functionality) -> &Self {
        let mut guard = self.write_registry();
        guard.register(functionality);
        self
    }

    /// Retrieves a registered functionality by command name.
    pub fn get(&self, name: &str) -> Option<Functionality> {
        let guard = self.read_registry();
        guard.get(name)
    }

    /// Returns all currently registered functionalities.
    pub fn get_all(&self) -> Vec<Functionality> {
        let guard = self.read_registry();
        guard.get_all()
    }

    /// Returns direct child commands for a registered command path.
    pub fn get_children(&self, name: &str) -> Option<Vec<Functionality>> {
        let guard = self.read_registry();
        guard.get_children(name)
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

        let command_tokens = args.get(1..).ok_or_else(|| CliCoreError::MissingCommand {
            help: DisplayHelp::show(&binary, &self.get_all()),
        })?;
        if command_tokens.is_empty() {
            return Err(CliCoreError::MissingCommand {
                help: DisplayHelp::show(&binary, &self.get_all()),
            });
        }

        let (strategy_string, functionality, command_args) = self
            .resolve_command_path(command_tokens)
            .ok_or_else(|| CliCoreError::UnknownCommand {
                command: command_tokens.join(" "),
                help: DisplayHelp::show(&binary, &self.get_all()),
            })?;

        functionality
            .strategy
            .execute(command_args)
            .map_err(|source| CliCoreError::StrategyExecution {
                command: strategy_string,
                source,
            })
    }

    fn resolve_command_path(
        &self,
        tokens: &[String],
    ) -> Option<(String, Functionality, Vec<String>)> {
        // Match the longest registered command path first so nested commands can
        // coexist with parent commands (for example: "test" and "test all").
        for end in (1..=tokens.len()).rev() {
            let candidate = tokens[..end].join(" ");
            if let Some(functionality) = self.get(&candidate) {
                return Some((candidate, functionality, tokens[end..].to_vec()));
            }
        }
        None
    }

    fn try_run_from_env(&self) -> Result<(), CliCoreError> {
        let argv = std::env::args().collect::<Vec<String>>();
        self.try_run_from_args(&argv)
    }

    fn registry(&self) -> &Arc<RwLock<FunctionalityRegistry>> {
        self.registry
            .get_or_init(|| Arc::new(RwLock::new(FunctionalityRegistry::new())))
    }

    fn read_registry(&self) -> RwLockReadGuard<'_, FunctionalityRegistry> {
        match self.registry().read() {
            Ok(guard) => guard,
            Err(poisoned) => self.handle_poison(poisoned, "read"),
        }
    }

    fn write_registry(&self) -> RwLockWriteGuard<'_, FunctionalityRegistry> {
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
    use crate::cli::FunctionalityRegistry;

    #[test]
    fn lock_poison_policy_defaults_to_fail_fast() {
        let core = CliCore::new();
        assert_eq!(core.lock_poison_policy(), LockPoisonPolicy::FailFast);
    }

    #[test]
    fn fail_fast_policy_panics_on_poisoned_read_lock() {
        let core = CliCore::new();
        let lock = Arc::new(RwLock::new(FunctionalityRegistry::new()));

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
        let core = CliCore::new();
        core.set_lock_poison_policy(LockPoisonPolicy::Recover);

        let lock = Arc::new(RwLock::new(FunctionalityRegistry::new()));
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
