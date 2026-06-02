use std::thread;

use cmdkit::{CMDKit, CMDKitBuilder, CMDKitError, Command, CoreConfig, InvocationInterpreter};
use crossbeam_channel::Sender;
use crossbeam_channel::unbounded;
use futures_channel::oneshot;

type WorkerResult = Result<(), CMDKitError>;

/// A future handle that resolves when a submitted invocation completes.
///
/// Awaiting this handle yields `Result<Result<(), CMDKitError>, Canceled>` where:
/// - outer `Err(Canceled)` means the executor dropped the completion channel.
/// - outer `Ok(inner)` carries the command execution result.
pub type ExecutionHandle = oneshot::Receiver<WorkerResult>;

struct QueuedInvocation {
    args: Vec<String>,
    completion_tx: oneshot::Sender<WorkerResult>,
}

pub trait CMDKitMaster: Send + Sync {
    /// Submits an invocation for worker execution and returns its completion handle.
    ///
    /// This method is non-blocking with respect to command execution. Callers can
    /// await the returned [`ExecutionHandle`] to observe completion.
    fn try_run_from_args(&self, args: &[String]) -> Result<ExecutionHandle, CMDKitError>;

    /// Submits process argv for worker execution and returns its completion handle.
    fn try_run_from_env(&self) -> Result<ExecutionHandle, CMDKitError>;

    /// Stops accepting work and waits for all workers to drain and stop.
    fn shutdown(self) -> Result<(), CMDKitError>;
}

/// Companion builder for creating either a plain [`CMDKit`] runtime or a
/// [`ThreadPoolCMDKitMaster`] from the same command/config registration flow.
pub struct CMDKitMasterBuilder {
    inner: CMDKitBuilder,
}

impl CMDKitMasterBuilder {
    /// Creates a new companion builder.
    pub fn new() -> Self {
        Self {
            inner: CMDKit::builder(),
        }
    }

    /// Wraps an existing cmdkit builder.
    pub fn from_cmdkit_builder(inner: CMDKitBuilder) -> Self {
        Self { inner }
    }

    /// Registers a command.
    pub fn register(self, command: Command) -> Self {
        Self {
            inner: self.inner.register(command),
        }
    }

    /// Registers a command and returns a structured error on failure.
    pub fn try_register(self, command: Command) -> Result<Self, CMDKitError> {
        Ok(Self {
            inner: self.inner.try_register(command)?,
        })
    }

    /// Registers multiple commands.
    pub fn with_commands(self, commands: &[Command]) -> Self {
        Self {
            inner: self.inner.with_commands(commands),
        }
    }

    /// Registers multiple commands and returns a structured error on failure.
    pub fn try_with_commands(self, commands: &[Command]) -> Result<Self, CMDKitError> {
        Ok(Self {
            inner: self.inner.try_with_commands(commands)?,
        })
    }

    /// Replaces runtime config.
    pub fn with_config(self, config: CoreConfig) -> Self {
        Self {
            inner: self.inner.with_config(config),
        }
    }

    /// Replaces argument interpreter.
    pub fn with_argument_interpreter<I>(self, interpreter: I) -> Self
    where
        I: InvocationInterpreter + 'static,
    {
        Self {
            inner: self.inner.with_argument_interpreter(interpreter),
        }
    }

    /// Builds and returns plain cmdkit runtime.
    pub fn build_runtime(self) -> CMDKit {
        self.inner.build()
    }

    /// Builds and returns thread-pool master executor.
    ///
    /// `worker_count` is mandatory and must be greater than 0.
    pub fn build_master(self, worker_count: usize) -> Result<ThreadPoolCMDKitMaster, CMDKitError> {
        if worker_count == 0 {
            return Err(CMDKitError::ExecutorUnavailable {
                message: "worker_count must be greater than 0".to_string(),
            });
        }

        Ok(ThreadPoolCMDKitMaster::new(self.inner.build(), worker_count))
    }
}

impl Default for CMDKitMasterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Multi-worker command dispatcher that returns awaitable completion handles.
///
/// `CMDKitMaster` accepts invocations immediately and executes them on background
/// worker threads against a shared, immutable [`CMDKit`] runtime instance.
pub struct ThreadPoolCMDKitMaster {
    submit_tx: Option<Sender<QueuedInvocation>>,
    worker_handles: Vec<thread::JoinHandle<()>>,
}

