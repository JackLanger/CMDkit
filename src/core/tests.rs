use std::{
    panic,
    sync::{Arc, RwLock},
};

use super::{CliCore, LockPoisonPolicy};
use crate::{cli::CommandRegistry, core::CoreConfig};

#[test]
fn lock_poison_policy_defaults_to_fail_fast() {
    let core = CliCore::new();
    assert_eq!(core.lock_poison_policy(), LockPoisonPolicy::FailFast);
}

#[test]
fn fail_fast_policy_panics_on_poisoned_read_lock() {
    let core = CliCore::new();
    let lock = Arc::new(RwLock::new(CommandRegistry::new()));

    let lock_for_thread = Arc::clone(&lock);
    let _ = std::thread::spawn(move || {
        let _guard = lock_for_thread
            .write()
            .expect("write lock should be acquired");
        panic!("poison lock");
    })
    .join();

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let poisoned = match lock.read() {
            Ok(_) => panic!("lock should be poisoned"),
            Err(poisoned) => poisoned,
        };
        drop(core.handle_poison(poisoned, "read"));
    }));
    assert!(result.is_err());
}

#[test]
fn recover_policy_returns_inner_guard_on_poisoned_read_lock() {
    let config = CoreConfig::new().with_lock_poison_policy(LockPoisonPolicy::Recover);

    let core = CliCore::create(config);

    let lock = Arc::new(RwLock::new(CommandRegistry::new()));
    let lock_for_thread = Arc::clone(&lock);
    let _ = std::thread::spawn(move || {
        let _guard = lock_for_thread
            .write()
            .expect("write lock should be acquired");
        panic!("poison lock");
    })
    .join();

    let poisoned = match lock.read() {
        Ok(_) => panic!("lock should be poisoned"),
        Err(poisoned) => poisoned,
    };
    let _guard = core.handle_poison(poisoned, "read");
}

#[test]
fn core_config_defaults_to_plain_text_help_renderer() {
    let config = CoreConfig::new();
    let text = config.help_renderer.render("app", &[]);
    assert!(text.contains("Usage:"));
}
