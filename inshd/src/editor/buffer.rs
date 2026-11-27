//! A file being edited, along with every cursor in it.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Error as IOError, ErrorKind, Read, Seek, SeekFrom, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};

use super::cursor::{Cursor, Operator, Register};
use super::highlighting::Highlighting;
use super::history::{Change, History};
use super::text::{Edit, Hit, Text};

use highlighter::{Language, Run};
use human_friendly::Bytes;
use insh_api::{EditorCursor, EditorKey, EditorMode, Position};

use uuid::Uuid;

/// A file being edited, along with every cursor in it.
///
/// The file is held open for as long as it is being edited, and the text of it is kept here rather
/// than by any client, so that every cursor in it is editing the one text. A change made at one
/// cursor moves the others along with the text they are on.
pub struct Buffer {
    /// The file.
    path: PathBuf,
    /// The open file, which is not there until it is first written when it did not exist.
    file: Option<File>,
    /// Whether the file could be opened to be written.
    writable: bool,
    /// The text.
    text: Text,
    /// Whether the file ended with a newline, which it is written back with.
    eol: bool,
    /// The changes made to the text.
    history: History,
    /// What each piece of the text is.
    highlighting: Highlighting,
    /// The cursors, by which one each is.
    cursors: HashMap<Uuid, Cursor>,
    /// How many cursors have been put in it, which is what keeps them in the order they came.
    order: Vec<Uuid>,
}

/// What pressing a key asks of whatever is keeping the buffer, which is what it cannot do itself.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    /// Put the text on the clipboard of whoever is moving the cursor.
    Clipboard(String),
    /// Be done with the cursor.
    Quit,
    /// Be done with the cursor and open the file in vim where it was instead.
    Vim(Position),
    /// Scroll what whoever is moving the cursor is shown.
    Scroll(Scroll),
}

/// Which way to scroll, and by how many lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scroll {
    /// Down by the lines.
    Down(usize),
    /// Up by the lines.
    Up(usize),
}

/// What an operator acts on.
#[derive(Debug, Clone, Copy)]
enum Span {
    /// The characters from the start up to the end.
    Chars {
        /// The first character.
        start: Position,
        /// Where the characters stop.
        end: Position,
    },
    /// Whole lines.
    Lines {
        /// The first line.
        first: usize,
        /// The last line.
        last: usize,
    },
}

impl Buffer {
    /// Return the file opened to be edited.
    pub fn open(path: &Path) -> Result<Self, OpenError> {
        let (file, writable): (Option<File>, bool) =
            match OpenOptions::new().read(true).write(true).open(path) {
                Ok(file) => (Some(file), true),
                Err(error) if error.kind() == ErrorKind::PermissionDenied => {
                    match File::open(path) {
                        Ok(file) => (Some(file), false),
                        Err(error) => return Err(OpenError::Open(error)),
                    }
                }
                // NOTE: A file which does not exist yet is made once it is written, so that
                // leaving without writing it leaves nothing behind.
                Err(error) if error.kind() == ErrorKind::NotFound => (None, true),
                Err(error) => return Err(OpenError::Open(error)),
            };

        let mut contents: String = String::new();
        if let Some(mut file) = file.as_ref() {
            if let Err(error) = file.read_to_string(&mut contents) {
                return Err(OpenError::Read(error));
            }
        }

        let body: &str = contents.strip_suffix('\n').unwrap_or(&contents);
        // NOTE: A new file is written ending with a newline, the way every line does.
        let eol: bool = file.is_none() || body.len() < contents.len();

        return Ok(Self {
            path: path.to_path_buf(),
            file,
            writable,
            text: Text::from(body),
            eol,
            history: History::builder().build(),
            highlighting: Highlighting::builder()
                .language(
                    path.extension()
                        .and_then(|extension| extension.to_str())
                        .and_then(Language::of_extension),
                )
                .build(),
            cursors: HashMap::new(),
            order: Vec::new(),
        });
    }

    /// Return the file.
    pub fn path(&self) -> &Path {
        return &self.path;
    }

    /// Return the text.
    pub fn text(&self) -> &Text {
        return &self.text;
    }

    /// Return what each piece of each of the lines in the range is.
    pub fn runs(&mut self, range: Range<usize>) -> Vec<Vec<Run>> {
        return self.highlighting.runs(self.text.lines(), range);
    }

    /// Return whether there are changes which have not been written.
    pub fn modified(&self) -> bool {
        return self.history.modified();
    }

    /// Return whether there are no cursors in it.
    pub fn is_empty(&self) -> bool {
        return self.cursors.is_empty();
    }

    /// Put a new cursor in it, as close to the position as one can be.
    pub fn add(&mut self, id: Uuid, at: Position) {
        let mut cursor: Cursor = Cursor::builder().build();
        cursor.go(self.text.within(at, cursor.mode));
        self.cursors.insert(id, cursor);
        self.order.push(id);
    }

    /// Take the cursor out of it.
    pub fn remove(&mut self, id: &Uuid) {
        self.cursors.remove(id);
        self.order.retain(|cursor| cursor != id);
    }

    /// Return the cursor.
    pub fn cursor(&self, id: &Uuid) -> Option<&Cursor> {
        return self.cursors.get(id);
    }

    /// Move the cursor onto the closest of the lines from the first to the last, if it is not on
    /// one of them already.
    pub fn keep(&mut self, id: &Uuid, first: usize, last: usize) {
        let cursor: &mut Cursor = match self.cursors.get_mut(id) {
            Some(cursor) => cursor,
            None => return,
        };

        let line: usize = cursor
            .at
            .line
            .clamp(first, last.max(first))
            .min(self.text.len() - 1);
        if line != cursor.at.line {
            let column: usize = cursor.column.min(self.text.last_column(line, cursor.mode));
            cursor.at = Position::new(line, column);
        }
    }

    /// Return every cursor, the way a client is shown them.
    pub fn cursors(&self) -> Vec<EditorCursor> {
        return self
            .order
            .iter()
            .filter_map(|id| {
                let cursor: &Cursor = self.cursors.get(id)?;
                Some(
                    EditorCursor::builder()
                        .cursor(*id)
                        .at(cursor.at)
                        .mode(cursor.mode)
                        .anchor(cursor.anchor)
                        .build(),
                )
            })
            .collect();
    }

    /// Do what the key does for the cursor, and return what that asks of whatever is keeping the
    /// buffer.
    pub fn press(&mut self, id: &Uuid, key: EditorKey) -> Option<Asked> {
        // NOTE: The cursor is taken out while the key is handled so that the text can be changed
        // and every other cursor moved along with it. Where this one goes is always said outright.
        let mut cursor: Cursor = self.cursors.remove(id)?;
        cursor.message = None;

        let asked: Option<Asked> = match cursor.mode {
            EditorMode::Normal => self.normal(&mut cursor, key),
            EditorMode::Command => self.command(&mut cursor, key),
            EditorMode::Insert => self.insert(&mut cursor, key),
            EditorMode::Visual | EditorMode::VisualLine | EditorMode::VisualBlock => {
                self.visual(&mut cursor, key)
            }
            EditorMode::Search => self.search(&mut cursor, key),
        };

        self.history.commit(*id, cursor.mode == EditorMode::Insert);
        cursor.at = self.text.within(cursor.at, cursor.mode);
        self.cursors.insert(*id, cursor);
        return asked;
    }

    /// Put the pasted text in at the cursor, if it is typing text in.
    pub fn paste_in(&mut self, id: &Uuid, text: &str) {
        let mut cursor: Cursor = match self.cursors.remove(id) {
            Some(cursor) => cursor,
            None => return,
        };

        if cursor.mode == EditorMode::Insert {
            cursor.message = None;
            // NOTE: A terminal sends a new line in pasted text as a carriage return, the same as
            // pressing enter.
            let text: String = text.replace("\r\n", "\n").replace('\r', "\n");
            let edit: Edit = self.put(cursor.at, &text);
            cursor.go(edit.end());
            self.history.commit(*id, true);
        }

        self.cursors.insert(*id, cursor);
    }

    /// Write the text to the file.
    pub fn write(&mut self) -> Result<usize, WriteError> {
        if !self.writable {
            return Err(WriteError::ReadOnly);
        }

        let mut contents: String = self.text.lines().join("\n");
        // NOTE: A new file which nothing was typed in is written empty rather than as one empty
        // line, the way vim writes it.
        if self.eol && !(self.file.is_none() && contents.is_empty()) {
            contents.push('\n');
        }

        if let Err(error) = self.overwrite(contents.as_bytes()) {
            return Err(WriteError::Write(error));
        }

        self.history.write();
        return Ok(contents.len());
    }

    /// Replace what is in the file with the contents.
    ///
    /// NOTE: The file which is held open is the one written to, rather than a new one being put in
    /// its place, so that it is the same file which is open all the while it is edited. A file
    /// which did not exist is made, and is then the one held open.
    fn overwrite(&mut self, contents: &[u8]) -> Result<(), IOError> {
        let file: &mut File = match &mut self.file {
            Some(file) => file,
            None => self.file.insert(
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(&self.path)?,
            ),
        };
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(contents)?;
        file.sync_data()?;
        return Ok(());
    }

