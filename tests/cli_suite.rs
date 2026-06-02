use std::sync::atomic::{AtomicUsize, Ordering};

use cmdkit::{Argument, CMDKit, Command, CommandStrategy, StrategyError, Switch};

struct TestStrategy;
struct TestStrategyV2;

static NEXT_TEST_ID: AtomicUsize = AtomicUsize::new(0);

fn unique_name(prefix: &str) -> String {
    let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{id}")
}

impl CommandStrategy for TestStrategy {
    fn execute(
        &self,
        _options: Vec<Switch>,
        _arguments: Vec<Argument>,
        _params: Vec<String>,
    ) -> Result<(), StrategyError> {
        println!("Test strategy executed");
        Ok(())
    }
}

impl CommandStrategy for TestStrategyV2 {
    fn execute(
        &self,
        _options: Vec<Switch>,
        _arguments: Vec<Argument>,
        _params: Vec<String>,
    ) -> Result<(), StrategyError> {
        println!("Test strategy v2 executed");
        Ok(())
    }
}

#[test]
fn test_register_and_get() {
    let name = unique_name("register_get");
    let functionality = Command::new(name.clone(), "A test functionality", TestStrategy);

    let core = CMDKit::builder().register(functionality.clone()).build();
    let retrieved = core.get(&name).expect("Functionality should be registered");
    assert_eq!(retrieved.metadata.name, functionality.metadata.name);
    assert_eq!(
        retrieved.metadata.description,
        functionality.metadata.description
    );
}

#[test]
fn test_get_all() {
    let name = unique_name("get_all");
    let functionality = Command::new(name.clone(), "A test functionality", TestStrategy);

    let core = CMDKit::builder().register(functionality.clone()).build();
    let all = core.get_all();
    assert!(all.iter().any(|f| f.metadata.name == name));
}

#[test]
fn test_non_existent() {
    let core = CMDKit::builder().build();
    let result = core.get("nonexistent");
    assert!(
        result.is_none(),
        "Expected None for non-existent functionality"
    );
}

#[test]
fn test_register_duplicate_name_overwrites() {
    let name = unique_name("duplicate");
    let core = CMDKit::builder()
        .register(Command::new(name.clone(), "original", TestStrategy))
        .register(Command::new(name.clone(), "updated", TestStrategyV2))
        .build();

    let retrieved = core.get(&name).expect("Functionality should be registered");
    assert_eq!(retrieved.metadata.description, "updated");
}

#[test]
fn test_instances_are_isolated() {
    let core_b = CMDKit::builder().build();
    let name = unique_name("isolated");

    let core_a = CMDKit::builder()
        .register(Command::new(name.clone(), "in a", TestStrategy))
        .build();

    assert!(core_a.get(&name).is_some());
    assert!(core_b.get(&name).is_none());
}
