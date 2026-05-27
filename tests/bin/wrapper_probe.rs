use std::sync::Arc;

use cli_core::{CLIStrategy, Functionality, StrategyError, core};

struct ProbeStrategy;

impl CLIStrategy for ProbeStrategy {
    fn execute(&self, _args: Vec<String>) -> Result<(), StrategyError> {
        Ok(())
    }
}

fn main() {
    println!("--FIRST--");
    core::run_with_functionalities(&[Functionality {
        name: "alpha".to_string(),
        description: "alpha command".to_string(),
        strategy: Arc::new(ProbeStrategy),
        children: Vec::new(),
    }]);
    println!("--SECOND--");
    core::run_with_functionalities(&[Functionality {
        name: "beta".to_string(),
        description: "beta command".to_string(),
        strategy: Arc::new(ProbeStrategy),
        children: Vec::new(),
    }]);
}