    /// Handle a key pressed for a cursor in normal mode.
    fn normal(&mut self, cursor: &mut Cursor, key: EditorKey) -> Option<Asked> {
        // NOTE: The key after `r` is the character to replace with, even when it is a digit.
        if let Some(times) = cursor.replacing.take() {
            self.replace(cursor, key, times);
            return None;
        }
        if cursor.counts(key) {
            return None;
        }
        let count: Option<usize> = cursor.count.take();
        // NOTE: A count typed before the operator and one typed after it are multiplied together.
        if let Some(operator) = cursor.operator.take() {
            let count: Option<usize> = match (operator.count, count) {
                (None, None) => None,
                (before, after) => Some(before.unwrap_or(1).saturating_mul(after.unwrap_or(1))),
            };
            self.operate(cursor, operator.key, key, count);
            return None;
        }
        let times: usize = count.unwrap_or(1);
        if self.moves(cursor, key, count, false) {
            return None;
        }

        match key {
            EditorKey::Char('i') => cursor.enter(EditorMode::Insert),
            EditorKey::Char('a') => {
                let at: Position = cursor.at;
                let column: usize = (at.column + 1).min(self.text.columns(at.line));
                cursor.enter(EditorMode::Insert);
                cursor.go(Position::new(at.line, column));
            }
            EditorKey::Char('A') => {
                let line: usize = cursor.at.line;
                cursor.enter(EditorMode::Insert);
                cursor.go(Position::new(line, self.text.columns(line)));
            }
            EditorKey::Char('o') => {
                let line: usize = cursor.at.line;
                self.put(Position::new(line, self.text.columns(line)), "\n");
                cursor.enter(EditorMode::Insert);
                cursor.go(Position::new(line + 1, 0));
            }
            EditorKey::Char('O') => {
                let line: usize = cursor.at.line;
                self.put(Position::new(line, 0), "\n");
                cursor.enter(EditorMode::Insert);
                cursor.go(Position::new(line, 0));
            }
            EditorKey::Char('v') => cursor.enter(EditorMode::Visual),
            EditorKey::Char('V') => cursor.enter(EditorMode::VisualLine),
            EditorKey::Control('v') => cursor.enter(EditorMode::VisualBlock),
            EditorKey::Char(' ') => {
                self.put(cursor.at, " ");
            }
            EditorKey::Char('p') => self.paste(cursor, true),
            EditorKey::Char('P') => self.paste(cursor, false),
            EditorKey::Char('x') => {
                let at: Position = cursor.at;
                let columns: usize = self.text.columns(at.line);
                let to: Position =
                    Position::new(at.line, at.column.saturating_add(times).min(columns));
                if to.column > at.column {
                    cursor.register = Some(Register::Chars(self.take(at, to)));
                }
                cursor.go(self.text.within(at, cursor.mode));
            }
            EditorKey::Char(operator @ ('d' | 'y')) => {
                cursor.operator = Some(Operator::builder().key(operator).count(count).build());
            }
            EditorKey::Char('r') => cursor.replacing = Some(times),
            EditorKey::Char(':') => cursor.enter(EditorMode::Command),
            EditorKey::Char('/') => cursor.enter(EditorMode::Search),
            EditorKey::Char('n') => self.hit(cursor, true, times),
            EditorKey::Char('N') => self.hit(cursor, false, times),
            EditorKey::Char('u') => self.undo(cursor, times),
            EditorKey::Control('r') => self.redo(cursor, times),
            EditorKey::Control('e') => return Some(Asked::Scroll(Scroll::Down(times))),
            EditorKey::Control('y') => return Some(Asked::Scroll(Scroll::Up(times))),
            _ => {}
        }

        return None;
    }

    /// Replace as many characters as the times from the cursor on with the character typed.
    ///
    /// NOTE: The same as in vim, nothing is replaced when there are not that many characters left
    /// on the line.
    fn replace(&mut self, cursor: &mut Cursor, key: EditorKey, times: usize) {
        let character: char = match key {
            EditorKey::Char(character) if !character.is_control() => character,
            _ => return,
        };
        let at: Position = cursor.at;
        let end: usize = at.column.saturating_add(times);
        if end > self.text.columns(at.line) {
            return;
        }

        self.take(at, Position::new(at.line, end));
        self.put(at, &character.to_string().repeat(times));
        cursor.go(Position::new(at.line, end - 1));
    }

    /// Take back the last changes made to the text, by whichever cursor made them.
    fn undo(&mut self, cursor: &mut Cursor, times: usize) {
        for _ in 0..times {
            let change: Change = match self.history.undo() {
                Some(change) => change,
                None => {
                    cursor.message = Some(String::from("Already at the oldest change"));
                    return;
                }
            };
            for step in change.steps.iter().rev() {
                match step.edit {
                    Edit::Inserted { at, end } => {
                        let (removed, edit): (String, Edit) = self.text.remove(at, end);
                        self.changed(&edit, &removed);
                    }
                    Edit::Removed { from, .. } => {
                        let edit: Edit = self.text.insert(from, &step.text);
                        self.changed(&edit, &step.text);
                    }
                }
            }
            if let Some(start) = change.start() {
                cursor.go(start);
            }
        }
    }

    /// Make the last changes which were undone again.
    fn redo(&mut self, cursor: &mut Cursor, times: usize) {
        for _ in 0..times {
            let change: Change = match self.history.redo() {
                Some(change) => change,
                None => {
                    cursor.message = Some(String::from("Already at the newest change"));
                    return;
                }
            };
            for step in &change.steps {
                match step.edit {
                    Edit::Inserted { at, .. } => {
                        let edit: Edit = self.text.insert(at, &step.text);
                        self.changed(&edit, &step.text);
                    }
                    Edit::Removed { from, to } => {
                        let (removed, edit): (String, Edit) = self.text.remove(from, to);
                        self.changed(&edit, &removed);
                    }
                }
            }
            if let Some(start) = change.start() {
                cursor.go(start);
            }
        }
    }

    /// Delete or yank what the key moves the cursor over, or the lines from the one it is on for
    /// the operator pressed again.
    ///
    /// NOTE: Moving up or down a line, or to another line of the file, takes in every line moved
    /// over, the same as moving a visual line selection would.
    fn operate(
        &mut self,
        cursor: &mut Cursor,
        operator: char,
        key: EditorKey,
        count: Option<usize>,
    ) {
        let times: usize = count.unwrap_or(1);
        let from: Position = cursor.at;
        let column: usize = cursor.column;

        let span: Span = if key == EditorKey::Char(operator) {
            Span::Lines {
                first: from.line,
                last: from.line.saturating_add(times.saturating_sub(1)),
            }
        } else {
            if !self.moves(cursor, key, count, true) {
                return;
            }
            // A movement which waits for another key, like `g`, keeps the operator waiting with it.
            if cursor.prefix.is_some() {
                cursor.operator = Some(Operator::builder().key(operator).count(count).build());
                return;
            }
            let to: Position = cursor.at;
            let columns: usize = self.text.columns(from.line);

            match key {
                EditorKey::Char('h') | EditorKey::Char('0') => Span::Chars {
                    start: to,
                    end: from,
                },
                EditorKey::Char('l') => Span::Chars {
                    start: from,
                    end: Position::new(from.line, from.column.saturating_add(times).min(columns)),
                },
                EditorKey::Char('$') => Span::Chars {
                    start: from,
                    end: Position::new(from.line, columns),
                },
                EditorKey::Char('w' | 'W' | 'b' | 'B') => {
                    self.exclusive(from.min(to), from.max(to))
                }
                EditorKey::Char('e' | 'E') => Span::Chars {
                    start: from,
                    end: Position::new(
                        to.line,
                        to.column.saturating_add(1).min(self.text.columns(to.line)),
                    ),
                },
                _ => Span::Lines {
                    first: from.line.min(to.line),
                    last: from.line.max(to.line),
                },
            }
        };

        match (operator, span) {
            ('d', Span::Chars { start, end }) => {
                if end > start {
                    cursor.register = Some(Register::Chars(self.take(start, end)));
                }
                cursor.go(start);
            }
            ('d', Span::Lines { first, last }) => self.delete_lines(cursor, first, last),
            (_, Span::Chars { start, end }) => {
                if end > start {
                    cursor.register = Some(Register::Chars(self.text.slice(start, end)));
                }
                cursor.go(start);
            }
            // NOTE: Yanking lines leaves the cursor where it was unless it moved up to the first.
            (_, Span::Lines { first, last }) => {
                let last: usize = last.min(self.text.len() - 1);
                cursor.register = Some(Register::Lines(self.text.lines()[first..=last].to_vec()));
                if first == from.line {
                    cursor.at = from;
                    cursor.column = column;
                }
            }
        }
    }

    /// Return what a movement from the start to the end which does not take in the character it
    /// ends on acts on.
    ///
    /// NOTE: The same as in vim, one which ends at the start of a line stops at the end of the line
    /// before it instead, and takes in whole lines when it also started in the indentation of its
    /// line.
    fn exclusive(&self, start: Position, end: Position) -> Span {
        if end.column > 0 || end.line == start.line {
            return Span::Chars { start, end };
        }

        let line: usize = end.line - 1;
        let indent: usize = self.text.lines()[start.line]
            .chars()
            .take_while(|character| matches!(character, ' ' | '\t'))
            .count();
        if start.column <= indent {
            return Span::Lines {
                first: start.line,
                last: line,
            };
        }
        return Span::Chars {
            start,
            end: Position::new(line, self.text.columns(line)),
        };
    }

    /// Delete the lines from the first to the last into the cursor's register.
    fn delete_lines(&mut self, cursor: &mut Cursor, first: usize, last: usize) {
        let last: usize = last.min(self.text.len() - 1);
        cursor.register = Some(Register::Lines(self.text.lines()[first..=last].to_vec()));
        self.remove_lines(first, last);
        cursor.go(Position::new(first.min(self.text.len() - 1), 0));
    }

