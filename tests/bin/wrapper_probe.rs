use std::sync::Arc;

use cli_core::{Command, CommandMetaData, CommandStrategy, StrategyError, core};

struct ProbeStrategy;

impl CommandStrategy for ProbeStrategy {
    fn execute(&self, _args: Vec<String>) -> Result<(), StrategyError> {
        Ok(())
    }
}

fn main() {
    println!("--FIRST--");
    core::run_with_commands(&[Command {
        metadata: CommandMetaData {
            name: "alpha".to_string(),
            description: "alpha command".to_string(),
        },
        strategy: Arc::new(ProbeStrategy),
        children: Vec::new(),
    }]);
    println!("--SECOND--");
    core::run_with_commands(&[Command {
        metadata: CommandMetaData {
            name: "beta".to_string(),
            description: "beta command".to_string(),
        },
        strategy: Arc::new(ProbeStrategy),
        children: Vec::new(),
    }]);
}
