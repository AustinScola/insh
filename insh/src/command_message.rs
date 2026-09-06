/*!
What a command has to say for itself once it has run.
*/

/// The text shown before the keys which do not form a command.
const UNKNOWN_COMMAND: &str = "unknown command ";

/// What a command has to say for itself once it has run.
#[derive(Debug, PartialEq, Eq)]
pub enum CommandMessage {
    /// The command ran but nothing on the screen shows that it did.
    Ran(String),
    /// The keys pressed do not form a command.
    UnknownCommand(String),
    /// The command could not be run.
    Failed(String),
}

impl CommandMessage {
    /// Return how the message is shown.
    pub fn text(&self) -> String {
        match self {
            Self::Ran(what) => what.clone(),
            Self::UnknownCommand(keys) => format!("{}{}", UNKNOWN_COMMAND, keys),
            Self::Failed(why) => why.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_ran_is_shown_as_it_is() {
        assert_eq!(
            CommandMessage::Ran(String::from("yanked path")).text(),
            "yanked path"
        );
    }

    #[test]
    fn an_unknown_command_is_shown_with_the_keys_pressed() {
        assert_eq!(
            CommandMessage::UnknownCommand(String::from("yq")).text(),
            "unknown command yq"
        );
    }

    #[test]
    fn why_a_command_failed_is_shown_as_it_is() {
        assert_eq!(CommandMessage::Failed(String::from("nope")).text(), "nope");
    }
}
