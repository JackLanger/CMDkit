use std::collections::BTreeMap;

use super::Command;

/// Internal registry storage for command-to-command mappings.
#[derive(Default, Clone)]
pub(crate) struct CommandRegistry {
    commands: BTreeMap<String, Command>,
}

impl CommandRegistry {
    pub(crate) fn get(&self, name: &str) -> Option<Command> {
        self.commands.get(name).cloned()
    }

    pub(crate) fn register(&mut self, command: Command) -> &mut CommandRegistry {
        self.commands.insert(command.metadata.name.clone(), command);
        self
    }

    pub(crate) fn get_all(&self) -> Vec<Command> {
        self.commands.values().cloned().collect()
    }
}
