use super::{StrategyError, StrategyErrorKind};

#[test]
fn strategy_error_kind_display_labels_are_stable() {
    assert_eq!(
        StrategyErrorKind::InvalidArguments.to_string(),
        "invalid-arguments"
    );
    assert_eq!(StrategyErrorKind::Execution.to_string(), "execution");
    assert_eq!(StrategyErrorKind::Internal.to_string(), "internal");
}

#[test]
fn strategy_error_constructors_set_kind_and_message() {
    let invalid = StrategyError::invalid_arguments("bad input");
    assert_eq!(invalid.kind, StrategyErrorKind::InvalidArguments);
    assert_eq!(invalid.message, "bad input");

    let execution = StrategyError::execution("failed");
    assert_eq!(execution.kind, StrategyErrorKind::Execution);
    assert_eq!(execution.message, "failed");

    let internal = StrategyError::internal("boom");
    assert_eq!(internal.kind, StrategyErrorKind::Internal);
    assert_eq!(internal.message, "boom");
}

#[test]
fn strategy_error_display_formats_kind_and_message() {
    let err = StrategyError::new(StrategyErrorKind::Execution, "could not run");
    assert_eq!(err.to_string(), "execution: could not run");
}
