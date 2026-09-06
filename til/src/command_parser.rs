use term::{Key, KeyEvent, KeyMods};

/// A key which a key event is matched against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPattern {
    /// The key which must be pressed.
    key: Key,
    /// The modifiers which must be held, or `None` if any modifiers match.
    mods: Option<KeyMods>,
}

impl KeyPattern {
    /// Return a pattern which matches a key pressed with exactly the given modifiers.
    pub fn exact(key: Key, mods: KeyMods) -> Self {
        Self {
            key,
            mods: Some(mods),
        }
    }

    /// Return a pattern which matches a key pressed with any modifiers.
    pub fn any(key: Key) -> Self {
        Self { key, mods: None }
    }

    /// Return whether or not a key event matches the pattern.
    fn matches(&self, key_event: &KeyEvent) -> bool {
        if key_event.key != self.key {
            return false;
        }
        match &self.mods {
            Some(mods) => &key_event.mods == mods,
            None => true,
        }
    }
}

/// The result of parsing a key event.
#[derive(Debug, PartialEq, Eq)]
pub enum Parsed<Command> {
    /// The keys pressed so far form a command.
    Command(Command),
    /// The keys pressed so far are the beginning of one or more commands, so more keys are needed.
    Pending,
    /// The keys pressed are not the beginning of any command.
    Unknown(Vec<KeyEvent>),
}

/// A sequence of keys and the command which it invokes.
#[derive(Debug)]
struct Binding<Command> {
    /// The keys which must be pressed in order.
    keys: Vec<KeyPattern>,
    /// The command which the keys invoke.
    command: Command,
}

impl<Command> Binding<Command> {
    /// Return whether or not the keys pressed so far match the beginning of the binding.
    fn matches(&self, pending: &[KeyEvent]) -> bool {
        self.keys
            .iter()
            .zip(pending)
            .all(|(pattern, key_event)| pattern.matches(key_event))
    }
}

/// Parses sequences of key events into commands.
///
/// Keys which do not complete a command on their own are remembered until they do (or until they
/// cannot).
#[derive(Debug)]
pub struct CommandParser<Command> {
    /// The commands which can be parsed.
    bindings: Vec<Binding<Command>>,
    /// The keys which have been pressed but have not formed a command yet.
    pending: Vec<KeyEvent>,
}

impl<Command> Default for CommandParser<Command> {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
            pending: Vec::new(),
        }
    }
}

impl<Command: Clone> CommandParser<Command> {
    /// Return a parser without any commands.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a command which is invoked by pressing the given keys in order.
    ///
    /// The same command may be bound to more than one sequence of keys.
    pub fn bind(mut self, keys: impl Into<Vec<KeyPattern>>, command: Command) -> Self {
        self.bindings.push(Binding {
            keys: keys.into(),
            command,
        });
        self
    }

    /// Return the keys which have been pressed but have not formed a command yet.
    pub fn pending(&self) -> &[KeyEvent] {
        &self.pending
    }

    /// Parse a key event, returning the command it completes, if more keys are needed, or that the
    /// keys pressed do not form a command.
    ///
    /// The pending keys are forgotten unless more keys are needed.
    pub fn parse(&mut self, key_event: KeyEvent) -> Parsed<Command> {
        self.pending.push(key_event);

        for binding in &self.bindings {
            if binding.keys.len() == self.pending.len() && binding.matches(&self.pending) {
                let command: Command = binding.command.clone();
                self.pending.clear();
                return Parsed::Command(command);
            }
        }

        for binding in &self.bindings {
            if binding.keys.len() > self.pending.len() && binding.matches(&self.pending) {
                return Parsed::Pending;
            }
        }

        Parsed::Unknown(std::mem::take(&mut self.pending))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The commands used for testing.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Command {
        Down,
        YankName,
        YankPath,
    }

    /// Return a parser with a single key command, a two key command, and a command which is bound
    /// to two different keys.
    fn parser() -> CommandParser<Command> {
        CommandParser::new()
            .bind(
                [KeyPattern::exact(Key::Char('j'), KeyMods::NONE)],
                Command::Down,
            )
            .bind([KeyPattern::any(Key::LineFeed)], Command::Down)
            .bind(
                [
                    KeyPattern::exact(Key::Char('y'), KeyMods::NONE),
                    KeyPattern::exact(Key::Char('e'), KeyMods::NONE),
                ],
                Command::YankName,
            )
            .bind(
                [
                    KeyPattern::exact(Key::Char('y'), KeyMods::NONE),
                    KeyPattern::exact(Key::Char('y'), KeyMods::NONE),
                ],
                Command::YankPath,
            )
    }

    /// Return a key event for a character pressed without any modifiers.
    fn char(character: char) -> KeyEvent {
        KeyEvent {
            key: Key::Char(character),
            mods: KeyMods::NONE,
        }
    }

    #[test]
    fn single_key_command() {
        let mut parser = parser();
        assert_eq!(parser.parse(char('j')), Parsed::Command(Command::Down));
    }

    #[test]
    fn unbound_key() {
        let mut parser = parser();
        assert_eq!(parser.parse(char('q')), Parsed::Unknown(vec![char('q')]));
    }

    #[test]
    fn modifiers_must_match() {
        let mut parser = parser();
        let key_event = KeyEvent {
            key: Key::Char('j'),
            mods: KeyMods::CONTROL,
        };
        assert_eq!(
            parser.parse(key_event.clone()),
            Parsed::Unknown(vec![key_event])
        );
    }

    #[test]
    fn modifiers_are_ignored_when_any() {
        let mut parser = parser();
        assert_eq!(
            parser.parse(KeyEvent {
                key: Key::LineFeed,
                mods: KeyMods::CONTROL,
            }),
            Parsed::Command(Command::Down)
        );
    }

    #[test]
    fn multiple_key_commands() {
        let mut parser = parser();
        assert_eq!(parser.parse(char('y')), Parsed::Pending);
        assert_eq!(parser.parse(char('e')), Parsed::Command(Command::YankName));

        assert_eq!(parser.parse(char('y')), Parsed::Pending);
        assert_eq!(parser.parse(char('y')), Parsed::Command(Command::YankPath));
    }

    #[test]
    fn pending_keys_are_forgotten_when_the_command_is_unknown() {
        let mut parser = parser();
        assert_eq!(parser.parse(char('y')), Parsed::Pending);
        assert_eq!(
            parser.parse(char('j')),
            Parsed::Unknown(vec![char('y'), char('j')])
        );
        assert_eq!(parser.parse(char('j')), Parsed::Command(Command::Down));
    }

    #[test]
    fn the_keys_of_a_command_being_typed_are_pending() {
        let mut parser = parser();
        assert!(parser.pending().is_empty());

        parser.parse(char('y'));
        assert_eq!(parser.pending(), [char('y')]);

        parser.parse(char('e'));
        assert!(parser.pending().is_empty());
    }
}
