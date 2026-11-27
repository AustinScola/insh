//! The changes made to a file being edited, which undoing and redoing go back and forth through.

use super::text::Edit;

use insh_api::Position;

use typed_builder::TypedBuilder;
use uuid::Uuid;

/// The changes made to a file being edited.
///
/// There is the one history for the file rather than one for each cursor in it, so undoing takes
/// back the last change made by whichever cursor made it. That is what keeps every change which is
/// undone or redone lined up with the text it was made to.
#[derive(Debug, TypedBuilder)]
pub struct History {
    /// The changes which have been made, the last made last.
    #[builder(setter(skip), default)]
    done: Vec<Change>,
    /// The changes which have been undone, the last undone last.
    #[builder(setter(skip), default)]
    undone: Vec<Change>,
    /// The edits made by the key being handled.
    #[builder(setter(skip), default)]
    pending: Vec<Step>,
    /// The cursor which is typing text in, whose last change is still being added to.
    #[builder(setter(skip), default)]
    open: Option<Uuid>,
    /// How many changes had been made when the file was last written, which is nothing once
    /// undoing and redoing can no longer get back to it.
    #[builder(setter(skip), default = Some(0))]
    written: Option<usize>,
}

impl History {
    /// Take note of an edit made by the key being handled, along with the text it put in or took
    /// out.
    pub fn record(&mut self, edit: Edit, text: String) {
        // NOTE: An edit which put in or took out nothing changed nothing worth undoing.
        if !text.is_empty() {
            self.pending.push(Step { edit, text });
        }
    }

    /// Take the edits made by the key pressed for the cursor as a change, or as more of its last
    /// change while it is typing text in.
    ///
    /// NOTE: Everything typed from going into insert mode until leaving it is the one change, the
    /// same as in vim, unless some other cursor changes the text in the meantime.
    pub fn commit(&mut self, cursor: Uuid, inserting: bool) {
        if !self.pending.is_empty() {
            let steps: Vec<Step> = std::mem::take(&mut self.pending);
            match self.done.last_mut() {
                Some(change) if self.open == Some(cursor) => change.steps.extend(steps),
                _ => {
                    if self
                        .written
                        .is_some_and(|written| written > self.done.len())
                    {
                        self.written = None;
                    }
                    self.undone.clear();
                    self.done.push(Change { steps });
                }
            }
            self.open = inserting.then_some(cursor);
        } else if !inserting && self.open == Some(cursor) {
            self.open = None;
        }
    }

    /// Return the last change made, taking it as undone.
    pub fn undo(&mut self) -> Option<Change> {
        self.open = None;
        let change: Change = self.done.pop()?;
        self.undone.push(change.clone());
        return Some(change);
    }

    /// Return the last change undone, taking it as made again.
    pub fn redo(&mut self) -> Option<Change> {
        self.open = None;
        let change: Change = self.undone.pop()?;
        self.done.push(change.clone());
        return Some(change);
    }

    /// Take note that the file was written.
    pub fn write(&mut self) {
        self.open = None;
        self.written = Some(self.done.len());
    }

    /// Return whether there are changes which have not been written.
    pub fn modified(&self) -> bool {
        return !self.pending.is_empty() || self.written != Some(self.done.len());
    }
}

/// The edits made with one change, which are undone and redone together.
#[derive(Debug, Clone)]
pub struct Change {
    /// The edits, the first made first.
    pub steps: Vec<Step>,
}

impl Change {
    /// Return where the change starts, which is where a cursor goes when it is undone or redone.
    pub fn start(&self) -> Option<Position> {
        return self.steps.first().map(|step| step.edit.start());
    }
}

/// One edit, along with the text it put in or took out, which is what it takes to make it again or
/// to take it back.
#[derive(Debug, Clone)]
pub struct Step {
    /// The edit.
    pub edit: Edit,
    /// The text it put in or took out.
    pub text: String,
}
