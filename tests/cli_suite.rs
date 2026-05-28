use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use cli_core::{CliCore, Command, CommandMetaData, CommandStrategy, StrategyError};

struct TestStrategy;
struct TestStrategyV2;

static NEXT_TEST_ID: AtomicUsize = AtomicUsize::new(0);

fn unique_name(prefix: &str) -> String {
    let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{id}")
}

impl CommandStrategy for TestStrategy {
    fn execute(&self, _args: Vec<String>) -> Result<(), StrategyError> {
        println!("Test strategy executed");
        Ok(())
    }
}

impl CommandStrategy for TestStrategyV2 {
    fn execute(&self, _args: Vec<String>) -> Result<(), StrategyError> {
        println!("Test strategy v2 executed");
        Ok(())
    }
}

#[test]
fn test_register_and_get() {
    let core = CliCore::new();
    let name = unique_name("register_get");
    let functionality = Command {
        metadata: CommandMetaData::new(name.clone(), "A test functionality"),
        strategy: Arc::new(TestStrategy),
    };

    core.register(functionality.clone());
    let retrieved = core.get(&name).expect("Functionality should be registered");
    assert_eq!(retrieved.metadata.name, functionality.metadata.name);
    assert_eq!(
        retrieved.metadata.description,
        functionality.metadata.description
    );
}

#[test]
fn test_get_all() {
    let core = CliCore::new();
    let name = unique_name("get_all");
    let functionality = Command {
        metadata: CommandMetaData::new(name.clone(), "A test functionality"),
        strategy: Arc::new(TestStrategy),
    };

    core.register(functionality.clone());
    let all = core.get_all();
    assert!(all.iter().any(|f| f.metadata.name == name));
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
fn test_register_duplicate_name_overwrites() {
    let core = CliCore::new();
    let name = unique_name("duplicate");

    core.register(Command {
        metadata: CommandMetaData::new(name.clone(), "original"),
        strategy: Arc::new(TestStrategy),
    });

    core.register(Command {
        metadata: CommandMetaData::new(name.clone(), "updated"),
        strategy: Arc::new(TestStrategyV2),
    });

    let retrieved = core.get(&name).expect("Functionality should be registered");
    assert_eq!(retrieved.metadata.description, "updated");
}

#[test]
fn test_instances_are_isolated() {
    let core_a = CliCore::new();
    let core_b = CliCore::new();
    let name = unique_name("isolated");

    core_a.register(Command {
        metadata: CommandMetaData::new(name.clone(), "in a"),
        strategy: Arc::new(TestStrategy),
    });

    assert!(core_a.get(&name).is_some());
    assert!(core_b.get(&name).is_none());
}
