use std::collections::HashMap;

use cli_core::{core, Command, CommandStrategy, StrategyError};

struct ProbeStrategy;

impl CommandStrategy for ProbeStrategy {
    fn execute(
        &self,
        _options: Vec<String>,
        _arguments: HashMap<String, String>,
        _subcommands: Vec<String>,
    ) -> Result<(), StrategyError> {
        Ok(())
    }
}

fn main() {
    println!("--FIRST--");
    core::run_with_commands(&[Command::new("alpha", "alpha command", ProbeStrategy)]);
    println!("--SECOND--");
    core::run_with_commands(&[Command::new("beta", "beta command", ProbeStrategy)]);
}
