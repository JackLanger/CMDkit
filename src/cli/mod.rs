mod command;
mod error;
mod registry;
mod strategy;

pub use command::{
    Argument, Command, CommandBuilder, CommandMetaData, Switch, argument, command, switch,
};
pub use error::{StrategyError, StrategyErrorKind};
pub(crate) use registry::CommandCatalogue;
pub use strategy::{CommandStrategy, FunctionStrategy, SubcommandCatalog, SubcommandRouter};
