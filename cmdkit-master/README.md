# cmdkit-master

cmdkit-master is the executor companion crate for `cmdkit`.

It provides a thread-pool-backed `CMDKitMaster` implementation that accepts
command invocations and returns asynchronous completion handles.

## Installation

```bash
cargo add cmdkit-master
```

## Relationship to cmdkit

- `cmdkit` remains the core runtime and builder API.
- `cmdkit-master` adds an opt-in execution model for queued worker dispatch.

## License

Licensed under GPL-3.0-or-later.
