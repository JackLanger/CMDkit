use cmdkit::{core::InvocationArgs, Command, CommandStrategy, StrategyError, CMDKit};

struct ProbeStrategy;

impl CommandStrategy for ProbeStrategy {
    fn execute(&self, _invocation: InvocationArgs) -> Result<(), StrategyError> {
        Ok(())
    }
}

fn main() {
    println!("--FIRST--");
    CMDKit::run_with_commands(&[Command::new("alpha", "alpha command", ProbeStrategy)]);
    println!("--SECOND--");
    CMDKit::run_with_commands(&[Command::new("beta", "beta command", ProbeStrategy)]);
}
