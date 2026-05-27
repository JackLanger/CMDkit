use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use cli_core::{CLIStrategy, CliCore, Functionality, StrategyError};

struct TestStrategy;
struct TestStrategyV2;

static NEXT_TEST_ID: AtomicUsize = AtomicUsize::new(0);

fn unique_name(prefix: &str) -> String {
    let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{id}")
}

impl CLIStrategy for TestStrategy {
    fn execute(&self, _args: Vec<String>) -> Result<(), StrategyError> {
        println!("Test strategy executed");
        Ok(())
    }

    fn help(&self) -> String {
        "Test strategy help".to_string()
    }
}

impl CLIStrategy for TestStrategyV2 {
    fn execute(&self, _args: Vec<String>) -> Result<(), StrategyError> {
        println!("Test strategy v2 executed");
        Ok(())
    }

    fn help(&self) -> String {
        "Test strategy v2 help".to_string()
    }
}

#[test]
fn test_register_and_get() {
    let core = CliCore::new();
    let name = unique_name("register_get");
    let functionality = Functionality {
        name: name.clone(),
        description: "A test functionality".to_string(),
        strategy: Arc::new(TestStrategy),
    };

    core.register(functionality.clone());
    let retrieved = core.get(&name).expect("Functionality should be registered");
    assert_eq!(retrieved.name, functionality.name);
    assert_eq!(retrieved.description, functionality.description);
}

#[test]
fn test_get_all() {
    let core = CliCore::new();
    let name = unique_name("get_all");
    let functionality = Functionality {
        name: name.clone(),
        description: "A test functionality".to_string(),
        strategy: Arc::new(TestStrategy),
    };

    core.register(functionality.clone());
    let all = core.get_all();
    assert!(all.iter().any(|f| f.name == name));
}

#[test]
fn test_non_existent() {
    let core = CliCore::new();
    let result = core.get("nonexistent");
    assert!(
        result.is_none(),
        "Expected None for non-existent functionality"
    );
}

#[test]
fn test_help_strategy() {
    let core = CliCore::new();
    let help_strategy = core
        .get("help")
        .expect("Help strategy should be registered");
    assert_eq!(help_strategy.name, "help");
    assert_eq!(help_strategy.description, "Display help information");
    let help_output = help_strategy.strategy.help();
    assert!(help_output.contains("Usage:"));
    assert!(help_output.contains("supported commands:"));
    assert!(help_output.contains("- help: Display help information"));
}

#[test]
fn test_register_duplicate_name_overwrites() {
    let core = CliCore::new();
    let name = unique_name("duplicate");

    core.register(Functionality {
        name: name.clone(),
        description: "original".to_string(),
        strategy: Arc::new(TestStrategy),
    });

    core.register(Functionality {
        name: name.clone(),
        description: "updated".to_string(),
        strategy: Arc::new(TestStrategyV2),
    });

    let retrieved = core.get(&name).expect("Functionality should be registered");
    assert_eq!(retrieved.description, "updated");
    assert_eq!(retrieved.strategy.help(), "Test strategy v2 help");
}

#[test]
fn test_instances_are_isolated() {
    let core_a = CliCore::new();
    let core_b = CliCore::new();
    let name = unique_name("isolated");

    core_a.register(Functionality {
        name: name.clone(),
        description: "in a".to_string(),
        strategy: Arc::new(TestStrategy),
    });

    assert!(core_a.get(&name).is_some());
    assert!(core_b.get(&name).is_none());
}
