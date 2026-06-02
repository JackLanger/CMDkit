use std::{error::Error, fmt};

/// Categorizes strategy failures for downstream error handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyErrorKind {
    /// Input arguments are syntactically valid for CLI routing but invalid for this strategy.
    InvalidArguments,
    /// Strategy business logic failed during normal execution.
    Execution,
    /// Unexpected internal failure in strategy implementation.
    Internal,
}

impl fmt::Display for StrategyErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::InvalidArguments => "invalid-arguments",
            Self::Execution => "execution",
            Self::Internal => "internal",
        };
        write!(f, "{label}")
    }
}

/// Structured error returned by [`super::CommandStrategy::execute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyError {
    /// Machine-friendly error category.
    pub kind: StrategyErrorKind,
    /// Human-readable explanation.
    pub message: String,
}

impl StrategyError {
    /// Creates a new strategy error from a kind and message.
    pub fn new(kind: StrategyErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Convenience constructor for argument validation failures.
    pub fn invalid_arguments(message: impl Into<String>) -> Self {
        Self::new(StrategyErrorKind::InvalidArguments, message)
    }

    /// Convenience constructor for runtime execution failures.
    pub fn execution(message: impl Into<String>) -> Self {
        Self::new(StrategyErrorKind::Execution, message)
    }

    /// Convenience constructor for internal or unexpected failures.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StrategyErrorKind::Internal, message)
    }
}

impl fmt::Display for StrategyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl Error for StrategyError {}

#[cfg(test)]
mod tests;
