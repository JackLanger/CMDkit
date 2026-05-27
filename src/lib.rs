pub mod cli;

pub use cli::{CLIStrategy, Functionality, get_all_global, get_global, register_global};

/// # Run the CLI application with the provided initializers.
/// This function initializes the global registry with the provided initializers and then runs the CLI logic.
/// ## Arguments
/// * `initializers` - A slice of functions that register functionalities to the global registry.
/// ## Example
/// ```Rust
/// fn main() {
///   cli_core::run_with_initializers(&[project::init::register_all]);
/// }
/// ```
/// This example shows how to run the CLI application with a project module that registers its functionalities.
/// The `register_all` function in the project module would be responsible for registering all project-related functionalities to the global registry.
/// After running this function, the CLI application will be ready to accept commands and execute the corresponding strategies based on the registered functionalities.
///
/// Note: The `run_with_initializers` function should be called in the main function of the application to start the CLI application with the necessary initializations.
pub fn run_with_initializers(initializers: &[fn()]) {
    for init in initializers {
        init();
    }
    run_from_env();
}

fn run_from_env() {
    let help = get_global("help").expect("Help strategy should be registered");
    let help_str = help.strategy.help();

    let strategy_string = std::env::args().nth(1).expect(help_str.as_str());
    let functionality = get_global(&strategy_string).expect(help_str.as_str());

    let strategy = &functionality.strategy;
    if strategy.accepts(&std::env::args().nth(2).unwrap_or_default()) {
        strategy.execute(std::env::args().collect::<Vec<String>>());
    } else {
        eprintln!("Unknown command: {}", strategy_string);
        println!("{}", strategy.help());
    }
}