    /// Handle a key pressed for a cursor in command mode.
    fn command(&mut self, cursor: &mut Cursor, key: EditorKey) -> Option<Asked> {
        match key {
            EditorKey::Char(character) if !character.is_control() => cursor.command.push(character),
            EditorKey::Escape => cursor.enter(EditorMode::Normal),
            // NOTE: The same as in vim, backspacing with nothing typed cancels the command.
            EditorKey::Backspace => {
                if cursor.command.pop().is_none() {
                    cursor.enter(EditorMode::Normal);
                }
            }
            EditorKey::Control('e') => return Some(Asked::Scroll(Scroll::Down(1))),
            EditorKey::Control('y') => return Some(Asked::Scroll(Scroll::Up(1))),
            EditorKey::Enter => {
                let command: String = cursor.command.trim().to_string();
                cursor.enter(EditorMode::Normal);

                match command.as_str() {
                    "w" => cursor.message = Some(self.written()),
                    "q" if self.modified() => {
                        cursor.message =
                            Some(String::from("Failed to quit: there are unwritten changes"));
                    }
                    "q" => return Some(Asked::Quit),
                    "q!" => return Some(Asked::Quit),
                    "wq" => {
                        let message: String = self.written();
                        if self.modified() {
                            cursor.message = Some(message);
                            return None;
                        }
                        return Some(Asked::Quit);
                    }
                    // NOTE: Vim opens what is written, so changes which have not been written would
                    // be left behind.
                    "vim" if self.modified() => {
                        cursor.message = Some(String::from(
                            "Failed to open vim: there are unwritten changes",
                        ));
                    }
                    "vim" => return Some(Asked::Vim(cursor.at)),
                    "vim!" => return Some(Asked::Vim(cursor.at)),
                    _ => {
                        cursor.message = Some(format!("Not an editor command: {}", command));
                    }
                }
            }
            _ => {}
        }

        return None;
    }

    /// Handle a key pressed for a cursor in search mode.
    fn search(&mut self, cursor: &mut Cursor, key: EditorKey) -> Option<Asked> {
        match key {
            EditorKey::Char(character) if !character.is_control() => cursor.search.push(character),
            EditorKey::Escape => cursor.enter(EditorMode::Normal),
            // NOTE: The same as in vim, backspacing with nothing typed cancels the search.
            EditorKey::Backspace => {
                if cursor.search.pop().is_none() {
                    cursor.enter(EditorMode::Normal);
                }
            }
            // NOTE: The same as in vim, searching for nothing searches for what was searched for
            // last.
            EditorKey::Enter => {
                let search: String = std::mem::take(&mut cursor.search);
                cursor.enter(EditorMode::Normal);
                if !search.is_empty() {
                    cursor.searched = Some(search);
                }
                self.hit(cursor, true, 1);
            }
            EditorKey::Control('e') => return Some(Asked::Scroll(Scroll::Down(1))),
            EditorKey::Control('y') => return Some(Asked::Scroll(Scroll::Up(1))),
            _ => {}
        }

        return None;
    }

    /// Go to the next hit of what the cursor searched for last, or the one before it, as many
    /// times as the times.
    fn hit(&self, cursor: &mut Cursor, forward: bool, times: usize) {
        let phrase: String = match &cursor.searched {
            Some(phrase) => phrase.clone(),
            None => {
                cursor.message = Some(String::from("No previous search"));
                return;
            }
        };

        let mut at: Position = cursor.at;
        let mut wrapped: bool = false;
        for _ in 0..times {
            let hit: Hit = match self.text.find(&phrase, at, forward) {
                Some(hit) => hit,
                None => {
                    cursor.message = Some(String::from("Search not found"));
                    return;
                }
            };
            at = hit.at;
            wrapped |= hit.wrapped;
        }

        if wrapped {
            cursor.message = Some(String::from(match forward {
                true => "Search hit the bottom, continuing at the top",
                false => "Search hit the top, continuing at the bottom",
            }));
        }
        cursor.go(at);
    }

    /// Handle a key pressed for a cursor in insert mode.
    fn insert(&mut self, cursor: &mut Cursor, key: EditorKey) -> Option<Asked> {
        let at: Position = cursor.at;
        match key {
            EditorKey::Escape => {
                cursor.enter(EditorMode::Normal);
                cursor.go(Position::new(at.line, at.column.saturating_sub(1)));
            }
            EditorKey::Char(character) if !character.is_control() => {
                let edit: Edit = self.put(at, &character.to_string());
                cursor.go(edit.end());
            }
            EditorKey::Tab => {
                let edit: Edit = self.put(at, "    ");
                cursor.go(edit.end());
            }
            EditorKey::Enter => {
                let edit: Edit = self.put(at, "\n");
                cursor.go(edit.end());
            }
            EditorKey::Control('e') => return Some(Asked::Scroll(Scroll::Down(1))),
            EditorKey::Control('y') => return Some(Asked::Scroll(Scroll::Up(1))),
            EditorKey::Backspace => {
                if at.column > 0 {
                    let before: Position = Position::new(at.line, at.column - 1);
                    self.take(before, at);
                    cursor.go(before);
                } else if at.line > 0 {
                    let end: Position = Position::new(at.line - 1, self.text.columns(at.line - 1));
                    self.take(end, at);
                    cursor.go(end);
                }
            }
            _ => {}
        }

        return None;
    }

    /// Handle a key pressed for a cursor in one of the visual modes.
    fn visual(&mut self, cursor: &mut Cursor, key: EditorKey) -> Option<Asked> {
        // NOTE: The key after `r` is the character to replace with, even when it is a digit.
        if cursor.replacing.take().is_some() {
            if let EditorKey::Char(character) = key {
                if !character.is_control() {
                    let start: Position = self.change_selection(cursor, |piece| {
                        piece
                            .chars()
                            .map(|old| match old {
                                '\n' => old,
                                _ => character,
                            })
                            .collect()
                    });
                    cursor.enter(EditorMode::Normal);
                    cursor.go(start);
                }
            }
            return None;
        }
        if cursor.counts(key) {
            return None;
        }
        let count: Option<usize> = cursor.count.take();
        if self.moves(cursor, key, count, false) {
            return None;
        }
        let times: usize = count.unwrap_or(1);

        match key {
            EditorKey::Escape => cursor.enter(EditorMode::Normal),
            EditorKey::Control('e') => return Some(Asked::Scroll(Scroll::Down(times))),
            EditorKey::Control('y') => return Some(Asked::Scroll(Scroll::Up(times))),
            EditorKey::Char('y') | EditorKey::Char('Y') => {
                let register: Register = self.selection(cursor);
                let text: String = register.text();
                cursor.register = Some(register);
                let start: Position = self.selection_start(cursor);
                cursor.enter(EditorMode::Normal);
                cursor.go(start);

                if key == EditorKey::Char('Y') {
                    return Some(Asked::Clipboard(text));
                }
            }
            EditorKey::Char('x') | EditorKey::Char('d') => {
                cursor.register = Some(self.selection(cursor));
                let start: Position = self.remove_selection(cursor);
                cursor.enter(EditorMode::Normal);
                cursor.go(start);
            }
            EditorKey::Char(case @ ('u' | 'U')) => {
                let start: Position = self.change_selection(cursor, |piece| match case {
                    'u' => piece.to_lowercase(),
                    _ => piece.to_uppercase(),
                });
                cursor.enter(EditorMode::Normal);
                cursor.go(start);
            }
            EditorKey::Char('r') => cursor.replacing = Some(1),
            EditorKey::Char('p') => {
                self.replace_selection(cursor);
                cursor.enter(EditorMode::Normal);
            }
            EditorKey::Char('v') => cursor.enter(EditorMode::Visual),
            EditorKey::Char('V') => cursor.enter(EditorMode::VisualLine),
            EditorKey::Control('v') => cursor.enter(EditorMode::VisualBlock),
            _ => {}
        }

        return None;
    }

    /// Move the cursor as many times as the count says if the key is one which moves it, and
    /// return whether it was.
    ///
    /// NOTE: For an operator, moving to the start of the next word can leave it just past the end
    /// of a line, since that is where what the operator acts on stops.
    fn moves(
        &self,
        cursor: &mut Cursor,
        key: EditorKey,
        count: Option<usize>,
        operating: bool,
    ) -> bool {
        let times: usize = count.unwrap_or(1);
        let at: Position = cursor.at;
        let last: usize = self.text.len() - 1;

        let to_line = |line: usize, column: usize| -> Position {
            let line: usize = line.min(last);
            Position::new(line, column.min(self.text.last_column(line, cursor.mode)))
        };

        let prefix: Option<char> = cursor.prefix.take();

        match key {
            EditorKey::Char('g') if prefix == Some('g') => cursor.at = to_line(0, cursor.column),
            EditorKey::Char('g') => cursor.prefix = Some('g'),
            EditorKey::Char('K') => cursor.at = to_line(0, cursor.column),
            // NOTE: With a count it is the line counted from one rather than the last which is gone
            // to.
            EditorKey::Char('J') | EditorKey::Char('G') => {
                let line: usize = count.map_or(last, |count| count.saturating_sub(1));
                cursor.at = to_line(line, cursor.column)
            }
            EditorKey::Char('h') => {
                cursor.go(Position::new(at.line, at.column.saturating_sub(times)))
            }
            EditorKey::Char('l') => cursor.go(to_line(at.line, at.column.saturating_add(times))),
            EditorKey::Char('j') => {
                cursor.at = to_line(at.line.saturating_add(times), cursor.column)
            }
            EditorKey::Char('k') => {
                cursor.at = to_line(at.line.saturating_sub(times), cursor.column)
            }
            EditorKey::Char('0') => cursor.go(Position::new(at.line, 0)),
            EditorKey::Char('$') => {
                cursor.at = to_line(at.line, usize::MAX);
                cursor.column = usize::MAX;
            }
            EditorKey::Char(word @ ('w' | 'W')) => {
                let to: Position = self.text.next_word_start(at, times, word == 'W', operating);
                cursor.go(match operating {
                    true => to,
                    false => self.text.within(to, cursor.mode),
                });
            }
            EditorKey::Char(word @ ('b' | 'B')) => {
                cursor.go(self.text.previous_word_start(at, times, word == 'B'))
            }
            EditorKey::Char(word @ ('e' | 'E')) => {
                let to: Position = self.text.word_end(at, times, word == 'E');
                cursor.go(self.text.within(to, cursor.mode));
            }
            _ => return false,
        }

        return true;
    }

