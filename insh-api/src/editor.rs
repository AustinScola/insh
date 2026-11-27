/*!
The requests and responses which carry the files a client is editing.

A client does not own any of this. The daemon opens the file, keeps the text of it and every
cursor in it, and works out what each key does; a client says what was typed and draws what it is
sent. That is what lets more than one cursor be in the one file at once, each of them with a mode
and a selection of its own, whoever it is that is moving it.
*/
use std::path::{Path, PathBuf};

use highlighter::Run;
use rend::Size;

use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// A spot in the text of a file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Position {
    /// Which line it is on, counted from nought.
    pub line: usize,
    /// Which character of the line it is before, counted from nought.
    pub column: usize,
}

impl Position {
    /// Return the position on the line before the character.
    pub fn new(line: usize, column: usize) -> Self {
        return Self { line, column };
    }
}

/// What the keys pressed for a cursor do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorMode {
    /// Moving around.
    #[default]
    Normal,
    /// Typing a command.
    Command,
    /// Typing text in.
    Insert,
    /// Selecting characters.
    Visual,
    /// Selecting lines.
    VisualLine,
    /// Selecting a block.
    VisualBlock,
    /// Typing a search.
    Search,
}

impl EditorMode {
    /// Return whether it is one of the modes something is being selected in.
    pub fn selects(&self) -> bool {
        return matches!(self, Self::Visual | Self::VisualLine | Self::VisualBlock);
    }
}

/// A key pressed in the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorKey {
    /// A character.
    Char(char),
    /// A character held with control.
    Control(char),
    /// The escape key.
    Escape,
    /// The enter key.
    Enter,
    /// The tab key.
    Tab,
    /// The key which deletes the character before the cursor.
    Backspace,
}

/// One of the cursors in a file.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct EditorCursor {
    /// Which cursor it is.
    cursor: Uuid,
    /// Where it is.
    at: Position,
    /// What the keys pressed for it do.
    mode: EditorMode,
    /// Where what it is selecting starts from, which is nowhere at all when it is not selecting.
    #[builder(default)]
    anchor: Option<Position>,
}

impl EditorCursor {
    /// Return which cursor it is.
    pub fn cursor(&self) -> &Uuid {
        return &self.cursor;
    }

    /// Return where it is.
    pub fn at(&self) -> Position {
        return self.at;
    }

    /// Return what the keys pressed for it do.
    pub fn mode(&self) -> EditorMode {
        return self.mode;
    }

    /// Return where what it is selecting starts from.
    pub fn anchor(&self) -> Option<Position> {
        return self.anchor;
    }
}

/// What of a file a client is shown.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct EditorView {
    /// The file.
    path: PathBuf,
    /// Which cursor is the client's own.
    cursor: Uuid,
    /// Which line the first one shown is.
    top: usize,
    /// The lines shown.
    lines: Vec<String>,
    /// What each piece of each of the lines shown is, which is nothing for a file in a language
    /// which is not known.
    #[builder(default)]
    runs: Vec<Vec<Run>>,
    /// How many lines the file has.
    line_count: usize,
    /// Every cursor in the file.
    cursors: Vec<EditorCursor>,
    /// What has been typed of a command, while one is being typed.
    #[builder(default)]
    command: Option<String>,
    /// What has been typed of a search, while one is being typed.
    #[builder(default)]
    search: Option<String>,
    /// Something to tell whoever is looking.
    #[builder(default)]
    message: Option<String>,
    /// Whether there are changes which have not been written.
    #[builder(default)]
    modified: bool,
}

impl EditorView {
    /// Return the file.
    pub fn path(&self) -> &Path {
        return &self.path;
    }

    /// Return which cursor is the client's own.
    pub fn cursor(&self) -> &Uuid {
        return &self.cursor;
    }

    /// Return which line the first one shown is.
    pub fn top(&self) -> usize {
        return self.top;
    }

    /// Return the lines shown.
    pub fn lines(&self) -> &[String] {
        return &self.lines;
    }

    /// Return what each piece of each of the lines shown is.
    pub fn runs(&self) -> &[Vec<Run>] {
        return &self.runs;
    }

    /// Return how many lines the file has.
    pub fn line_count(&self) -> usize {
        return self.line_count;
    }

    /// Return every cursor in the file.
    pub fn cursors(&self) -> &[EditorCursor] {
        return &self.cursors;
    }

