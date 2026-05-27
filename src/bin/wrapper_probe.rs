use std::sync::Arc;

use cli_core::{CLIStrategy, Functionality, StrategyError, core};

struct ProbeStrategy;

impl CLIStrategy for ProbeStrategy {
    fn execute(&self, _args: Vec<String>) -> Result<(), StrategyError> {
        Ok(())
    }
}

fn register_alpha() -> Functionality {
    Functionality {
        name: "alpha".to_string(),
        description: "alpha command".to_string(),
        strategy: Arc::new(ProbeStrategy),
        children: Vec::new(),
    }
}

fn register_beta() -> Functionality {
    Functionality {
        name: "beta".to_string(),
        description: "beta command".to_string(),
        strategy: Arc::new(ProbeStrategy),
        children: Vec::new(),
    }
}

fn main() {
    println!("--FIRST--");
    core::run_with_initializers(&[register_alpha]);
    println!("--SECOND--");
    core::run_with_initializers(&[register_beta]);
}