    /// Put the string in at the position, moving every cursor which is after it.
    fn put(&mut self, at: Position, string: &str) -> Edit {
        let edit: Edit = self.text.insert(at, string);
        self.history.record(edit, string.to_string());
        self.changed(&edit, string);
        return edit;
    }

    /// Take out what is between the two positions, moving every cursor which is after it, and
    /// return what it was.
    fn take(&mut self, from: Position, to: Position) -> String {
        let (removed, edit): (String, Edit) = self.text.remove(from, to);
        self.history.record(edit, removed.clone());
        self.changed(&edit, &removed);
        return removed;
    }

    /// Take note that the text was changed, moving every cursor along with it, given the text
    /// which was put in or taken out.
    fn changed(&mut self, edit: &Edit, text: &str) {
        match *edit {
            Edit::Inserted { at, .. } => self.highlighting.edit(self.text.lines(), at, "", text),
            Edit::Removed { from, .. } => self.highlighting.edit(self.text.lines(), from, text, ""),
        }
        for cursor in self.cursors.values_mut() {
            cursor.adjust(edit);
            cursor.at = self.text.within(cursor.at, cursor.mode);
        }
    }

    /// Write the file, and return what to tell the cursor which asked for it.
    fn written(&mut self) -> String {
        return match self.write() {
            Ok(bytes) => format!("Wrote file ({})", Bytes(bytes)),
            Err(error) => error.to_string(),
        };
    }

    /// Return the first and last positions selected by the cursor, in order.
    fn selected(&self, cursor: &Cursor) -> (Position, Position) {
        let anchor: Position = cursor.anchor.unwrap_or(cursor.at);
        return (anchor.min(cursor.at), anchor.max(cursor.at));
    }

    /// Return the columns of each line which the cursor's block selection takes in.
    fn block_columns(&self, cursor: &Cursor) -> (usize, usize) {
        let anchor: Position = cursor.anchor.unwrap_or(cursor.at);
        return (
            anchor.column.min(cursor.at.column),
            anchor.column.max(cursor.at.column) + 1,
        );
    }

    /// Return where the cursor's selection starts, which is where it goes once it is done with.
    fn selection_start(&self, cursor: &Cursor) -> Position {
        let (first, _): (Position, Position) = self.selected(cursor);
        return match cursor.mode {
            EditorMode::VisualLine => Position::new(first.line, 0),
            EditorMode::VisualBlock => Position::new(first.line, self.block_columns(cursor).0),
            _ => first,
        };
    }

    /// Return what the cursor has selected.
    fn selection(&self, cursor: &Cursor) -> Register {
        let (first, last): (Position, Position) = self.selected(cursor);

        return match cursor.mode {
            EditorMode::VisualLine => Register::Lines(
                self.text.lines()[first.line..=last.line.min(self.text.len() - 1)].to_vec(),
            ),
            EditorMode::VisualBlock => {
                let (from, to): (usize, usize) = self.block_columns(cursor);
                Register::Block(
                    (first.line..=last.line)
                        .map(|line| {
                            self.text
                                .slice(Position::new(line, from), Position::new(line, to))
                        })
                        .collect(),
                )
            }
            _ => Register::Chars(self.text.slice(first, self.text.next(last))),
        };
    }

    /// Take out what the cursor has selected, and return where it started.
    fn remove_selection(&mut self, cursor: &Cursor) -> Position {
        let (first, last): (Position, Position) = self.selected(cursor);
        let start: Position = self.selection_start(cursor);

        match cursor.mode {
            EditorMode::VisualLine => {
                self.remove_lines(first.line, last.line);
                return Position::new(first.line.min(self.text.len() - 1), 0);
            }
            EditorMode::VisualBlock => {
                let (from, to): (usize, usize) = self.block_columns(cursor);
                for line in first.line..=last.line {
                    self.take(Position::new(line, from), Position::new(line, to));
                }
            }
            _ => {
                let after: Position = self.text.next(last);
                self.take(first, after);
            }
        }

        return start;
    }

    /// Return the stretches of text which the cursor has selected, which are each on lines of
    /// their own.
    fn selected_ranges(&self, cursor: &Cursor) -> Vec<(Position, Position)> {
        let (first, last): (Position, Position) = self.selected(cursor);

        return match cursor.mode {
            EditorMode::VisualLine => {
                let last: usize = last.line.min(self.text.len() - 1);
                vec![(
                    Position::new(first.line, 0),
                    Position::new(last, self.text.columns(last)),
                )]
            }
            EditorMode::VisualBlock => {
                let (from, to): (usize, usize) = self.block_columns(cursor);
                (first.line..=last.line)
                    .map(|line| (Position::new(line, from), Position::new(line, to)))
                    .collect()
            }
            _ => vec![(first, self.text.next(last))],
        };
    }

    /// Change each stretch of what the cursor has selected with the change, and return where the
    /// selection started.
    fn change_selection<F>(&mut self, cursor: &Cursor, change: F) -> Position
    where
        F: Fn(&str) -> String,
    {
        let start: Position = self.selection_start(cursor);

        for (from, to) in self.selected_ranges(cursor) {
            let before: String = self.text.slice(from, to);
            let after: String = change(&before);
            if after != before {
                self.take(from, to);
                self.put(from, &after);
            }
        }

        return start;
    }

    /// Replace what the cursor has selected with what it yanked last, which it goes on having
    /// yanked.
    ///
    /// NOTE: The same as in vim, characters put in place of a block are put on each of its lines,
    /// and lines put in place of characters are put on lines of their own.
    fn replace_selection(&mut self, cursor: &mut Cursor) {
        let register: Register = match cursor.register.clone() {
            Some(register) => register,
            None => return,
        };

        match (cursor.mode, register) {
            (EditorMode::VisualLine, register) => {
                let (from, to): (Position, Position) = self.selected_ranges(cursor)[0];
                let text: String = match register {
                    Register::Chars(chars) => chars,
                    Register::Lines(lines) | Register::Block(lines) => lines.join("\n"),
                };
                self.take(from, to);
                self.put(from, &text);
                cursor.go(from);
            }
            (EditorMode::VisualBlock, Register::Chars(chars)) if !chars.contains('\n') => {
                let start: Position = self.selection_start(cursor);
                for (from, to) in self.selected_ranges(cursor) {
                    // NOTE: A line too short to have any of the block on it is left alone.
                    if self.text.slice(from, to).is_empty() {
                        continue;
                    }
                    self.take(from, to);
                    self.put(from, &chars);
                }
                cursor.go(start);
            }
            (_, Register::Lines(lines)) => {
                let start: Position = self.remove_selection(cursor);
                self.put(start, &format!("\n{}\n", lines.join("\n")));
                cursor.go(Position::new(start.line + 1, 0));
            }
            _ => {
                let start: Position = self.remove_selection(cursor);
                cursor.go(start);
                self.paste(cursor, false);
            }
        }
    }

    /// Take out the lines from the first to the last.
    fn remove_lines(&mut self, first: usize, last: usize) {
        let end: usize = self.text.len() - 1;
        let last: usize = last.min(end);

        if last < end {
            self.take(Position::new(first, 0), Position::new(last + 1, 0));
        } else if first > 0 {
            let columns: usize = self.text.columns(first - 1);
            self.take(
                Position::new(first - 1, columns),
                Position::new(last, self.text.columns(last)),
            );
        } else {
            self.take(
                Position::new(0, 0),
                Position::new(last, self.text.columns(last)),
            );
        }
    }

    /// Put what the cursor yanked last back after it, or before it.
    fn paste(&mut self, cursor: &mut Cursor, after: bool) {
        let register: Register = match cursor.register.clone() {
            Some(register) => register,
            None => return,
        };
        let at: Position = cursor.at;
        let columns: usize = self.text.columns(at.line);
        let column: usize = match after {
            true => (at.column + 1).min(columns),
            false => at.column,
        };

        match register {
            Register::Chars(chars) => {
                let edit: Edit = self.put(Position::new(at.line, column), &chars);
                let end: Position = edit.end();
                cursor.go(Position::new(end.line, end.column.saturating_sub(1)));
            }
            Register::Lines(lines) if after => {
                self.put(
                    Position::new(at.line, columns),
                    &format!("\n{}", lines.join("\n")),
                );
                cursor.go(Position::new(at.line + 1, 0));
            }
            Register::Lines(lines) => {
                self.put(
                    Position::new(at.line, 0),
                    &format!("{}\n", lines.join("\n")),
                );
                cursor.go(Position::new(at.line, 0));
            }
            Register::Block(lines) => {
                for (offset, piece) in lines.iter().enumerate() {
                    let line: usize = at.line + offset;
                    if line >= self.text.len() {
                        let end: Position = self.text.end();
                        self.put(end, "\n");
                    }
                    let columns: usize = self.text.columns(line);
                    if columns < column {
                        self.put(Position::new(line, columns), &" ".repeat(column - columns));
                    }
                    self.put(Position::new(line, column), piece);
                }
                cursor.go(Position::new(at.line, column));
            }
        }
    }
}

