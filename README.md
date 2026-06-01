# CMDkit

CMDkit is a small, implementation-first Rust framework for building command-line tools.

It is designed around three ideas:

- explicit command trees
- instance-owned runtime state
- strategy-based command execution

That makes it a good fit for CLIs that need nested routing, testable dispatch, and predictable parsing without process-global state.

## Installation

```bash
cargo add cmdkit
```

## Highlights

- Register commands with `Command::new(...)` or fluent `command(...).build()`.
- Attach handlers as structs (`CommandStrategy`) or closures (`handler_fn` / `Command::from_fn`).
- Compose nested command hierarchies with subcommands.
- Parse command input into three channels:
    - `options: Vec<Switch>` for switch/flag inputs
    - `arguments: Vec<Argument>` for value-bearing inputs
    - `params: Vec<String>` for remaining positional parameters
- Customize help output via `HelpRenderer`.
- Configure the runtime help renderer via `CoreConfig`.

## Core API

### Runtime

- `CliCore::new()` creates a runtime with default configuration.
- `CliCore::builder()` starts a fluent builder for registering commands before building the runtime.
- `CliCore::create(config)` uses custom `CoreConfig`.
- `register`, `get`, and `get_all` manage command registration on a runtime instance.
- `try_run_from_args(&[String])` is ideal for tests and embedding.
- `run_with_commands` and `try_run_with_commands` are convenience wrappers.

Each `CliCore` instance owns its own registry. Runtime state is not shared across instances.

### Command Construction

- `Command::new(name, description, strategy)`
- `Command::from_fn(name, description, closure)`
- `command(name, description)` fluent builder:
  - `.handler(...)`
  - `.handler_fn(...)`
  - `.subcommand(...)`
  - `.with_usage(...)`
  - `.with_long_description(...)`
  - `.with_examples(...)`
  - `.with_options(...)`
  - `.with_arguments(...)`
  - `.with_aliases(...)`
  - `.build()`

### Metadata Declarations

CMDkit metadata separates value-taking inputs from switch-like inputs:

- `switch(...)` / `Switch`: declares switch/flag inputs
- `argument(...)` / `Argument`: declares value-bearing inputs

Both support aliases.

## Quick Start

```rust
use cmdkit::{argument, command, switch, Argument, CliCore, CommandStrategy, StrategyError, Switch};

struct CreateProject;

impl CommandStrategy for CreateProject {
    fn execute(
        &self,
        options: Vec<Switch>,
        arguments: Vec<Argument>,
        _params: Vec<String>,
    ) -> Result<(), StrategyError> {
        let name = arguments
            .iter()
            .find(|arg| arg.name == "name")
            .and_then(|arg| arg.value.clone())
            .ok_or_else(|| StrategyError::invalid_arguments("missing --name <value>"))?;

        let language = arguments
            .iter()
            .find(|arg| arg.name == "language")
            .and_then(|arg| arg.value.clone())
            .ok_or_else(|| StrategyError::invalid_arguments("missing --language <value>"))?;

        let dry_run = options.iter().any(|flag| flag.name == "dry-run");

        println!("create project: {name}, language: {language}, dry-run: {dry_run}");
        Ok(())
    }
}

fn main() {
  
    let core = CliCore::builder()
          .register(
                command("create", "Create a new project")
                  .handler(CreateProject)
                  .with_aliases(vec!["new", "init"])
                  .with_options(vec![
                      switch("dry-run", "Preview only").with_aliases(vec!["check".to_string()]),
                  ])
                  .with_arguments(vec![
                      argument("name", "Project name").with_aliases(vec!["n"]),
                      argument("language", "Target language").with_aliases(vec!["l"]),
                  ])
                  .build())
            .try_run_from_env()
            .expect("CLI execution failed");
}
```

## Nested Command Trees

Nested trees can be built directly with the fluent builder:

```rust
use cmdkit::{command, CliCore};
fn main () {
    let core = CliCore::builder()
        .register(
            command("project", "Project commands")
                .subcommand(
                    command("create", "Create a project").handler_fn(|options, arguments, _| {
                        println!("options={options:?} arguments={arguments:?}");
                        Ok(())
                    }),
                )
                .subcommand(
                    command("delete", "Delete a project").handler_fn(|_, arguments, params| {
                        println!("arguments={arguments:?} params={params:?}");
                        Ok(())
                      }),
                    )
                    .build(),
          ).build();
}

```

Routing commands forward execution to leaf commands. The selected leaf strategy receives parsed input.

## Parser Behavior

For an invocation like:

```text
app create --name demo --language rust --dry-run
```

the strategy receives:

- an `Argument { name: "name", value: Some("demo") }`
- an `Argument { name: "language", value: Some("rust") }`
- an `options` entry with `Switch { name: "dry-run", ... }`

Supported forms include:

- `--key value`
- `--key=value`
- aliases declared in metadata

Unknown flags are rejected with `StrategyErrorKind::InvalidArguments`.

## Strategy Token Semantics

For `try_run_from_args`, CMDkit applies deterministic forwarding rules:

- `argv[1]` selects the top-level command only.
- The selected command receives and parses `argv[2..]`.
- Parsing at each command level stops at the first token that matches a declared subcommand name or alias.
- That boundary token and the remaining tail are forwarded to subcommand routing.
- Any non-flag tokens seen before the boundary stay in `params` at the current command level.
- After a subcommand boundary, parsing responsibility shifts to the selected child command.

Practical implication: if you pass `tool run --mode fast`, the `--mode` token is parsed by `run` (the child), not by `tool` (the parent).

## Help Rendering

Default help is plain text via `PlainTextHelpRenderer` and includes recursively discovered subcommands.

Trigger help with:

```text
<binary> help
```

Or rely on the generated help from `MissingCommand` / `UnknownCommand` errors.

You can provide a custom renderer:

```rust
use cmdkit::{Command, HelpRenderer};

struct JsonHelp;

impl HelpRenderer for JsonHelp {
    fn render(&self, caller: &str, commands: &[Command]) -> String {
        format!("{{\"bin\":\"{}\",\"commands\":{}}}", caller, commands.len())
    }
}
```

## Runtime Configuration

````rust

use cmdkit::{CliCore, CoreConfig};

fn main() {
    let config = CoreConfig::new();
    let core = CliCore::builder().with_config(config).build();
}

````

Use `CoreConfig` to customize runtime behavior such as the help renderer.
The registry is owned per `CliCore` instance and does not rely on lock-poison handling.

## Error Model

- `CliCoreError` for dispatch/runtime-level failures:
  - `MissingCommand`
  - `UnknownCommand`
  - `StrategyExecution`
- `StrategyError` for command handler failures with `StrategyErrorKind`:
  - `InvalidArguments`
  - `Execution`
  - `Internal`

`CliCoreError::StrategyExecution` preserves the originating `StrategyError` as source.

## Testing and Embedding

Use `try_run_from_args` to test dispatch deterministically:

```rust
use cmdkit::{CliCore, CliCoreError};

fn run_embedded(args: Vec<String>) -> Result<(), CliCoreError> {
    let core = CliCore::new();
    core.try_run_from_args(&args)
}
```

## License

This project is licensed under GPL-3.0-or-later.
