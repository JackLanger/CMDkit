use crate::cli::{CLIStrategy, get_all_global};

pub struct DisplayHelp;

impl CLIStrategy for DisplayHelp {
    fn execute(&self, _args: Vec<String>) {
        println!("{}", self.help());
    }

    fn accepts(&self, command: &str) -> bool {
        command == "help"
    }

    fn help(&self) -> String {
        format!(
            r#"Usage: projectmngr <project_language> <project_name>
    Projectmngr is simple cli tool to initiate and manage projects on disk. It helps in create new Projects from templates and manage them with ease.

    Example: projectmngr new c my_project

    supported commands:
    {}

        "#,
            get_all_global()
                .iter()
                .map(|e| format!("    - {}: {}", e.name, e.description))
                .collect::<Vec<String>>()
                .join("\n")
        )
    }
}
