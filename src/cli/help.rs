use std::sync::{Arc, RwLock};

use crate::cli::{CLIStrategy, Functionality, FunctionalityRegistry, StrategyError};

pub(crate) fn build_help_functionality(
    registry: Arc<RwLock<FunctionalityRegistry>>,
) -> Functionality {
    Functionality {
        name: "help".to_string(),
        description: "Display help information".to_string(),
        strategy: Arc::new(DisplayHelp { registry }),
    }
}

/// # Default Help Strategy
///  Help strategy implementation for the CLI.
/// This strategy is registered by default and provides a comprehensive help message that lists all available functionalities and their descriptions.
/// When the `help` command is executed, it displays usage information and a list of all registered functionalities along with their descriptions.
/// This strategy is designed to be simple and informative, making it easy for users to understand how to use the CLI and what commands are available.
/// The help message is dynamically generated based on the currently registered functionalities, ensuring that it always reflects the latest state of the CLI.
pub(crate) struct DisplayHelp {
    registry: Arc<RwLock<FunctionalityRegistry>>,
}

impl CLIStrategy for DisplayHelp {
    fn execute(&self, _args: Vec<String>) -> Result<(), StrategyError> {
        println!("{}", self.help());
        Ok(())
    }

    fn help(&self) -> String {
        let binary = std::env::args().next().unwrap_or_else(|| "app".to_string());
        let guard = match self.registry.read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        format!(
            r#"Usage: {} <command> [args...]
    Registered commands are listed below.

    supported commands:
    {}

        "#,
            binary,
            guard
                .get_all()
                .iter()
                .map(|e| format!("    - {}: {}", e.name, e.description))
                .collect::<Vec<String>>()
                .join("\n")
        )
    }
}
