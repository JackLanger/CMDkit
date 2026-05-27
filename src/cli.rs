pub(crate) mod help;
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock, RwLock},
};

pub trait CLIStrategy: Send + Sync {
    /// Executes the strategy with the given project name and language.
    fn execute(&self, args: Vec<String>);
    /// Determines if the strategy accepts the given command.
    fn accepts(&self, command: &str) -> bool;
    /// Provides help information for the strategy.
    fn help(&self) -> String;
}

#[derive(Clone)]
pub struct Functionality {
    pub name: String,
    pub description: String,
    pub strategy: Arc<dyn CLIStrategy>,
}

/// Global registry for CLI functionalities.
/// Uses one-time initialization and read/write locking for safe global registration.
static REGISTRY: OnceLock<RwLock<FunctionalityRegistry>> = OnceLock::new();

pub struct FunctionalityRegistry {
    functionalities: HashMap<String, Functionality>,
}

impl FunctionalityRegistry {
    pub fn new() -> Self {
        Self {
            functionalities: HashMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<Functionality> {
        self.functionalities.get(name).cloned()
    }

    pub fn register(&mut self, functionality: Functionality) -> &mut FunctionalityRegistry {
        self.functionalities
            .insert(functionality.name.clone(), functionality);
        self
    }

    pub fn get_all(&self) -> Vec<Functionality> {
        self.functionalities.values().cloned().collect()
    }
}

fn registry() -> &'static RwLock<FunctionalityRegistry> {
    REGISTRY.get_or_init(|| {
        let mut r = FunctionalityRegistry::new();
        r.register(Functionality {
            name: "help".to_string(),
            description: "Display help information".to_string(),
            strategy: Arc::new(help::DisplayHelp),
        });
        RwLock::new(r)
    })
}

pub fn register_global(functionality: Functionality) {
    let mut guard = registry()
        .write()
        .expect("[ERROR] Failed to acquire write lock for CLI registry");
    guard.register(functionality);
}

pub fn get_global(name: &str) -> Option<Functionality> {
    let guard = registry()
        .read()
        .expect("[ERROR] Failed to acquire read lock for CLI registry");
    guard.get(name)
}

pub fn get_all_global() -> Vec<Functionality> {
    let guard = registry()
        .read()
        .expect("[ERROR] Failed to acquire read lock for CLI registry");
    guard.get_all()
}