    /// Return the client's own cursor.
    pub fn own(&self) -> Option<&EditorCursor> {
        return self
            .cursors
            .iter()
            .find(|cursor| cursor.cursor == self.cursor);
    }

    /// Return what has been typed of a command.
    pub fn command(&self) -> Option<&str> {
        return self.command.as_deref();
    }

    /// Return what has been typed of a search.
    pub fn search(&self) -> Option<&str> {
        return self.search.as_deref();
    }

    /// Return something to tell whoever is looking.
    pub fn message(&self) -> Option<&str> {
        return self.message.as_deref();
    }

    /// Return whether there are changes which have not been written.
    pub fn modified(&self) -> bool {
        return self.modified;
    }
}

/// Request parameters for editing a file, which goes on being reported on until the cursor is
/// done with.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct EditRequestParams {
    /// The file.
    path: PathBuf,
    /// How much text is shown.
    size: Size,
    /// Where the cursor starts, which is the start of the file when it is nowhere in particular.
    #[builder(default)]
    at: Option<Position>,
}

impl EditRequestParams {
    /// Return the file.
    pub fn path(&self) -> &Path {
        return &self.path;
    }

    /// Return how much text is shown.
    pub fn size(&self) -> Size {
        return self.size;
    }

    /// Return where the cursor starts.
    pub fn at(&self) -> Option<Position> {
        return self.at;
    }
}

/// Request parameters for pressing a key for a cursor.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct EditorKeyRequestParams {
    /// Which cursor.
    cursor: Uuid,
    /// The key.
    key: EditorKey,
}

impl EditorKeyRequestParams {
    /// Return which cursor.
    pub fn cursor(&self) -> &Uuid {
        return &self.cursor;
    }

    /// Return the key.
    pub fn key(&self) -> EditorKey {
        return self.key;
    }
}

/// Request parameters for pasting text for a cursor.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct PasteEditorRequestParams {
    /// Which cursor.
    cursor: Uuid,
    /// The text.
    text: String,
}

impl PasteEditorRequestParams {
    /// Return which cursor.
    pub fn cursor(&self) -> &Uuid {
        return &self.cursor;
    }

    /// Return the text.
    pub fn text(&self) -> &str {
        return &self.text;
    }
}

/// Request parameters for taking note of how much text is shown for a cursor.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct ResizeEditorRequestParams {
    /// Which cursor.
    cursor: Uuid,
    /// How much text is shown.
    size: Size,
}

impl ResizeEditorRequestParams {
    /// Return which cursor.
    pub fn cursor(&self) -> &Uuid {
        return &self.cursor;
    }

    /// Return how much text is shown.
    pub fn size(&self) -> Size {
        return self.size;
    }
}

/// Request parameters for being done with a cursor which is no longer shown.
///
/// The cursor is named by the request which made it rather than by the cursor itself, so that one
/// can be done with before the client has been told which cursor it is.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct CloseEditorRequestParams {
    /// The request which the file was edited on.
    edit: Uuid,
}

impl CloseEditorRequestParams {
    /// Return the request which the file was edited on.
    pub fn edit(&self) -> &Uuid {
        return &self.edit;
    }
}

/// Response parameters for what of a file is shown.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct EditorViewResponseParams {
    /// What is shown.
    ///
    /// NOTE: It is boxed so that every other response is not made as big as it is.
    #[builder(setter(into))]
    view: Box<EditorView>,
}

impl EditorViewResponseParams {
    /// Return what is shown.
    pub fn view(&self) -> &EditorView {
        return &self.view;
    }
}

/// Response parameters for text to put on the clipboard.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct EditorClipboardResponseParams {
    /// The text.
    text: String,
}

impl EditorClipboardResponseParams {
    /// Return the text.
    pub fn text(&self) -> &str {
        return &self.text;
    }
}

/// Response parameters for a cursor being done with, which is the last response.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct EditorClosedResponseParams {
    /// Why the file could not be edited, when it could not be.
    #[builder(default)]
    error: Option<String>,
    /// Where the cursor was, when it was done with to open the file in vim instead.
    #[builder(default)]
    vim: Option<Position>,
}

impl EditorClosedResponseParams {
    /// Return why the file could not be edited.
    pub fn error(&self) -> Option<&str> {
        return self.error.as_deref();
    }

    /// Return where the cursor was, when it was done with to open the file in vim instead.
    pub fn vim(&self) -> Option<Position> {
        return self.vim;
    }
}