mod open_error {
    //! An error opening a file to be edited.

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::io::{Error as IOError, ErrorKind};

    /// An error opening a file to be edited.
    #[derive(Debug)]
    pub enum OpenError {
        /// An error opening the file.
        Open(IOError),
        /// An error reading the file.
        Read(IOError),
    }

    impl Display for OpenError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::Open(error) => write!(formatter, "Failed to open the file: {}", error),
                Self::Read(error) if error.kind() == ErrorKind::InvalidData => {
                    write!(formatter, "Failed to read the file: it is not UTF-8")
                }
                Self::Read(error) => write!(formatter, "Failed to read the file: {}", error),
            }
        }
    }
}
pub use open_error::OpenError;

mod write_error {
    //! An error writing a file being edited.

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::io::Error as IOError;

    /// An error writing a file being edited.
    #[derive(Debug)]
    pub enum WriteError {
        /// The file could only be opened to be read.
        ReadOnly,
        /// An error writing the file.
        Write(IOError),
    }

    impl Display for WriteError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::ReadOnly => write!(formatter, "Failed to write the file: it is read only"),
                Self::Write(error) => write!(formatter, "Failed to write the file: {}", error),
            }
        }
    }
}
pub use write_error::WriteError;

#[cfg(test)]
mod tests {
    use std::fs::{read_to_string, remove_file, write};
    use std::path::PathBuf;

    use super::{Asked, Buffer, Scroll};
    use crate::editor::highlighting::Highlighting;

    use highlighter::{Language, Run};
    use insh_api::{EditorKey, EditorMode, Position};

    use test_case::test_case;
    use uuid::Uuid;

    /// The first cursor.
    const FIRST: Uuid = Uuid::from_u128(1);

    /// The second cursor.
    const SECOND: Uuid = Uuid::from_u128(2);

    /// A file which is removed again once it is done with.
    struct Scratch {
        /// Where it is.
        path: PathBuf,
    }

    impl Scratch {
        /// Return a new file with the contents.
        fn new(contents: &str) -> Self {
            let path: PathBuf = std::env::temp_dir().join(format!("inshd-{}", Uuid::new_v4()));
            write(&path, contents).unwrap();
            Self { path }
        }

        /// Return a new file with the contents, whose name ends with the extension.
        fn with_extension(contents: &str, extension: &str) -> Self {
            let path: PathBuf =
                std::env::temp_dir().join(format!("inshd-{}.{}", Uuid::new_v4(), extension));
            write(&path, contents).unwrap();
            Self { path }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = remove_file(&self.path);
        }
    }

    /// Return the file opened with a cursor in it.
    fn open(scratch: &Scratch) -> Buffer {
        let mut buffer: Buffer = Buffer::open(&scratch.path).unwrap();
        buffer.add(FIRST, Position::default());
        buffer
    }

    /// Press each of the characters for the cursor, with `\x1b` standing for escape and the other
    /// control characters for control and the letter they go with.
    fn typed(buffer: &mut Buffer, cursor: &Uuid, keys: &str) -> Option<Asked> {
        let mut asked: Option<Asked> = None;
        for character in keys.chars() {
            let key: EditorKey = match character {
                '\x1b' => EditorKey::Escape,
                '\n' => EditorKey::Enter,
                '\x16' => EditorKey::Control('v'),
                '\x12' => EditorKey::Control('r'),
                '\x05' => EditorKey::Control('e'),
                '\x19' => EditorKey::Control('y'),
                '\t' => EditorKey::Tab,
                '\x7f' => EditorKey::Backspace,
                character => EditorKey::Char(character),
            };
            asked = buffer.press(cursor, key);
        }
        asked
    }

    /// Return the text of the buffer as one string.
    fn text(buffer: &Buffer) -> String {
        buffer.text().lines().join("\n")
    }

