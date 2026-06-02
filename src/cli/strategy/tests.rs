use std::sync::{Arc, Mutex};

use super::{
    CommandStrategy, FallbackSubcommandStrategy, FunctionStrategy, SubcommandCatalog,
    SubcommandRouter,
};
use crate::{Command, StrategyError, StrategyErrorKind, SubcommandRouter as PublicRouter, command};

#[test]
fn router_errors_when_subcommand_token_is_missing() {
    let router = SubcommandRouter::new().register(
        command("run", "run command")
            .handler_fn(|_, _, _| Ok(()))
            .build(),
    );

    let result = router.execute(Vec::new(), Vec::new(), Vec::new());
    match result {
        Err(err) => {
            assert_eq!(err.kind, StrategyErrorKind::InvalidArguments);
            assert!(err.message.contains("missing subcommand"));
            assert!(err.message.contains("run"));
        }
        _ => panic!("expected missing subcommand error"),
    }
}

#[test]
fn router_errors_when_subcommand_is_unknown() {
    let router = SubcommandRouter::new().register(
        command("run", "run command")
            .handler_fn(|_, _, _| Ok(()))
            .build(),
    );

    let result = router.execute(Vec::new(), Vec::new(), vec!["ghost".to_string()]);
    match result {
        Err(err) => {
            assert_eq!(err.kind, StrategyErrorKind::InvalidArguments);
            assert!(err.message.contains("unknown subcommand 'ghost'"));
            assert!(err.message.contains("run"));
        }
        _ => panic!("expected unknown subcommand error"),
    }
}

#[test]
fn router_resolves_alias_and_forwards_tail_params() {
    let calls: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let calls_for_handler = Arc::clone(&calls);

    let subcommand = command("run", "run command")
        .with_aliases(vec!["r"])
        .handler_fn(move |_, _, params| {
            calls_for_handler
                .lock()
                .expect("calls lock should not be poisoned")
                .push(params);
            Ok(())
        })
        .build();

    let router = SubcommandRouter::new().register(subcommand);
    let result = router.execute(
        Vec::new(),
        Vec::new(),
        vec!["r".to_string(), "tail-1".to_string(), "tail-2".to_string()],
    );

    assert!(result.is_ok());
    let guard = calls.lock().expect("calls lock should not be poisoned");
    assert_eq!(
        guard.as_slice(),
        &[vec!["tail-1".to_string(), "tail-2".to_string()]]
    );
}

#[test]
fn fallback_executes_primary_strategy_when_no_params_are_provided() {
    let fallback_calls: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let fallback_calls_for_handler = Arc::clone(&fallback_calls);
    let child_calls: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let child_calls_for_handler = Arc::clone(&child_calls);

    let fallback_strategy: Arc<dyn CommandStrategy> =
        Arc::new(FunctionStrategy::new(move |_, _, params| {
            fallback_calls_for_handler
                .lock()
                .expect("fallback lock should not be poisoned")
                .push(params);
            Ok(())
        }));

    let router = PublicRouter::new().register(
        command("run", "run command")
            .handler_fn(move |_, _, params| {
                child_calls_for_handler
                    .lock()
                    .expect("child lock should not be poisoned")
                    .push(params);
                Ok(())
            })
            .build(),
    );

    let fallback = FallbackSubcommandStrategy::new(fallback_strategy, router);
    let result = fallback.execute(Vec::new(), Vec::new(), Vec::new());

    assert!(result.is_ok());
    assert_eq!(
        fallback_calls
            .lock()
            .expect("fallback lock should not be poisoned")
            .as_slice(),
        &[Vec::<String>::new()]
    );
    assert!(
        child_calls
            .lock()
            .expect("child lock should not be poisoned")
            .is_empty()
    );
}

#[test]
fn fallback_routes_to_router_when_params_exist() {
    let fallback_calls: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let fallback_calls_for_handler = Arc::clone(&fallback_calls);
    let child_calls: Arc<Mutex<Vec<Vec<String>>>> = Arc::new(Mutex::new(Vec::new()));
    let child_calls_for_handler = Arc::clone(&child_calls);

    let fallback_strategy: Arc<dyn CommandStrategy> =
        Arc::new(FunctionStrategy::new(move |_, _, params| {
            fallback_calls_for_handler
                .lock()
                .expect("fallback lock should not be poisoned")
                .push(params);
            Ok(())
        }));

    let router = PublicRouter::new().register(
        command("run", "run command")
            .handler_fn(move |_, _, params| {
                child_calls_for_handler
                    .lock()
                    .expect("child lock should not be poisoned")
                    .push(params);
                Ok(())
            })
            .build(),
    );

    let fallback = FallbackSubcommandStrategy::new(fallback_strategy, router);
    let result = fallback.execute(
        Vec::new(),
        Vec::new(),
        vec!["run".to_string(), "tail".to_string()],
    );

    assert!(result.is_ok());
    assert!(
        fallback_calls
            .lock()
            .expect("fallback lock should not be poisoned")
            .is_empty()
    );
    assert_eq!(
        child_calls
            .lock()
            .expect("child lock should not be poisoned")
            .as_slice(),
        &[vec!["tail".to_string()]]
    );
}

struct CatalogStrategy {
    catalog: CatalogOnly,
}

impl CommandStrategy for CatalogStrategy {
    fn execute(
        &self,
        _options: Vec<crate::Switch>,
        _arguments: Vec<crate::Argument>,
        _params: Vec<String>,
    ) -> Result<(), StrategyError> {
        Ok(())
    }

    fn subcommand_catalog(&self) -> Option<&dyn SubcommandCatalog> {
        Some(&self.catalog)
    }
}

struct CatalogOnly {
    commands: Vec<Command>,
}

impl SubcommandCatalog for CatalogOnly {
    fn subcommands(&self) -> Vec<Command> {
        self.commands.clone()
    }
}

#[test]
fn fallback_catalog_merges_router_and_fallback_without_duplicate_names() {
    let router = PublicRouter::new()
        .register(
            command("shared", "router shared")
                .handler_fn(|_, _, _| Ok(()))
                .build(),
        )
        .register(
            command("router-only", "router only")
                .handler_fn(|_, _, _| Ok(()))
                .build(),
        );

    let fallback_strategy: Arc<dyn CommandStrategy> = Arc::new(CatalogStrategy {
        catalog: CatalogOnly {
            commands: vec![
                command("shared", "fallback shared")
                    .handler_fn(|_, _, _| Ok(()))
                    .build(),
                command("fallback-only", "fallback only")
                    .handler_fn(|_, _, _| Ok(()))
                    .build(),
            ],
        },
    });

    let fallback = FallbackSubcommandStrategy::new(fallback_strategy, router);
    let mut names = SubcommandCatalog::subcommands(&fallback)
        .into_iter()
        .map(|cmd| cmd.metadata.name)
        .collect::<Vec<String>>();
    names.sort();

    assert_eq!(
        names,
        vec![
            "fallback-only".to_string(),
            "router-only".to_string(),
            "shared".to_string(),
        ]
    );
}
