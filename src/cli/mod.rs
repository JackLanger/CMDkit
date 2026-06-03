mod command;
mod error;
mod registry;
mod strategy;

pub use command::{
    Argument, ArgumentDefinition, ArgumentValue, Command, CommandBuilder, CommandMetaData,
    SwitchDefinition, ValueType, argument, argument_of_type, command, switch,
};

pub use error::{StrategyError, StrategyErrorKind};
pub(crate) use registry::CommandCatalogue;
pub use strategy::{CommandStrategy, FunctionStrategy, SubcommandCatalog, SubcommandRouter};