impl CMDKitMaster for ThreadPoolCMDKitMaster {
    fn try_run_from_args(&self, args: &[String]) -> Result<ExecutionHandle, CMDKitError> {
        self.dispatch(args)
    }

    fn try_run_from_env(&self) -> Result<ExecutionHandle, CMDKitError> {
        let argv = std::env::args().collect::<Vec<String>>();
        self.try_run_from_args(&argv)
    }

    fn shutdown(mut self) -> Result<(), CMDKitError> {
        self.close_and_join()
    }
}

impl ThreadPoolCMDKitMaster {
    /// Creates a worker-backed dispatcher around an existing runtime.
    pub fn new(cmdkit: CMDKit, worker_count: usize) -> Self {
        let (submit_tx, submit_rx) = unbounded::<QueuedInvocation>();
        let shared_cmdkit = std::sync::Arc::new(cmdkit);
        let mut worker_handles = Vec::with_capacity(worker_count.max(1));

        for _ in 0..worker_count.max(1) {
            let rx = submit_rx.clone();
            let cmdkit = std::sync::Arc::clone(&shared_cmdkit);

            let handle = thread::spawn(move || {
                loop {
                    let Ok(job) = rx.recv() else {
                        break;
                    };

                    let result = cmdkit.try_run_from_args(&job.args);
                    let _ = job.completion_tx.send(result);
                }
            });

            worker_handles.push(handle);
        }

        Self {
            submit_tx: Some(submit_tx),
            worker_handles,
        }
    }

    fn dispatch(&self, args: &[String]) -> Result<ExecutionHandle, CMDKitError> {
        let (completion_tx, completion_rx) = oneshot::channel();
        self.submit_tx
            .as_ref()
            .ok_or_else(|| CMDKitError::ExecutorUnavailable {
                message: "worker queue is closed".to_string(),
            })?
            .send(QueuedInvocation {
                args: args.to_vec(),
                completion_tx,
            })
            .map_err(|_| CMDKitError::ExecutorUnavailable {
                message: "worker queue is closed".to_string(),
            })?;

        Ok(completion_rx)
    }

    fn close_and_join(&mut self) -> Result<(), CMDKitError> {
        self.submit_tx.take();

        for handle in self.worker_handles.drain(..) {
            handle
                .join()
                .map_err(|_| CMDKitError::ExecutorUnavailable {
                    message: "worker thread panicked during shutdown".to_string(),
                })?;
        }

        Ok(())
    }
}

