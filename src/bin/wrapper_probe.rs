use cmdkit::{Argument, Command, CommandStrategy, StrategyError, Switch, core};

struct ProbeStrategy;

impl CommandStrategy for ProbeStrategy {
    fn execute(
        &self,
        _options: Vec<Switch>,
        _arguments: Vec<Argument>,
        _params: Vec<String>,
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
