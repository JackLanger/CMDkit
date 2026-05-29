# CLI-Core

CLI-Core is a Rust library for implementation-first command dispatch with an instance-owned runtime.

The architecture is intentionally small and portable:

- commands are registered as a tree of implementations
- `Command` owns command metadata and an internal strategy handle
- `CommandStrategy` owns behavior for the selected command only
- routing nodes forward subcommands; leaf strategies consume the parsed invocation
- help output is generated from command metadata through a pluggable `HelpRenderer`

## Core concepts

### Command model

`Command` is the unit of registration. It contains:

- `metadata: CommandMetaData`
- an internal strategy handle

`CommandMetaData` includes required and optional help-facing fields:

- required: `name`, `description`
- optional: `usage`, `long_description`, `examples`, `options`, `aliases`

Use `CommandMetaData::new(...)` and builder-style metadata methods such as `with_usage(...)`, `with_examples(...)`, and `with_options(...)`.

### Strategy model

`CommandStrategy` defines one method:

```rust
fn execute(
    &self,
    options: Vec<String>,
    arguments: HashMap<String, String>,
    subcommands: Vec<String>,
) -> Result<(), StrategyError>
```

`CliCore` resolves `argv[1]` as the command name and parses the remaining tokens into:

- `options`: bare flags such as `--verbose`
- `arguments`: flag/value pairs such as `--path ./tmp`
- `subcommands`: the remaining command chain for nested routing

Only the final selected command strategy receives the parsed invocation.
Intermediate routing commands can ignore flags and options.

### Help rendering

Help is rendered from registered command metadata via `HelpRenderer`.

Default behavior uses `PlainTextHelpRenderer`, configured in `CoreConfig::new()`.
You can inject a custom renderer with `CoreConfig::with_help_renderer(...)`.

## Quick start

### 1. Define a strategy

```rust
use std::collections::HashMap;

use cli_core::{CommandStrategy, StrategyError};

struct NewProject;

impl CommandStrategy for NewProject {
    fn execute(
        &self,
        options: Vec<String>,
        arguments: HashMap<String, String>,
        subcommands: Vec<String>,
    ) -> Result<(), StrategyError> {
        let project_name = arguments
            .get("name")
            .cloned()
            .ok_or_else(|| StrategyError::invalid_arguments("missing --name <project_name>"))?;

        println!("creating project: {project_name}");
        println!("options: {options:?}");
        println!("subcommands: {subcommands:?}");
        Ok(())
    }
}
```

### 2. Register commands

```rust
use cli_core::{CliCore, Command};

let core = CliCore::new();
core.register(
    Command::new("new", "Create a new project", NewProject)
        .with_usage("new --name <project_name>"),
);
```

For nested commands, build a tree and let the runtime route to the leaf:

```rust
use std::collections::HashMap;

use cli_core::{command, Command, CommandStrategy, StrategyError};

struct RunTask;

impl CommandStrategy for RunTask {
    fn execute(
        &self,
        options: Vec<String>,
        arguments: HashMap<String, String>,
        subcommands: Vec<String>,
    ) -> Result<(), StrategyError> {
        println!("options: {options:?}");
        println!("arguments: {arguments:?}");
        println!("subcommands: {subcommands:?}");
        Ok(())
    }
}

let app = command("app", "Application root")
    .subcommand(
        command("run", "Run tasks")
            .subcommand(Command::new("task", "Execute a task", RunTask)),
    )
    .build();
```

### 3. Run dispatch

```rust
core.run_with_commands(&[]);
```

Or use crate-level helpers:

```rust
cli_core::run_with_commands(&[]);
```

### 4. Run with explicit args (tests/embedding)

```rust
use cli_core::CliCoreError;

fn run_embedded(args: Vec<String>) -> Result<(), CliCoreError> {
    let core = CliCore::new();
    core.try_run_from_args(&args)
}
```

### 5. Pass parsed flags and values

```rust
let args = vec![
    "app".to_string(),
    "new".to_string(),
    "--name".to_string(),
    "my_app".to_string(),
];

core.try_run_from_args(&args)?;
```

## Configuring the runtime

`CoreConfig` is runtime-owned and immutable after `CliCore::create(config)`.

```rust
use cli_core::{CliCore, CoreConfig, LockPoisonPolicy};

let config = CoreConfig::new()
    .with_lock_poison_policy(LockPoisonPolicy::Recover);

let core = CliCore::create(config);
```

## Custom help rendering

```rust
use cli_core::{Command, HelpRenderer};

struct JsonHelpRenderer;

impl HelpRenderer for JsonHelpRenderer {
    fn render(&self, caller: &str, commands: &[Command]) -> String {
        format!("{{\"bin\":\"{}\",\"command_count\":{}}}", caller, commands.len())
    }
}
```

Use it with `CoreConfig::with_help_renderer(...)`.

## Proc macro (`#[cli]`)

The `#[cli]` macro lives in the separate `cli-core-macros` crate. Add it alongside `cli-core` and import it from that package:

```rust
use std::collections::HashMap;

use cli_core::StrategyError;
use cli_core_macros::cli;

#[cli]
fn list_files(
    &self,
    options: Vec<String>,
    arguments: HashMap<String, String>,
    subcommands: Vec<String>,
) -> Result<(), StrategyError> {
    println!("options: {options:?}");
    println!("arguments: {arguments:?}");
    println!("subcommands: {subcommands:?}");
    Ok(())
}
```

This generates `ListFiles` with `ListFiles::new()` and a `list_files_strategy()` factory.

If you do not want the macro crate, you can still build commands directly with `Command::new(...)` or `Command::from_fn(...)`.

## Error model

- routing errors: `CliCoreError`
- strategy errors: `StrategyError` with kinds
  - `InvalidArguments`
  - `Execution`
  - `Internal`
- `CliCoreError::StrategyExecution` retains the original strategy error as source

## Notes

- command lookup is flat by command name at the runtime boundary
- help is metadata-driven and can recursively traverse registered subcommand trees
- routing commands only forward nested subcommands; leaf strategies consume parsed flags and values
- the runtime is instance-owned, so the architecture stays portable and does not depend on process-global state