impl Drop for ThreadPoolCMDKitMaster {
    fn drop(&mut self) {
        let _ = self.close_and_join();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use cmdkit::{CMDKit, CMDKitError, command};
    use futures_executor::block_on;

    use super::{CMDKitMaster, CMDKitMasterBuilder, ThreadPoolCMDKitMaster};

    #[test]
    fn cmdkit_master_executes_command_and_returns_success_handle() {
        let calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let calls_for_handler = Arc::clone(&calls);

        let cmd = command("echo", "echo command").handler_fn(move |_sw, _args, params| {
            calls_for_handler
                .lock()
                .expect("calls lock should not be poisoned")
                .push(params.join(" "));
            Ok(())
        });

        let core = CMDKit::builder().with_commands(&[cmd.build()]).build();
        let master = ThreadPoolCMDKitMaster::new(core, 1);

        let handle = master
            .try_run_from_args(&["app".to_string(), "echo".to_string(), "hello".to_string()])
            .expect("submission should succeed");

        let result = block_on(handle).expect("completion channel should stay open");
        assert!(result.is_ok());

        let guard = calls.lock().expect("calls lock should not be poisoned");
        assert_eq!(guard.as_slice(), ["hello"]);

        drop(guard);
        master.shutdown().expect("shutdown should succeed");
    }

    #[test]
    fn cmdkit_master_propagates_execution_error_through_handle() {
        let core = CMDKit::builder().build();
        let master = ThreadPoolCMDKitMaster::new(core, 1);

        let handle = master
            .try_run_from_args(&["app".to_string(), "missing".to_string()])
            .expect("submission should succeed");

        let result = block_on(handle).expect("completion channel should stay open");
        match result {
            Err(CMDKitError::UnknownCommand { command, .. }) => {
                assert_eq!(command, "missing");
            }
            _ => panic!("expected unknown command error"),
        }

        master.shutdown().expect("shutdown should succeed");
    }

    #[test]
    fn cmdkit_master_help_path_returns_resolved_success_handle() {
        let core = CMDKit::builder().build();
        let master = ThreadPoolCMDKitMaster::new(core, 1);

        let handle = master
            .try_run_from_args(&["app".to_string(), "help".to_string()])
            .expect("help submission should succeed");

        let result = block_on(handle).expect("completion channel should stay open");
        assert!(result.is_ok());

        master.shutdown().expect("shutdown should succeed");
    }

    #[test]
    fn cmdkit_master_normalizes_zero_worker_count() {
        let calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let calls_for_handler = Arc::clone(&calls);

        let cmd = command("ping", "ping command").handler_fn(move |_sw, _args, _params| {
            calls_for_handler
                .lock()
                .expect("calls lock should not be poisoned")
                .push("ran".to_string());
            Ok(())
        });

        let core = CMDKit::builder().with_commands(&[cmd.build()]).build();
        let master = ThreadPoolCMDKitMaster::new(core, 0);

        let handle = master
            .try_run_from_args(&["app".to_string(), "ping".to_string()])
            .expect("submission should succeed");

        let result = block_on(handle).expect("completion channel should stay open");
        assert!(result.is_ok());

        let guard = calls.lock().expect("calls lock should not be poisoned");
        assert_eq!(guard.len(), 1);

        drop(guard);
        master.shutdown().expect("shutdown should succeed");
    }

    #[test]
    fn cmdkit_master_drop_tears_down_workers_without_explicit_shutdown() {
        let calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let calls_for_handler = Arc::clone(&calls);

        let cmd =
            command("drop-check", "drop check command").handler_fn(move |_sw, _args, _params| {
                calls_for_handler
                    .lock()
                    .expect("calls lock should not be poisoned")
                    .push("executed".to_string());
                Ok(())
            });

        {
            let core = CMDKit::builder().with_commands(&[cmd.build()]).build();
            let master = ThreadPoolCMDKitMaster::new(core, 2);

            let handle = master
                .try_run_from_args(&["app".to_string(), "drop-check".to_string()])
                .expect("submission should succeed");

            let result = block_on(handle).expect("completion channel should stay open");
            assert!(result.is_ok());

            // Intentionally rely on Drop for executor teardown.
        }

        let guard = calls.lock().expect("calls lock should not be poisoned");
        assert_eq!(guard.as_slice(), ["executed"]);
    }

    #[test]
    fn companion_builder_builds_master_executor() {
        let calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let calls_for_handler = Arc::clone(&calls);

        let cmd = command("builder-check", "builder check command").handler_fn(
            move |_sw, _args, _params| {
                calls_for_handler
                    .lock()
                    .expect("calls lock should not be poisoned")
                    .push("ran".to_string());
                Ok(())
            },
        );

        let master = CMDKitMasterBuilder::new()
            .with_commands(&[cmd.build()])
            .build_master(2)
            .expect("builder should accept non-zero worker count");

        let handle = master
            .try_run_from_args(&["app".to_string(), "builder-check".to_string()])
            .expect("submission should succeed");

        let result = block_on(handle).expect("completion channel should stay open");
        assert!(result.is_ok());

        let guard = calls.lock().expect("calls lock should not be poisoned");
        assert_eq!(guard.as_slice(), ["ran"]);

        drop(guard);
        master.shutdown().expect("shutdown should succeed");
    }

    #[test]
    fn companion_builder_rejects_zero_worker_count() {
        let err = CMDKitMasterBuilder::new()
            .build_master(0)
            .expect_err("worker_count=0 should be rejected");

        match err {
            CMDKitError::ExecutorUnavailable { message } => {
                assert!(message.contains("worker_count"));
            }
            _ => panic!("expected executor unavailable error"),
        }
    }
}
