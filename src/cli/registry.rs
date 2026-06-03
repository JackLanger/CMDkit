use super::Command;
use std::collections::{BTreeMap, BTreeSet};

/// Internal registry storage for command-to-command mappings.
#[derive(Default, Clone)]
pub(crate) struct CommandCatalogue {
    commands: BTreeMap<String, Command>,
    aliases: BTreeMap<String, String>,
}

impl CommandCatalogue {
    pub(crate) fn get(&self, name: &str) -> Option<&Command> {
        if let Some(command) = self.commands.get(name) {
            return Some(command);
        }

        self.aliases
            .get(name)
            .and_then(|canonical| self.commands.get(canonical))
    }

    pub(crate) fn register(&mut self, command: Command) -> Result<&mut CommandCatalogue, String> {
        self.validate_alias_collisions(&command)?;

        let command_name = command.metadata.name.clone();

        if let Some(previous) = self.commands.remove(&command_name) {
            for alias in previous.metadata.aliases {
                self.aliases.remove(&alias);
            }
        }

        for alias in &command.metadata.aliases {
            self.aliases.insert(alias.clone(), command_name.clone());
        }

        self.commands.insert(command_name, command);
        Ok(self)
    }

    pub(crate) fn get_all(&self) -> Vec<&Command> {
        self.commands.values().collect()
    }

    fn validate_alias_collisions(&self, command: &Command) -> Result<(), String> {
        if let Some(existing_owner) = self.aliases.get(&command.metadata.name)
            && existing_owner != &command.metadata.name
        {
            return Err(format!(
                "command name '{}' conflicts with existing alias owned by '{}'",
                command.metadata.name, existing_owner
            ));
        }

        let mut seen_aliases = BTreeSet::new();
        for alias in &command.metadata.aliases {
            if alias == &command.metadata.name {
                return Err(format!(
                    "alias '{}' duplicates command name '{}'",
                    alias, command.metadata.name
                ));
            }

            if !seen_aliases.insert(alias.clone()) {
                return Err(format!(
                    "alias '{}' is declared more than once for command '{}'",
                    alias, command.metadata.name
                ));
            }

            if let Some(existing_command) = self.commands.get(alias)
                && existing_command.metadata.name != command.metadata.name
            {
                return Err(format!(
                    "alias '{}' conflicts with existing command name '{}'",
                    alias, existing_command.metadata.name
                ));
            }

            if let Some(existing_owner) = self.aliases.get(alias)
                && existing_owner != &command.metadata.name
            {
                return Err(format!(
                    "alias '{}' conflicts with existing alias owned by '{}'",
                    alias, existing_owner
                ));
            }
        }

        Ok(())
    }
}
