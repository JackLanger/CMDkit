use cmdkit::{CMDKit, Command, CommandStrategy, StrategyError, core::InvocationArgs};

struct ProbeStrategy;

impl CommandStrategy for ProbeStrategy {
    fn execute(
        &self,
        _context: &cmdkit::ExecutionContext,
        _invocation: InvocationArgs,
    ) -> Result<(), StrategyError> {
        Ok(())
    }
}

fn main() {
    println!("--FIRST--");
    CMDKit::run_with_commands(&[Command::new("alpha", "alpha command", ProbeStrategy)]);
    println!("--SECOND--");
    CMDKit::run_with_commands(&[Command::new("beta", "beta command", ProbeStrategy)]);
}