    #[test_case("abc", "ix\x1b", "xabc"; "insert")]
    #[test_case("abc", "lix\x1b", "axbc"; "insert after moving right")]
    #[test_case("abc", "$ix\x1b", "abxc"; "insert at the end of the line")]
    #[test_case("abc", "$0ix\x1b", "xabc"; "insert at the start of the line")]
    #[test_case("abcd", "2lix\x1b", "abxcd"; "a count moves more than once")]
    #[test_case("a\nb\nc\nd", "3jix\x1b", "a\nb\nc\nxd"; "a count moves down")]
    #[test_case("ab\ncd", "lox\x1b", "ab\nx\ncd"; "open a line after")]
    #[test_case("ab\ncd", "jlOx\x1b", "ab\nx\ncd"; "open a line before")]
    #[test_case("ab", "ox\x1b", "ab\nx"; "open a line after the last")]
    #[test_case("ab", "Ox\x1b", "x\nab"; "open a line before the first")]
    #[test_case("abc", "l ", "a bc"; "a space")]
    #[test_case("abc", "vx", "bc"; "delete a selection")]
    #[test_case("ab\ncd\nef", "jVx", "ab\nef"; "delete a line")]
    #[test_case("ab\ncd\nef", "jjVx", "ab\ncd"; "delete the last line")]
    #[test_case("abc", "\x16x", "bc"; "delete a block")]
    #[test_case("abc", "vyp", "aabc"; "yank and paste")]
    #[test_case("ab\ncd", "Vyp", "ab\nab\ncd"; "yank a line and paste it below")]
    #[test_case("abc", "i\tx\x1b", "    xabc"; "a tab is four spaces")]
    #[test_case("abc", "li\n\x1b", "a\nbc"; "split a line")]
    #[test_case("abc", "$i\x7f\x1b", "ac"; "backspace")]
    #[test_case("ab\ncd", "ji\x7f\x1b", "abcd"; "backspace joins lines")]
    #[test_case("abcd", "v2lx", "d"; "a count moves a selection")]
    #[test_case("ab\ncd\nef", "Vjx", "ef"; "move a line selection")]
    #[test_case("abc\ndef", "\x16jlx", "c\nf"; "move a block selection")]
    #[test_case("abc", ":w\x1bix\x1b", "xabc"; "escape cancels a command")]
    #[test_case("abc", "lx", "ac"; "delete a character")]
    #[test_case("abcd", "l2x", "ad"; "a count deletes more than one character")]
    #[test_case("abc", "$5x", "ab"; "a count stops at the end of the line")]
    #[test_case("abc", "$xx", "a"; "delete the last character twice")]
    #[test_case("", "x", ""; "delete on an empty line")]
    #[test_case("ab\ncd", "xp", "ba\ncd"; "delete a character and paste it")]
    #[test_case("a\nb\nc", "Gix\x1b", "a\nb\nxc"; "go to the end of the file")]
    #[test_case("a\nb\nc", "Jix\x1b", "a\nb\nxc"; "go to the end of the file another way")]
    #[test_case("a\nb\nc", "jjggix\x1b", "xa\nb\nc"; "go to the beginning of the file")]
    #[test_case("a\nb\nc", "jjKix\x1b", "xa\nb\nc"; "go to the beginning of the file another way")]
    #[test_case("a\nb\nc", "jjgjix\x1b", "a\nb\nxc"; "a lone g does nothing")]
    #[test_case("ab\ncd\nef", "jVGx", "ab"; "select to the end of the file")]
    #[test_case("a\nb\nc", "2Gix\x1b", "a\nxb\nc"; "a count goes to the line")]
    #[test_case("a\nb\nc", "jj1Jix\x1b", "xa\nb\nc"; "a count goes to the line another way")]
    #[test_case("a\nb\nc", "9Gix\x1b", "a\nb\nxc"; "a count past the end goes to the last line")]
    #[test_case("a\nb\nc", "V2Gx", "c"; "select to a line")]
    #[test_case("ab\ncd\nef", "jdd", "ab\nef"; "delete the line")]
    #[test_case("ab\ncd\nef", "jjdd", "ab\ncd"; "delete the last line with dd")]
    #[test_case("ab", "dd", ""; "delete the only line")]
    #[test_case("a\nb\nc\nd", "2dd", "c\nd"; "a count deletes more than one line")]
    #[test_case("a\nb", "5dd", ""; "a count stops at the end of the file")]
    #[test_case("a\nb\nc", "ddp", "b\na\nc"; "delete a line and paste it below")]
    #[test_case("abc", "lvyP", "abbc"; "yank and paste before")]
    #[test_case("abc", "$xP", "acb"; "delete the last character and paste it before")]
    #[test_case("", "ix\x1bvxP", "x"; "paste before on an empty line")]
    #[test_case("a\nb\nc", "jyyP", "a\nb\nb\nc"; "yank a line and paste it above")]
    #[test_case("a\nb\nc", "jjddP", "a\nc\nb"; "delete the last line and paste it above")]
    #[test_case("a\nb", "yjGP", "a\na\nb\nb"; "paste lines above")]
    #[test_case("abc\ndef", "l\x16jy0P", "babc\nedef"; "paste a block before")]
    #[test_case("abc", "P", "abc"; "paste before with nothing yanked")]
    #[test_case("a\nb\nc", "dj", "c"; "delete down")]
    #[test_case("a\nb\nc", "jdk", "c"; "delete up")]
    #[test_case("a\nb\nc", "jdG", "a"; "delete to the end of the file")]
    #[test_case("a\nb\nc", "jjdgg", ""; "delete to the beginning of the file")]
    #[test_case("a\nb\nc\nd", "d2G", "c\nd"; "delete to a line")]
    #[test_case("abcd", "ld$", "a"; "delete to the end of the line")]
    #[test_case("abcd", "$d0", "d"; "delete to the beginning of the line")]
    #[test_case("abcd", "$dh", "abd"; "delete left")]
    #[test_case("abcd", "dl", "bcd"; "delete right")]
    #[test_case("abcd", "$dl", "abc"; "delete right on the last character")]
    #[test_case("abcd", "d2l", "cd"; "a count after delete")]
    #[test_case("abcd", "2dl", "cd"; "a count before delete")]
    #[test_case("a\nb\nc\nd\ne\nf", "2d2j", "f"; "counts before and after delete multiply")]
    #[test_case("abc", "dxix\x1b", "xabc"; "a key which does not move cancels delete")]
    #[test_case("abc", "d\x1bx", "bc"; "escape cancels delete")]
    #[test_case("a\nb\nc", "yyp", "a\na\nb\nc"; "yank the line")]
    #[test_case("a\nb\nc", "2yyp", "a\na\nb\nb\nc"; "a count yanks more than one line")]
    #[test_case("a\nb", "5yyp", "a\na\nb\nb"; "a count stops yanking at the end of the file")]
    #[test_case("a\nb\nc", "yjGp", "a\nb\nc\na\nb"; "yank down")]
    #[test_case("a\nb\nc", "jykp", "a\na\nb\nb\nc"; "yank up")]
    #[test_case("a\nb\nc", "jyGp", "a\nb\nb\nc\nc"; "yank to the end of the file")]
    #[test_case("a\nb\nc", "jjyggp", "a\na\nb\nc\nb\nc"; "yank to the beginning of the file")]
    #[test_case("abcd", "ly$$p", "abcdbcd"; "yank to the end of the line")]
    #[test_case("abcd", "$y0p", "aabcbcd"; "yank to the beginning of the line")]
    #[test_case("abcd", "$yhp", "abccd"; "yank left")]
    #[test_case("abcd", "y2l$p", "abcdab"; "a count after yank")]
    #[test_case("abcd", "2ylp", "aabbcd"; "a count before yank")]
    #[test_case("abc", "yxix\x1b", "xabc"; "a key which does not move cancels yank")]
    #[test_case("abc", "yyu", "abc"; "yanking is not a change")]
    #[test_case("foo bar baz", "dw", "bar baz"; "delete a word")]
    #[test_case("foo bar baz", "d2w", "baz"; "delete words")]
    #[test_case("a b c", "2dw", "c"; "a count before deleting words")]
    #[test_case("foo.bar baz", "dW", "baz"; "delete a big word")]
    #[test_case("foo bar\nbaz", "wdw", "foo \nbaz"; "delete the last word of a line")]
    #[test_case("foo  \nbar", "dw", "\nbar"; "delete the last word of a line and the blanks after it")]
    #[test_case("foo\nbar", "d2w", ""; "delete words across lines")]
    #[test_case("a\n\nb", "jdw", "a\nb"; "delete a word on an empty line")]
    #[test_case("ab", "$dw", "a"; "delete a word on the last character")]
    #[test_case("foo bar", "de", " bar"; "delete to the end of a word")]
    #[test_case("foo.bar baz", "dE", " baz"; "delete to the end of a big word")]
    #[test_case("ab", "$de", "a"; "delete to the end of a word on the last character")]
    #[test_case("foo bar", "$db", "foo r"; "delete back a word")]
    #[test_case("foo bar", "wdb", "bar"; "delete back to the start of the line")]
    #[test_case("foo.bar", "$dB", "r"; "delete back a big word")]
    #[test_case("a foo\nbar", "jdb", "a \nbar"; "delete back a word onto the line before")]
    #[test_case("  foo\nbar", "jdb", "bar"; "delete back a word onto the start of the line before")]
    #[test_case("foo bar", "yw$p", "foo barfoo "; "yank a word")]
    #[test_case("foo bar", "ye$p", "foo barfoo"; "yank to the end of a word")]
    #[test_case("ab cd", "vwx", "d"; "select a word")]
    #[test_case("abc", "ax\x1b", "axbc"; "insert after the cursor")]
    #[test_case("abc", "$ax\x1b", "abcx"; "insert after the last character")]
    #[test_case("", "ax\x1b", "x"; "insert after on an empty line")]
    #[test_case("abc\nd", "Ax\x1b", "abcx\nd"; "insert at the end of the line")]
    #[test_case("abc", "Axy\x1bu", "abc"; "undo what was inserted at the end of the line")]
    #[test_case("abc", "lrx", "axc"; "replace a character")]
    #[test_case("abcd", "l2rx", "axxd"; "a count replaces more than one character")]
    #[test_case("abc", "l3rx", "abc"; "a count past the end of the line replaces nothing")]
    #[test_case("abc", "r5", "5bc"; "replace with a digit")]
    #[test_case("abc", "r\x1bx", "bc"; "escape cancels replace")]
    #[test_case("", "rx", ""; "replace on an empty line")]
    #[test_case("abc", "2rxu", "abc"; "undo a replace")]
    #[test_case("abc", "xu", "abc"; "undo")]
    #[test_case("abc", "xxu", "bc"; "undo takes back one change")]
    #[test_case("abc", "xxuu", "abc"; "undo twice")]
    #[test_case("abc", "xx2u", "abc"; "a count undoes more than once")]
    #[test_case("abc", "u", "abc"; "undo with nothing to undo")]
    #[test_case("abc", "ixy\x1bu", "abc"; "undo everything typed in insert mode")]
    #[test_case("abc", "ix\x1biy\x1bu", "xabc"; "undo what was typed the last time in insert mode")]
    #[test_case("abc", "oxy\x1bu", "abc"; "undo opening a line along with what was typed")]
    #[test_case("abc", "i\x7f\x7fx\x1bu", "abc"; "undo backspacing")]
    #[test_case("a\nb\nc", "ddpu", "b\nc"; "undo a paste")]
    #[test_case("a\nb\nc", "ddPu", "b\nc"; "undo a paste before")]
    #[test_case("a\nb\nc", "jddu", "a\nb\nc"; "undo deleting a line")]
    #[test_case("a\nb\nc", "Gddu", "a\nb\nc"; "undo deleting the last line")]
    #[test_case("abc\ndef", "\x16jlxu", "abc\ndef"; "undo deleting a block")]
    #[test_case("abc", "vd", "bc"; "delete a selection with d")]
    #[test_case("ab\ncd\nef", "jVd", "ab\nef"; "delete a line with d")]
    #[test_case("abc\ndef", "\x16jld", "c\nf"; "delete a block with d")]
    #[test_case("abc", ":wx\x7f\x7f\x7fx", "bc"; "backspacing past the start cancels a command")]
    #[test_case("abc", "/b\x1bx", "bc"; "escape cancels a search")]
    #[test_case("abc", "/b\x7f\x7fx", "bc"; "backspacing past the start cancels a search")]
    #[test_case("abc", "xu\x12", "bc"; "redo")]
    #[test_case("abc", "xxuu\x12", "bc"; "redo makes one change again")]
    #[test_case("abc", "xxuu2\x12", "c"; "a count redoes more than once")]
    #[test_case("abc", "x\x12", "bc"; "redo with nothing to redo")]
    #[test_case("abc", "ixy\x1bu\x12", "xyabc"; "redo everything typed in insert mode")]
    #[test_case("abc", "xuix\x1b\x12", "xabc"; "a change after undoing leaves nothing to redo")]
    #[test_case("ABC", "vlu", "abC"; "lowercase a selection")]
    #[test_case("abc", "vlU", "ABc"; "uppercase a selection")]
    #[test_case("ab\ncd\nef", "jVjU", "ab\nCD\nEF"; "uppercase lines")]
    #[test_case("abc\ndef", "l\x16jU", "aBc\ndEf"; "uppercase a block")]
    #[test_case("abc", "vlUu", "abc"; "undo uppercasing")]
    #[test_case("abc", "vlUx", "Bc"; "uppercasing goes back to normal mode")]
    #[test_case("ab\ncd", "vjrx", "xx\nxd"; "replace a selection")]
    #[test_case("ab\ncd", "Vjrx", "xx\nxx"; "replace lines")]
    #[test_case("abc\ndef", "l\x16jr5", "a5c\nd5f"; "replace a block with a digit")]
    #[test_case("abc", "vlr\x1bx", "c"; "escape cancels replacing a selection")]
    #[test_case("abc", "vlrxu", "abc"; "undo replacing a selection")]
    #[test_case("abc def", "vey$vp", "abc deabc"; "paste over a selection")]
    #[test_case("abc def", "veywvep", "abc abc"; "paste over a word")]
    #[test_case("abc", "vypvp", "aabc"; "pasting over a selection keeps what was yanked")]
    #[test_case("abc", "vp", "abc"; "paste over a selection with nothing yanked")]
    #[test_case("a\nb\nc", "yyjVjp", "a\na"; "paste lines over lines")]
    #[test_case("ab\ncd", "vyjVp", "ab\na"; "paste characters over a line")]
    #[test_case("abc\nd", "yyjvp", "abc\n\nabc\n"; "paste a line over characters")]
    #[test_case("abc\ndef", "vy\x16jlp", "ac\naf"; "paste characters over each line of a block")]
    #[test_case("ab\n\ncd", "vy\x16jjp", "ab\n\nad"; "a line too short for the block is left alone")]
    #[test_case("abc\ndef", "\x16jy$\x16jp", "aba\nded"; "paste a block over a block")]
    #[test_case("ab\ncd", "vjpu", "ab\ncd"; "undo pasting over a selection")]
    #[test_case("ab\ncd\nef", "vjVx", "ef"; "switch from visual to visual line")]
    #[test_case("abc\ndef", "vjl\x16x", "c\nf"; "switch from visual to visual block")]
    #[test_case("ab\ncd\nef", "Vjvx", "d\nef"; "switch from visual line to visual")]
    #[test_case("abc\ndef", "lVj\x16x", "ac\ndf"; "switch from visual line to visual block")]
    #[test_case("abc\ndef", "\x16jlvx", "f"; "switch from visual block to visual")]
    #[test_case("ab\ncd\nef", "\x16jVx", "ef"; "switch from visual block to visual line")]
    fn test_keys(contents: &str, keys: &str, expected: &str) {
        let scratch: Scratch = Scratch::new(contents);
        let mut buffer: Buffer = open(&scratch);

        typed(&mut buffer, &FIRST, keys);

        assert_eq!(text(&buffer), expected);
    }

