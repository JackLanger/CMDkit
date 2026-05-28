# CLI-Core

CLI-Core is a Rust library for command dispatch with an instance-owned runtime.

The current architecture is intentionally simple:

- `Command` is a metadata wrapper plus strategy reference
- `CommandStrategy` is responsible for runtime behavior
- subtask/chained routing is handled inside strategies (chain of responsibility)
- help output is rendered from command metadata through a pluggable `HelpRenderer`

## Core Concepts

### Command model

`Command` does not own a subcommand tree. It contains:

- `metadata: CommandMetaData`
- `strategy: Arc<dyn CommandStrategy>`

`CommandMetaData` includes required and optional help-facing fields:

- required: `name`, `description`
- optional: `usage`, `long_description`, `examples`, `options`, `aliases`

Use `CommandMetaData::new(...)` and builder-style metadata methods such as `with_usage(...)`, `with_examples(...)`, and `with_options(...)`.

### Strategy model

`CommandStrategy` defines one method:

```rust
fn execute(&self, args: Vec<String>) -> Result<(), StrategyError>
```

`CliCore` resolves `argv[1]` as the command name and forwards `argv[2..]` to `execute`.
If you need nested behavior (for example `test all --fast`), implement it in your strategy.

### Help rendering

Help is rendered from registered command metadata via `HelpRenderer`.

Default behavior uses `PlainTextHelpRenderer`, configured in `CoreConfig::new()`.
You can inject a custom renderer with `CoreConfig::with_help_renderer(...)`.

## Quick Start

### 1. Define a strategy

```rust
use cli_core::{CommandStrategy, StrategyError};

struct NewProject;

impl CommandStrategy for NewProject {
    fn execute(&self, args: Vec<String>) -> Result<(), StrategyError> {
        let project_name = args
            .first()
            .cloned()
            .ok_or_else(|| StrategyError::invalid_arguments("missing <project_name>"))?;

        println!("creating project: {project_name}");
        Ok(())
    }
}
```

### 2. Register commands

```rust
use std::sync::Arc;
use cli_core::{CliCore, Command, CommandMetaData};

let core = CliCore::new();
core.register(Command {
    metadata: CommandMetaData::new("new", "Create a new project")
        .with_usage("new <project_name>")
        .with_examples(vec!["new my_app".to_string()]),
    strategy: Arc::new(NewProject),
});
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

## Configuring The Runtime

`CoreConfig` is runtime-owned and immutable after `CliCore::create(config)`.

```rust
use cli_core::{CliCore, CoreConfig, LockPoisonPolicy};

let config = CoreConfig::new()
    .with_lock_poison_policy(LockPoisonPolicy::Recover);

let core = CliCore::create(config);
```

## Custom Help Rendering

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

## Proc Macro (`#[cli]`)

The `#[cli]` macro generates a strategy wrapper type from a function that matches `execute` shape:

```rust
use cli_core::{cli, StrategyError};

#[cli]
fn list_files(&self, args: Vec<String>) -> Result<(), StrategyError> {
    println!("args: {args:?}");
    Ok(())
}
```

This generates `ListFiles` with `ListFiles::new()` and a `list_files_strategy()` factory.

## Error Model

- routing errors: `CliCoreError`
- strategy errors: `StrategyError` with kinds
  - `InvalidArguments`
  - `Execution`
  - `Internal`
- `CliCoreError::StrategyExecution` retains the original strategy error as source

## Notes

- command lookup is flat by command name
- help is metadata-driven, not strategy-method-driven
- strategy implementations own deeper argument interpretation and chaining
