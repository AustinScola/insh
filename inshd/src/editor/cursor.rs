//! One of the cursors in a file being edited.

use super::text::Edit;

use insh_api::{EditorKey, EditorMode, Position};

use typed_builder::TypedBuilder;

/// One of the cursors in a file being edited.
///
/// Everything about how a cursor is being moved is kept here rather than by whoever is moving it,
/// so that a file can have any number of them in it at once, each in a mode of its own and each
/// with its own yanked text.
#[derive(Debug, Default, TypedBuilder)]
pub struct Cursor {
    /// Where it is.
    #[builder(default)]
    pub at: Position,
    /// What the keys pressed for it do.
    #[builder(default)]
    pub mode: EditorMode,
    /// Where what it is selecting starts from.
    #[builder(default)]
    pub anchor: Option<Position>,
    /// The column it goes back to when it moves onto a line long enough for it, which is where it
    /// was last moved along a line to.
    #[builder(default)]
    pub column: usize,
    /// What has been typed of a command.
    #[builder(default)]
    pub command: String,
    /// What has been typed of a search.
    #[builder(default)]
    pub search: String,
    /// What was searched for last.
    #[builder(default)]
    pub searched: Option<String>,
    /// How many times to do what the next key does, while one is being typed.
    #[builder(default)]
    pub count: Option<usize>,
    /// A key pressed which the next key goes on with, while one is waiting for it.
    #[builder(default)]
    pub prefix: Option<char>,
    /// An operator pressed which acts on the movement after it, while one is waiting for it.
    #[builder(default)]
    pub operator: Option<Operator>,
    /// How many characters the next key replaces, while one is waiting for it.
    #[builder(default)]
    pub replacing: Option<usize>,
    /// What was yanked last.
    #[builder(default)]
    pub register: Option<Register>,
    /// Something to tell whoever is moving it.
    #[builder(default)]
    pub message: Option<String>,
}

impl Cursor {
    /// Move it, along with where its selection starts, to wherever the text it is on went to.
    pub fn adjust(&mut self, edit: &Edit) {
        self.at = edit.adjust(self.at);
        self.anchor = self.anchor.map(|anchor| edit.adjust(anchor));
    }

    /// Put it at the position, and take that as the column it goes back to.
    pub fn go(&mut self, at: Position) {
        self.at = at;
        self.column = at.column;
    }

    /// Take the key as part of a count if it is a digit which starts or goes on with one, and
    /// return whether it was.
    ///
    /// NOTE: A nought on its own is the movement to the start of the line, and is only part of a
    /// count once one has been started.
    pub fn counts(&mut self, key: EditorKey) -> bool {
        let digit: usize = match key {
            EditorKey::Char('0') if self.count.is_none() => return false,
            EditorKey::Char(digit @ '0'..='9') => digit.to_digit(10).unwrap_or_default() as usize,
            _ => return false,
        };

        let count: usize = self.count.unwrap_or(0);
        self.count = Some(count.saturating_mul(10).saturating_add(digit));
        return true;
    }

    /// Go into the mode.
    pub fn enter(&mut self, mode: EditorMode) {
        self.anchor = match mode.selects() {
            true => Some(self.anchor.unwrap_or(self.at)),
            false => None,
        };
        self.mode = mode;
        self.count = None;
        self.prefix = None;
        self.operator = None;
        self.replacing = None;
        self.command.clear();
        self.search.clear();
    }
}

/// An operator waiting for the movement it acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TypedBuilder)]
pub struct Operator {
    /// The key which is the operator.
    pub key: char,
    /// How many times to do it, which is typed before it.
    #[builder(default)]
    pub count: Option<usize>,
}

/// Yanked text, along with how it was selected, which is what says where it goes when it is put
/// back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Register {
    /// Some characters.
    Chars(String),
    /// Whole lines.
    Lines(Vec<String>),
    /// A part of each of some lines.
    Block(Vec<String>),
}

impl Register {
    /// Return it as text for the clipboard.
    pub fn text(&self) -> String {
        return match self {
            Self::Chars(chars) => chars.clone(),
            Self::Lines(lines) => lines.iter().map(|line| format!("{}\n", line)).collect(),
            Self::Block(lines) => lines.join("\n"),
        };
    }
}