    #[test]
    fn test_a_cursor_moves_along_with_what_another_types_before_it() {
        let scratch: Scratch = Scratch::new("abc\ndef");
        let mut buffer: Buffer = open(&scratch);
        buffer.add(SECOND, Position::default());
        typed(&mut buffer, &SECOND, "jll");

        typed(&mut buffer, &FIRST, "ixy");

        assert_eq!(text(&buffer), "xyabc\ndef");
        assert_eq!(buffer.cursor(&SECOND).unwrap().at, Position::new(1, 2));
        assert_eq!(buffer.cursor(&FIRST).unwrap().mode, EditorMode::Insert);
        assert_eq!(buffer.cursor(&SECOND).unwrap().mode, EditorMode::Normal);
    }

    #[test]
    fn test_a_cursor_on_a_line_another_deletes_stays_in_the_text() {
        let scratch: Scratch = Scratch::new("ab\ncd");
        let mut buffer: Buffer = open(&scratch);
        buffer.add(SECOND, Position::default());
        typed(&mut buffer, &SECOND, "jl");

        typed(&mut buffer, &FIRST, "jVx");

        assert_eq!(text(&buffer), "ab");
        assert_eq!(buffer.cursor(&SECOND).unwrap().at, Position::new(0, 1));
    }

    #[test]
    fn test_a_cursor_on_a_line_another_opens_one_before_moves_down() {
        let scratch: Scratch = Scratch::new("ab\ncd");
        let mut buffer: Buffer = open(&scratch);
        buffer.add(SECOND, Position::default());
        typed(&mut buffer, &SECOND, "l");

        typed(&mut buffer, &FIRST, "O");

        assert_eq!(text(&buffer), "\nab\ncd");
        assert_eq!(buffer.cursor(&FIRST).unwrap().at, Position::new(0, 0));
        assert_eq!(buffer.cursor(&FIRST).unwrap().mode, EditorMode::Insert);
        assert_eq!(buffer.cursor(&SECOND).unwrap().at, Position::new(1, 1));
    }

    #[test]
    fn test_undo_takes_back_the_last_change_whichever_cursor_made_it() {
        let scratch: Scratch = Scratch::new("abc\ndef");
        let mut buffer: Buffer = open(&scratch);
        buffer.add(SECOND, Position::default());
        typed(&mut buffer, &FIRST, "x");
        typed(&mut buffer, &SECOND, "jx");

        typed(&mut buffer, &FIRST, "u");

        assert_eq!(text(&buffer), "bc\ndef");
        assert_eq!(buffer.cursor(&FIRST).unwrap().at, Position::new(1, 0));
    }

    #[test]
    fn test_typing_is_one_change_until_another_cursor_makes_one() {
        let scratch: Scratch = Scratch::new("abc\ndef");
        let mut buffer: Buffer = open(&scratch);
        buffer.add(SECOND, Position::default());
        typed(&mut buffer, &FIRST, "ix");
        typed(&mut buffer, &SECOND, "jx");
        typed(&mut buffer, &FIRST, "y\x1b");

        typed(&mut buffer, &FIRST, "u");

        assert_eq!(text(&buffer), "xabc\nef");
    }

    #[test_case("lxu", Position::new(0, 1); "undo goes to where the change was")]
    #[test_case("lxhu\x12", Position::new(0, 1); "redo goes to where the change was")]
    fn test_undo_and_redo_move_the_cursor(keys: &str, expected: Position) {
        let scratch: Scratch = Scratch::new("abc");
        let mut buffer: Buffer = open(&scratch);

        typed(&mut buffer, &FIRST, keys);

        assert_eq!(buffer.cursor(&FIRST).unwrap().at, expected);
    }

    #[test_case("l2rx", Position::new(0, 2); "replacing goes to the last character replaced")]
    #[test_case("lax", Position::new(0, 3); "inserting after goes on after what was inserted")]
    #[test_case("lAx", Position::new(0, 4); "inserting at the end of the line goes on after it")]
    fn test_editing_moves_the_cursor(keys: &str, expected: Position) {
        let scratch: Scratch = Scratch::new("abc");
        let mut buffer: Buffer = open(&scratch);

        typed(&mut buffer, &FIRST, keys);

        assert_eq!(buffer.cursor(&FIRST).unwrap().at, expected);
    }

    #[test_case("lyy", Position::new(0, 1); "yanking the line stays put")]
    #[test_case("lyj", Position::new(0, 1); "yanking down stays put")]
    #[test_case("jlyk", Position::new(0, 1); "yanking up goes up")]
    #[test_case("$yh", Position::new(0, 1); "yanking left goes left")]
    #[test_case("ly$", Position::new(0, 1); "yanking to the end of the line stays put")]
    fn test_yanking_moves_the_cursor(keys: &str, expected: Position) {
        let scratch: Scratch = Scratch::new("abc\ndef");
        let mut buffer: Buffer = open(&scratch);

        typed(&mut buffer, &FIRST, keys);

        assert_eq!(buffer.cursor(&FIRST).unwrap().at, expected);
    }

    #[test_case("foo.bar baz\n  qux", "w", Position::new(0, 3); "a word")]
    #[test_case("foo.bar baz\n  qux", "2w", Position::new(0, 4); "a count of words")]
    #[test_case("foo.bar baz\n  qux", "4w", Position::new(1, 2); "a word on the next line")]
    #[test_case("foo.bar baz\n  qux", "W", Position::new(0, 8); "a big word")]
    #[test_case("foo.bar baz\n  qux", "G$w", Position::new(1, 4); "a word at the end of the file")]
    #[test_case("foo.bar baz\n  qux", "e", Position::new(0, 2); "the end of a word")]
    #[test_case("foo.bar baz\n  qux", "ee", Position::new(0, 3); "the end of the next word")]
    #[test_case("foo.bar baz\n  qux", "E", Position::new(0, 6); "the end of a big word")]
    #[test_case("foo.bar baz\n  qux", "5e", Position::new(1, 4); "the end of a word on the next line")]
    #[test_case("foo.bar baz\n  qux", "G$e", Position::new(1, 4); "the end of a word at the end of the file")]
    #[test_case("foo.bar baz\n  qux", "$b", Position::new(0, 8); "back a word")]
    #[test_case("foo.bar baz\n  qux", "$3b", Position::new(0, 3); "back a count of words")]
    #[test_case("foo.bar baz\n  qux", "$2B", Position::new(0, 0); "back a big word")]
    #[test_case("foo.bar baz\n  qux", "jb", Position::new(0, 8); "back a word on the line before")]
    #[test_case("foo.bar baz\n  qux", "b", Position::new(0, 0); "back a word at the start of the file")]
    #[test_case("a\n\nb", "w", Position::new(1, 0); "a word stops on an empty line")]
    #[test_case("a\n\nb", "ww", Position::new(2, 0); "a word goes on past an empty line")]
    #[test_case("a\n\nb", "Gb", Position::new(1, 0); "back a word stops on an empty line")]
    #[test_case("a\n\nb", "e", Position::new(2, 0); "the end of a word goes past an empty line")]
    fn test_moving_by_words(contents: &str, keys: &str, expected: Position) {
        let scratch: Scratch = Scratch::new(contents);
        let mut buffer: Buffer = open(&scratch);

        typed(&mut buffer, &FIRST, keys);

        assert_eq!(buffer.cursor(&FIRST).unwrap().at, expected);
    }

    #[test_case("abc", "vlyP", Position::new(0, 1); "pasting characters before goes to the last of them")]
    #[test_case("a\nb\nc", "jjyyggP", Position::new(0, 0); "pasting lines above goes to the first of them")]
    #[test_case("a\nb", "jlyyP", Position::new(1, 0); "pasting a line above the one the cursor is on")]
    #[test_case("abc\ndef", "l\x16jy$P", Position::new(0, 2); "pasting a block before goes to its start")]
    fn test_pasting_before_moves_the_cursor(contents: &str, keys: &str, expected: Position) {
        let scratch: Scratch = Scratch::new(contents);
        let mut buffer: Buffer = open(&scratch);

        typed(&mut buffer, &FIRST, keys);

        assert_eq!(buffer.cursor(&FIRST).unwrap().at, expected);
    }

