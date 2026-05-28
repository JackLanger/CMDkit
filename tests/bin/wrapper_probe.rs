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
        metadata: CommandMetaData::new("alpha", "alpha command"),
        strategy: Arc::new(ProbeStrategy),
    }]);
    println!("--SECOND--");
    core::run_with_commands(&[Command {
        metadata: CommandMetaData::new("beta", "beta command"),
        strategy: Arc::new(ProbeStrategy),
    }]);
}
