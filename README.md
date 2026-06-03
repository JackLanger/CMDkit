# CMDkit

CMDkit is a deterministic command-execution runtime that separates command definition from invocation parsing and execution orchestration, while enabling full runtime configuration during setup.

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

- `CMDKit::new()` creates a runtime with default configuration.
- `CMDKit::builder()` starts a fluent builder for registering commands before building the runtime.
- `CMDKit::create(config)` uses custom `CoreConfig`.
- `register`, `get`, and `get_all` manage command registration on a runtime instance.
- `try_run_from_args(&[String])` is ideal for tests and embedding.
- `run_with_commands` and `try_run_with_commands` are convenience wrappers.

Each `CMDKit` instance owns its own registry. Runtime state is not shared across instances.

### Architecture Contract

The runtime model follows a strict build-then-dispatch lifecycle:

- Mutation is builder-only: command registration and config changes happen in `CMDKitBuilder`.
- `build()` is the freeze boundary: once built, `CMDKit` has no runtime mutation API.
- No process-global mutable state: each `CMDKit` instance owns an isolated registry and config.
- Runtime operations are read-only: dispatch and lookup use immutable access to core state.
- Dispatch is deterministic: `try_run_from_args` takes explicit argv input and returns structured errors.

Invariants:

- A built `CMDKit` never mutates its registry or config during runtime.
- Two distinct `CMDKit` instances do not share mutable state and cannot affect each other.

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
use cmdkit::{argument, command, switch, CMDKit, CommandStrategy, InvocationArgs, StrategyError};

struct CreateProject;

impl CommandStrategy for CreateProject {
    fn execute(&self, invocation: InvocationArgs) -> Result<(), StrategyError> {
        let options = invocation.switches;
        let arguments = invocation.args;

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
  
    let core = CMDKit::builder()
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
use cmdkit::{command, CMDKit};
fn main () {
    let core = CMDKit::builder()
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

use cmdkit::{CMDKit, CoreConfig};

fn main() {
    let config = CoreConfig::new();
    let core = CMDKit::builder().with_config(config).build();
}

````

Use `CoreConfig` to customize runtime behavior such as the help renderer.
The registry is owned per `CMDKit` instance and does not rely on lock-poison handling.

## Implementing Extensions

CMDkit exposes two main extension points: `HelpRenderer` and `ArgumentInterpreter`.

### Custom Help Renderer

Implement `HelpRenderer` when you want to replace the default plain-text help output:

```rust
use cmdkit::{Command, HelpRenderer};

struct CompactHelp;

impl HelpRenderer for CompactHelp {
    fn render(&self, caller: &str, commands: &[Command]) -> String {
        format!("{}: {} commands available", caller, commands.len())
    }
}
```

### Custom Argument Interpreter

Implement `ArgumentInterpreter` when you want to control how raw input is turned into invocation data:

```rust
use cmdkit::{ArgumentInterpreter, CMDKitError, Command, InvocationArgs};

struct FixedCommandInterpreter;

impl ArgumentInterpreter for FixedCommandInterpreter {
    fn interpret(
        &self,
        _arg: &[String],
        _registered_commands: &[Command],
    ) -> Result<InvocationArgs, CMDKitError> {
        Ok(InvocationArgs {
            name: "status".to_string(),
            args: Vec::new(),
            switches: Vec::new(),
            params: Vec::new(),
            order: Vec::new(),
            subcommand: None,
        })
    }
}
```

## Error Model

- `CMDKitError` for dispatch/runtime-level failures:
  - `MissingCommand`
  - `UnknownCommand`
  - `StrategyExecution`
- `StrategyError` for command handler failures with `StrategyErrorKind`:
  - `InvalidArguments`
  - `Execution`
  - `Internal`

`CMDKitError::StrategyExecution` preserves the originating `StrategyError` as source.

## Testing and Embedding

Use `try_run_from_args` to test dispatch deterministically:

```rust
use cmdkit::{CMDKit, CMDKitError};

fn run_embedded(args: Vec<String>) -> Result<(), CMDKitError> {
    let core = CMDKit::builder().build();
    core.try_run_from_args(&args)
}
```

## License

This project is licensed under Apache-2.0. See [LICENSE](LICENSE) for details.