    #[test_case("xu", false; "undoing every change")]
    #[test_case("x:w\nu", true; "undoing a change which was written")]
    #[test_case("x:w\nu\x12", false; "redoing back to what was written")]
    #[test_case("x:w\nuix\x1bu", true; "undoing once what was written cannot be gotten back to")]
    fn test_modified(keys: &str, expected: bool) {
        let scratch: Scratch = Scratch::new("abc");
        let mut buffer: Buffer = open(&scratch);

        typed(&mut buffer, &FIRST, keys);

        assert_eq!(buffer.modified(), expected);
    }

    #[test]
    fn test_writing_keeps_the_newline_at_the_end() {
        let scratch: Scratch = Scratch::new("abc\n");
        let mut buffer: Buffer = open(&scratch);

        typed(&mut buffer, &FIRST, "vx:w\n");

        assert_eq!(read_to_string(&scratch.path).unwrap(), "bc\n");
        assert!(!buffer.modified());
    }

    #[test]
    fn test_a_file_which_does_not_exist_is_made_once_it_is_written() {
        let scratch: Scratch = Scratch::new("");
        remove_file(&scratch.path).unwrap();
        let mut buffer: Buffer = open(&scratch);

        typed(&mut buffer, &FIRST, "iabc\x1b");
        assert!(!scratch.path.exists());

        typed(&mut buffer, &FIRST, ":w\n");
        assert_eq!(read_to_string(&scratch.path).unwrap(), "abc\n");
    }

    #[test]
    fn test_a_new_file_which_nothing_was_typed_in_is_written_empty() {
        let scratch: Scratch = Scratch::new("");
        remove_file(&scratch.path).unwrap();
        let mut buffer: Buffer = open(&scratch);

        typed(&mut buffer, &FIRST, ":w\n");

        assert_eq!(read_to_string(&scratch.path).unwrap(), "");
    }

    #[test]
    fn test_a_new_file_made_by_something_else_is_written_over() {
        let scratch: Scratch = Scratch::new("");
        remove_file(&scratch.path).unwrap();
        let mut buffer: Buffer = open(&scratch);
        write(&scratch.path, "something else wrote this\n").unwrap();

        typed(&mut buffer, &FIRST, "iabc\x1b:w\n");

        assert_eq!(read_to_string(&scratch.path).unwrap(), "abc\n");
    }

    #[test_case(":q\n", Some(Asked::Quit); "quit")]
    #[test_case("vx:q\n", None; "quit is refused with unwritten changes")]
    #[test_case("vx:q!\n", Some(Asked::Quit); "quit even with unwritten changes")]
    #[test_case("vx:wq\n", Some(Asked::Quit); "write and quit")]
    #[test_case("vY", Some(Asked::Clipboard(String::from("a"))); "yank to the clipboard")]
    #[test_case("l:vim\n", Some(Asked::Vim(Position::new(0, 1))); "open in vim")]
    #[test_case("vx:vim\n", None; "open in vim is refused with unwritten changes")]
    #[test_case("vx:vim!\n", Some(Asked::Vim(Position::new(0, 0))); "open in vim even with unwritten changes")]
    #[test_case(":nope\n", None; "an unknown command")]
    #[test_case("\x05", Some(Asked::Scroll(Scroll::Down(1))); "scroll down")]
    #[test_case("3\x05", Some(Asked::Scroll(Scroll::Down(3))); "a count scrolls down more than one line")]
    #[test_case("\x19", Some(Asked::Scroll(Scroll::Up(1))); "scroll up")]
    #[test_case("2\x19", Some(Asked::Scroll(Scroll::Up(2))); "a count scrolls up more than one line")]
    #[test_case("i\x05", Some(Asked::Scroll(Scroll::Down(1))); "scroll down in insert mode")]
    #[test_case("v2\x19", Some(Asked::Scroll(Scroll::Up(2))); "scroll up in visual mode")]
    fn test_asked(keys: &str, expected: Option<Asked>) {
        let scratch: Scratch = Scratch::new("abc");
        let mut buffer: Buffer = open(&scratch);

        assert_eq!(typed(&mut buffer, &FIRST, keys), expected);
    }

    #[test_case("a b a b", "/b\n", Position::new(0, 2); "search")]
    #[test_case("a b a b", "/bx\x7f\n", Position::new(0, 2); "backspace in a search")]
    #[test_case("a b a b", "/b\nn", Position::new(0, 6); "next hit")]
    #[test_case("a b a b", "/b\nnn", Position::new(0, 2); "next hit wraps around")]
    #[test_case("a b a b", "/b\n2n", Position::new(0, 2); "a count goes to more than one hit")]
    #[test_case("a b a b", "/b\nN", Position::new(0, 6); "previous hit wraps around")]
    #[test_case("a b\na b", "G$/a\nN", Position::new(1, 0); "previous hit")]
    #[test_case("a b a b", "/b\n/\n", Position::new(0, 6); "an empty search searches for the last")]
    #[test_case("a b a b", "/x\n", Position::new(0, 0); "a search with no hit stays put")]
    #[test_case("a b a b", "n", Position::new(0, 0); "next hit with nothing searched for stays put")]
    fn test_searching(contents: &str, keys: &str, expected: Position) {
        let scratch: Scratch = Scratch::new(contents);
        let mut buffer: Buffer = open(&scratch);

        typed(&mut buffer, &FIRST, keys);

        assert_eq!(buffer.cursor(&FIRST).unwrap().at, expected);
        assert_eq!(buffer.cursor(&FIRST).unwrap().mode, EditorMode::Normal);
    }

    #[test_case("/x\n", Some("Search not found"); "no hit")]
    #[test_case("n", Some("No previous search"); "nothing searched for")]
    #[test_case("/b\nn", Some("Search hit the bottom, continuing at the top"); "wrapping forward")]
    #[test_case("/b\n", None; "not wrapping")]
    #[test_case("/b\nN", Some("Search hit the top, continuing at the bottom"); "wrapping back")]
    #[test_case("N", Some("No previous search"); "nothing searched for going back")]
    fn test_search_messages(keys: &str, expected: Option<&str>) {
        let scratch: Scratch = Scratch::new("abc");
        let mut buffer: Buffer = open(&scratch);

        typed(&mut buffer, &FIRST, keys);

        assert_eq!(buffer.cursor(&FIRST).unwrap().message.as_deref(), expected);
    }

    #[test_case("abc", "i", "x\ry", "x\nyabc"; "a carriage return is a new line")]
    #[test_case("abc", "a", "xy", "axybc"; "after the cursor")]
    #[test_case("abc", "", "xy", "abc"; "nothing is pasted outside of insert mode")]
    fn test_pasting_in(contents: &str, keys: &str, pasted: &str, expected: &str) {
        let scratch: Scratch = Scratch::new(contents);
        let mut buffer: Buffer = open(&scratch);
        typed(&mut buffer, &FIRST, keys);

        buffer.paste_in(&FIRST, pasted);

        assert_eq!(text(&buffer), expected);
    }

    #[test]
    fn test_pasting_in_goes_on_after_the_text_and_is_undone_with_what_was_typed() {
        let scratch: Scratch = Scratch::new("abc");
        let mut buffer: Buffer = open(&scratch);
        typed(&mut buffer, &FIRST, "iw");

        buffer.paste_in(&FIRST, "x\ny");
        assert_eq!(buffer.cursor(&FIRST).unwrap().at, Position::new(1, 1));
        typed(&mut buffer, &FIRST, "z\x1bu");

        assert_eq!(text(&buffer), "abc");
    }

    #[test_case(Position::new(1, 2), Position::new(1, 2); "a position in the text")]
    #[test_case(Position::new(0, 9), Position::new(0, 2); "past the end of a line")]
    #[test_case(Position::new(9, 0), Position::new(1, 2); "past the last line")]
    fn test_a_cursor_is_put_as_close_to_where_it_is_asked_for_as_it_can_be(
        at: Position,
        expected: Position,
    ) {
        let scratch: Scratch = Scratch::new("abc\ndef");
        let mut buffer: Buffer = Buffer::open(&scratch.path).unwrap();

        buffer.add(FIRST, at);

        assert_eq!(buffer.cursor(&FIRST).unwrap().at, expected);
    }

    #[test]
    fn test_the_colors_keep_up_with_every_change() {
        let scratch: Scratch =
            Scratch::with_extension("fn a() {}\nfn b() {\n    let x = 1;\n}\n", "rs");
        let mut buffer: Buffer = open(&scratch);
        buffer.runs(0..buffer.text().len());

        for keys in [
            "O/*\x1b",
            "jjA */\x1b",
            "u",
            "\x12",
            "ggdd",
            "Gp",
            "kx",
            "uuu",
            "ggyGPx",
            "gglr/",
            "ggl2r*",
            "jvjr\"",
            "u",
            "ggVjr/",
            "ggyyjVp",
            "gg\x16jlr-",
            "ggyiwjviwp",
        ] {
            typed(&mut buffer, &FIRST, keys);

            let lines: Vec<String> = buffer.text().lines().to_vec();
            let kept: Vec<Vec<Run>> = buffer.runs(0..lines.len());
            let afresh: Vec<Vec<Run>> = Highlighting::builder()
                .language(Some(Language::Rust))
                .build()
                .runs(&lines, 0..lines.len());
            assert_eq!(kept, afresh, "after {:?}", keys);
        }
    }
}
