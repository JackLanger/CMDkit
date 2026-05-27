# CLI-Core

CLI-Core is a small Rust library for building command-line applications with an instance-owned command registry, a built-in help command, and a cleaner way to connect parsed arguments to feature implementations.

## Why this crate is required

Most CLI applications start with an argument parser and a set of registered commands. That works for simple tools, but it often leaves the actual command handling tangled inside `main`, or spread across ad hoc parsing branches that are hard to reuse.

CLI-Core solves that by moving the command contract into a shared core:

- commands are registered once
- each command is backed by a feature implementation
- `main` only initializes the registry and dispatches the incoming arguments
- help text is generated from the registered features instead of being maintained separately

This gives you a more portable structure than a parser-only approach. The command layer is expressed as reusable functionality, so the same feature implementation can be wired into a binary without coupling the CLI entry point to the business logic.

It provides:

- an instance-owned `core::CliCore` runtime with lazy registry initialization
- a `CLIStrategy` trait for defining command behavior
- typed strategy errors via `StrategyError` and `StrategyErrorKind`
- a built-in `help` strategy
- `run_with_initializers` convenience wrappers for the default global instance

Without it, each binary would need to reimplement command registration, lookup, dispatch, and help output on its own, which usually leads to tightly coupled CLI code.

## How to use it

### 1. Implement a command strategy

```rust
use cli_core::{CLIStrategy, StrategyError};

struct NewProject;

impl CLIStrategy for NewProject {
 fn execute(&self, args: Vec<String>) -> Result<(), StrategyError> {
  let project_name = args
   .first()
   .cloned()
   .ok_or_else(|| StrategyError::invalid_arguments("missing <project_name>"))?;

  println!("Creating project: {project_name}");
  Ok(())
 }

 fn help(&self) -> String {
  "new <project_name> - create a new project".to_string()
 }
}
```

### 2. Register your commands with initializers

```rust
use std::sync::Arc;
use cli_core::Functionality;

fn register_new() -> Functionality {
 Functionality {
  name: "new".to_string(),
  description: "Create a new project".to_string(),
  strategy: Arc::new(NewProject),
 }
}
```

### 3. Run the CLI from `main`

Preferred: explicit instance ownership.

```rust
fn main() {
 let core = cli_core::core::CliCore::new();
 core.run_with_initializers(&[register_new]);
}
```

Compatibility wrapper (uses a default global instance):

```rust
fn main() {
 cli_core::run_with_initializers(&[register_new]);
}
```

### 4. Optional: run against explicit args (embedding/tests)

```rust
fn run_embedded(args: Vec<String>) -> Result<(), cli_core::core::CliCoreError> {
 let core = cli_core::core::CliCore::new();
 core.register(register_new());
 core.try_run_from_args(&args)
}
```

## Example usage

```bash
cargo run -- help
cargo run -- new my_project
```

The first argument selects the command. The built-in `help` command lists the registered functionalities and their descriptions.

## Command flow

1. `CliCore` lazily initializes a registry and registers built-in `help`.
2. The registry always contains the default `help` strategy.
3. The CLI reads the first argument as the command name.
4. The strategy receives only trailing command arguments (`argv[2..]`).
5. If strategy validation or execution fails, the error bubbles up from the strategy.

## Relation to argument parsers

Argument parsers usually answer the question, "How do I turn argv into structured input?" CLI-Core focuses on the next step: "Which feature implementation should handle this command, and how do I keep that mapping portable and reusable?"

In practice, that means you can still use argument parsing concepts at the edges, but the core library owns command registration, command lookup, and strategy execution.

## Error model

- Entry routing errors are returned as `CliCoreError`.
- Strategy-level failures are returned as `StrategyError` with kinds: `InvalidArguments`, `Execution`, `Internal`.
- `CliCoreError::StrategyExecution` preserves the original `StrategyError` as source.

## Notes

- Commands are discovered from the registry at runtime.
- The help output is generated from the current `CliCore` registry, so it stays in sync with registered commands.
- This crate is intended to be used as a shared core for binaries that need consistent command dispatch.
