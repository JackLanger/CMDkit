mod command;
mod error;
mod registry;
mod strategy;

pub use command::{Command, CommandBuilder, CommandMetaData, Switch, command};
pub use error::{StrategyError, StrategyErrorKind};
pub(crate) use registry::CommandRegistry;
pub use strategy::{CommandStrategy, FunctionStrategy, SubcommandCatalog, SubcommandRouter};
