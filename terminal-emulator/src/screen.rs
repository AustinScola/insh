/*!
This module contains the [`Screen`] struct which is what the bytes a program writes mean to show.
*/

/// Contains the [`Props`] struct.
mod props {
    use rend::Size;

    use typed_builder::TypedBuilder;

    /// How many rows are kept after they have scrolled off the top of a screen.
    const SCROLLBACK: usize = 10_000;

    /// The properties of a screen.
    #[derive(TypedBuilder, Debug, Clone, Copy)]
    pub struct Props {
        /// The size of the screen.
        pub size: Size,
        /// How many rows are kept after they have scrolled off the top.
        #[builder(default = SCROLLBACK)]
        pub scrollback: usize,
    }
}
pub use props::Props;

/// Contains the [`Screen`] struct.
mod screen {
    use std::collections::{BTreeSet, VecDeque};
    use std::mem;
    use std::ops::Range;

    use super::Props;
    use crate::action::Action;
    use crate::character_set::CharacterSet;
    use crate::cursor::Cursor;
    use crate::grid::{Grid, Line};
    use crate::modes::Modes;

    use ansi::{
        ClearTabStops, ControlCharacter, ControlFunction, EraseInDisplay, EraseInLine, Mode,
        OperatingSystemCommand,
    };
    use rend::{Cell, Fabric, Location, Size, Style};

    use typed_builder::TypedBuilder;

    /// What there is to say about a screen apart from what is written on its rows.
    #[derive(TypedBuilder, Debug, Clone, PartialEq, Eq)]
    struct Status {
        /// Where the cursor is.
        cursor: Location,
        /// The ways the screen has been asked to behave.
        modes: Modes,
        /// What the program has asked the title to be.
        title: Option<String>,
    }

    /// What the bytes a program writes mean to show.
    #[derive(Debug, Clone)]
    pub struct Screen {
        /// The size of the screen.
        size: Size,
        /// The rows which are being written on.
        grid: Grid,
        /// The rows which are not being written on, while the other screen is.
        other: Option<Grid>,
        /// Where the program is writing and the way it is writing there.
        cursor: Cursor,
        /// The cursor which was put aside to be come back to.
        saved: Option<Cursor>,
        /// The rows which the text scrolls between.
        region: Range<usize>,
        /// The columns which the cursor moves on to when it tabs.
        tabs: BTreeSet<usize>,
        /// The ways the screen has been asked to behave.
        modes: Modes,
        /// What the characters a program writes stand for, one for each of the four slots a
        /// program designates a character set in.
        character_sets: [CharacterSet; 4],
        /// Which of the four slots the characters being written are read in.
        character_set: usize,
        /// What the program has asked the title to be.
        title: Option<String>,
        /// The rows which have scrolled off the top, the most recent of them last.
        scrollback: VecDeque<Line>,
        /// How many rows are kept after they have scrolled off the top.
        scrollback_length: usize,
        /// How many times the bell has rung since it was last asked about.
        bells: usize,
        /// How many rows back through what has scrolled off the top is being looked.
        looking_back: usize,
        /// What was last taken as having been sent, apart from the rows.
        sent: Status,
    }

    impl From<Props> for Screen {
        fn from(props: Props) -> Self {
            let size: Size = Self::at_least_one(props.size);
            let cursor: Cursor = Cursor::default();

            return Self {
                size,
                grid: Grid::blank(size),
                other: None,
                sent: Status::builder()
                    .cursor(cursor.at)
                    .modes(Modes::default())
                    .title(None)
                    .build(),
                cursor,
                saved: None,
                region: 0..size.rows,
                tabs: Self::default_tabs(size.columns),
                modes: Modes::default(),
                character_sets: [CharacterSet::default(); 4],
                character_set: 0,
                title: None,
                scrollback: VecDeque::new(),
                scrollback_length: props.scrollback,
                bells: 0,
                looking_back: 0,
            };
        }
    }

    impl Screen {
        /// How many columns apart the tab stops are until a program says otherwise.
        const TAB_WIDTH: usize = 8;

        /// Do what the bytes a program wrote say to do.
        pub fn feed(&mut self, actions: impl IntoIterator<Item = Action>) {
            for action in actions {
                self.perform(action);
            }
        }

        /// Do the one thing which the bytes a program wrote say to do.
        ///
        /// Anything a program does brings the screen back from whatever was being looked back
        /// through, the way a terminal does when something is written while you are scrolled up.
        pub fn perform(&mut self, action: Action) {
            if self.looking_back != 0 {
                self.stop_looking_back();
            }

            match action {
                Action::Print(character) => self.print(character),
                Action::Control(control) => self.control(control),
                Action::Function(function) => self.function(function),
                // NOTE: A sequence which is not a control function is one this does not know, so
                // there is nothing to be done about it. It is not thrown away any earlier than
                // here, so that it can be looked at while working out what is missing.
                Action::Sequence(_) => {}
            }
        }

        /// Return the size of the screen.
        pub fn size(&self) -> Size {
            return self.size;
        }

        /// Return where the cursor is.
        pub fn cursor(&self) -> Location {
            return self.cursor.at;
        }

        /// Return whether the cursor is shown.
        pub fn is_cursor_shown(&self) -> bool {
            return self.modes.contains(Modes::CURSOR_VISIBLE);
        }

        /// Return the ways the screen has been asked to behave.
        pub fn modes(&self) -> Modes {
            return self.modes;
        }

        /// Return what the program has asked the title to be.
        pub fn title(&self) -> Option<&str> {
            return self.title.as_deref();
        }

        /// Return the rows which have scrolled off the top, the most recent of them last.
        pub fn scrollback(&self) -> &VecDeque<Line> {
            return &self.scrollback;
        }

        /// Return which rows have been written on since they were last asked for, and take them as
        /// having been sent.
        ///
        /// This is what keeps a screen from having to be sent somewhere else whole every time a
        /// program writes one character on it.
        pub fn take_damage(&mut self) -> BTreeSet<usize> {
            return self.grid.take_damage();
        }

        /// Return whether anything apart from the rows has changed since this was last asked, and
        /// take it as having been sent.
        ///
        /// A program can move the cursor or change the way the screen behaves without writing on
        /// any row at all, which is something that the damage has nothing to say about.
        pub fn take_status_changed(&mut self) -> bool {
            let status: Status = Status::builder()
                .cursor(self.cursor.at)
                .modes(self.modes)
                .title(self.title.clone())
                .build();

            let changed: bool = status != self.sent;
            self.sent = status;
            return changed;
        }

        /// Return the given row of what is being looked at.
        ///
        /// Looking back through what has scrolled off the top shows the rows which went before
        /// the ones which are on the screen now, so that a row is whichever one is that far down
        /// from wherever the looking back has reached.
        pub fn line(&self, row: usize) -> &Line {
            let back: usize = self.looking_back;
            if back == 0 {
                return self.grid.line(row);
            }

            // The rows which have scrolled off come first, the most recent of them last.
            let from_end: usize = back.saturating_sub(row);
            if from_end > 0 && from_end <= self.scrollback.len() {
                return &self.scrollback[self.scrollback.len() - from_end];
            }

            return self.grid.line(row - back.min(row));
        }

        /// Return how far back through what has scrolled off the top is being looked.
        pub fn looking_back(&self) -> usize {
            return self.looking_back;
        }

        /// Look the given number of rows further back through what has scrolled off the top, or
        /// fewer rows back when that is negative, and return whether it moved at all.
        ///
        /// Looking back does not go past the oldest row which was kept, nor forward past the rows
        /// which are on the screen now.
        pub fn look_back(&mut self, rows: isize) -> bool {
            let was: usize = self.looking_back;

            self.looking_back = was.saturating_add_signed(rows).min(self.scrollback.len());

            if self.looking_back != was {
                self.grid.damage_all();
            }

            return self.looking_back != was;
        }

        /// Stop looking back through what has scrolled off the top.
        pub fn stop_looking_back(&mut self) {
            self.look_back(-isize::try_from(self.looking_back).unwrap_or(isize::MAX));
        }

        /// Return how many times the bell has rung, and forget that it did.
        pub fn take_bells(&mut self) -> usize {
            let bells: usize = self.bells;
            self.bells = 0;
            return bells;
        }

        /// Change the size of the screen.
        ///
        /// What is written stays in the row and column it was written in. Nothing is broken up and
        /// put back together to fit the new width, which is what a terminal does only if it goes
        /// to the trouble, and the rows which no longer fit scroll off the top.
        pub fn resize(&mut self, size: Size) {
            let size: Size = Self::at_least_one(size);
            if size == self.size {
                return;
            }

            let style: Style = Style::default();
            let scrolled: Vec<Line> = self.grid.resize(size, style, self.cursor.at.row);
            if let Some(other) = &mut self.other {
                // NOTE: The alternate screen has no history of its own, so whatever it loses is
                // gone. A program which uses it draws the whole of it again when it changes size.
                other.resize(size, style, other.size().rows);
            }

            // NOTE: The rows which scrolled off the top are kept the way the ones which scroll off
            // it while a program writes are, since a program wrote them just the same.
            let scrolled_off: usize = scrolled.len();
            for line in scrolled {
                self.remember(line);
            }

            self.tabs = Self::default_tabs(size.columns);
            self.region = 0..size.rows;
            self.size = size;

            // The cursor goes up along with the rows which scrolled off above it, so that it stays
            // on the row it was writing in.
            self.cursor.at.row = self
                .cursor
                .at
                .row
                .saturating_sub(scrolled_off)
                .min(size.rows - 1);
            self.cursor.at.column = self.cursor.at.column.min(size.columns - 1);
            self.cursor.wrapping = false;
        }

        /// Return a size which has at least one row and one column.
        ///
        /// A screen of no size at all has nowhere for the cursor to be, and every column of it
        /// would be both the first and past the last.
        fn at_least_one(size: Size) -> Size {
            return Size::new(size.rows.max(1), size.columns.max(1));
        }

        /// Return the columns which the cursor tabs to until a program says otherwise.
        fn default_tabs(columns: usize) -> BTreeSet<usize> {
            return (Self::TAB_WIDTH..columns)
                .step_by(Self::TAB_WIDTH)
                .collect();
        }

        /// Write the character where the cursor is.
        fn print(&mut self, character: char) {
            let character: char = self.character_sets[self.character_set].character(character);
            let cell: Cell = Cell::Char(character);
            let width: usize = cell.width();

            // A character written in no columns of its own belongs to the cluster before it, the
            // way a combining accent does.
            if width == 0 {
                self.combine(character);
                return;
            }

            // NOTE: A screen which has been asked not to wrap writes over the last column again
            // and again rather than going onto the row below.
            if self.cursor.wrapping {
                match self.modes.contains(Modes::AUTO_WRAP) {
                    true => self.wrap(),
                    false => self.cursor.wrapping = false,
                }
            }

            // A cluster which does not fit in what is left of the row goes onto the next one.
            if self.cursor.at.column + width > self.size.columns {
                if !self.modes.contains(Modes::AUTO_WRAP) {
                    return;
                }
                self.wrap();
            }

            let row: usize = self.cursor.at.row;
            let column: usize = self.cursor.at.column;
            let style: Style = self.cursor.style;

            if self.modes.contains(Modes::INSERT) {
                self.grid.line_mut(row).insert(column, width, style);
            }
            self.grid.line_mut(row).set(column, cell, style);

            match column + width >= self.size.columns {
                // There is no column past the last one for the cursor to be in, so it stays where
                // it is and the next cluster wraps.
                true => {
                    self.cursor.at.column = self.size.columns - 1;
                    self.cursor.wrapping = true;
                }
                false => self.cursor.at.column += width,
            }
        }

        /// Add the character onto the cluster which was written last.
        fn combine(&mut self, character: char) {
            let row: usize = self.cursor.at.row;

            let column: usize = match self.cursor.wrapping {
                true => self.cursor.at.column,
                false => match self.cursor.at.column {
                    0 => return,
                    column => column - 1,
                },
            };

            self.grid.line_mut(row).append(column, character);
        }

        /// Move to the first column of the next row, scrolling when there is no next row.
        fn wrap(&mut self) {
            self.cursor.wrapping = false;
            self.cursor.at.column = 0;
            self.down_a_row();
        }

        /// Do what the control says.
        fn control(&mut self, control: ControlCharacter) {
            match control {
                ControlCharacter::Bell => self.bells += 1,
                ControlCharacter::Backspace => self.back(),
                ControlCharacter::CharacterTabulation => self.tab(1),
                // NOTE: A terminal takes a vertical tab and a form feed as a line feed, which is
                // what the programs which send them expect of it.
                ControlCharacter::LineFeed
                | ControlCharacter::LineTabulation
                | ControlCharacter::FormFeed => {
                    self.cursor.wrapping = false;
                    self.down_a_row();
                    if self.modes.contains(Modes::NEW_LINE) {
                        self.cursor.at.column = 0;
                    }
                }
                ControlCharacter::CarriageReturn => {
                    self.cursor.wrapping = false;
                    self.cursor.at.column = 0;
                }
                ControlCharacter::LockingShiftZero => self.character_set = 0,
                ControlCharacter::LockingShiftOne => self.character_set = 1,
                _ => {}
            }
        }

        /// Move the cursor back a column, which it does not do past the first one.
        fn back(&mut self) {
            self.cursor.wrapping = false;
            self.cursor.at.column = self.cursor.at.column.saturating_sub(1);
        }

        /// Move the cursor on to the given number of tab stops along.
        fn tab(&mut self, count: usize) {
            self.cursor.wrapping = false;

            let last: usize = self.size.columns - 1;
            for _ in 0..count {
                self.cursor.at.column = self
                    .tabs
                    .range(self.cursor.at.column + 1..)
                    .next()
                    .copied()
                    .unwrap_or(last);
            }
        }

        /// Move the cursor back the given number of tab stops.
        fn back_tab(&mut self, count: usize) {
            self.cursor.wrapping = false;

            for _ in 0..count {
                self.cursor.at.column = self
                    .tabs
                    .range(..self.cursor.at.column)
                    .next_back()
                    .copied()
                    .unwrap_or(0);
            }
        }

        /// Move the cursor down a row, scrolling the text up when it is at the bottom of the rows
        /// it scrolls between.
        fn down_a_row(&mut self) {
            if self.cursor.at.row + 1 == self.region.end {
                self.scroll(1);
                return;
            }
            if self.cursor.at.row + 1 < self.size.rows {
                self.cursor.at.row += 1;
            }
        }

        /// Move the cursor up a row, scrolling the text down when it is at the top of the rows it
        /// scrolls between.
        fn up_a_row(&mut self) {
            if self.cursor.at.row == self.region.start {
                self.scroll(-1);
                return;
            }
            self.cursor.at.row = self.cursor.at.row.saturating_sub(1);
        }

        /// Move the rows the text scrolls between the given number of rows up, or down when that
        /// is negative, keeping whichever of them scroll off the top.
        fn scroll(&mut self, rows: isize) {
            let region: Range<usize> = self.region.clone();
            // NOTE: The rows which are made room for are brought in written the way the program is
            // writing, which is what a terminal which erases in the color it is writing on does.
            let off: Vec<Line> = self.grid.scroll(region, rows, self.cursor.style);

            for line in off {
                self.remember(line);
            }
        }

        /// Keep the row now that it has scrolled off the top, letting go of the oldest one when
        /// there are more of them than are kept.
        fn remember(&mut self, line: Line) {
            if self.scrollback_length == 0 {
                return;
            }

            self.scrollback.push_back(line);
            while self.scrollback.len() > self.scrollback_length {
                self.scrollback.pop_front();
            }
        }

        /// Move the cursor to the given row and column, which are counted from the top of the rows
        /// the text scrolls between when the screen has been asked to count them that way.
        fn move_to(&mut self, row: usize, column: usize) {
            self.cursor.wrapping = false;

            let (top, bottom): (usize, usize) = match self.modes.contains(Modes::ORIGIN) {
                true => (self.region.start, self.region.end - 1),
                false => (0, self.size.rows - 1),
            };

            self.cursor.at.row = (top + row).min(bottom);
            self.cursor.at.column = column.min(self.size.columns - 1);
        }

        /// Do what the control function says.
        fn function(&mut self, function: ControlFunction) {
            let count = |count: u16| usize::from(count).max(1);
            let at = |position: u16| usize::from(position).max(1) - 1;

            match function {
                ControlFunction::CursorUp(rows) => {
                    self.cursor.wrapping = false;
                    let top: usize = self.top();
                    self.cursor.at.row = self.cursor.at.row.saturating_sub(count(rows)).max(top);
                }
                ControlFunction::CursorDown(rows) | ControlFunction::LinePositionForward(rows) => {
                    self.cursor.wrapping = false;
                    let bottom: usize = self.bottom();
                    self.cursor.at.row = (self.cursor.at.row + count(rows)).min(bottom);
                }
                ControlFunction::CursorForward(columns)
                | ControlFunction::CharacterPositionForward(columns) => {
                    self.cursor.wrapping = false;
                    self.cursor.at.column =
                        (self.cursor.at.column + count(columns)).min(self.size.columns - 1);
                }
                ControlFunction::CursorBack(columns) => {
                    self.cursor.wrapping = false;
                    self.cursor.at.column = self.cursor.at.column.saturating_sub(count(columns));
                }
                ControlFunction::CursorNextLine(rows) => {
                    let bottom: usize = self.bottom();
                    self.cursor.wrapping = false;
                    self.cursor.at.row = (self.cursor.at.row + count(rows)).min(bottom);
                    self.cursor.at.column = 0;
                }
                ControlFunction::CursorPreviousLine(rows) => {
                    let top: usize = self.top();
                    self.cursor.wrapping = false;
                    self.cursor.at.row = self.cursor.at.row.saturating_sub(count(rows)).max(top);
                    self.cursor.at.column = 0;
                }
                ControlFunction::CursorColumn(column)
                | ControlFunction::CharacterPositionAbsolute(column) => {
                    self.cursor.wrapping = false;
                    self.cursor.at.column = at(column).min(self.size.columns - 1);
                }
                ControlFunction::CursorRow(row) => {
                    let column: usize = self.cursor.at.column;
                    self.move_to(at(row), column);
                }
                ControlFunction::CursorPosition { row, column } => {
                    self.move_to(at(row), at(column));
                }
                ControlFunction::CursorForwardTabulation(stops) => self.tab(count(stops)),
                ControlFunction::CursorBackwardTabulation(stops) => self.back_tab(count(stops)),

                ControlFunction::Index => {
                    self.cursor.wrapping = false;
                    self.down_a_row();
                }
                ControlFunction::ReverseIndex => {
                    self.cursor.wrapping = false;
                    self.up_a_row();
                }
                ControlFunction::NextLine => {
                    self.cursor.wrapping = false;
                    self.cursor.at.column = 0;
                    self.down_a_row();
                }

                ControlFunction::SaveCursor | ControlFunction::SaveCursorPosition => {
                    self.saved = Some(self.cursor);
                }
                ControlFunction::RestoreCursor | ControlFunction::RestoreCursorPosition => {
                    if let Some(saved) = self.saved {
                        self.cursor = saved;
                        self.cursor.at.row = self.cursor.at.row.min(self.size.rows - 1);
                        self.cursor.at.column = self.cursor.at.column.min(self.size.columns - 1);
                    }
                }

                ControlFunction::EraseInDisplay(erase) => self.erase_display(erase),
                ControlFunction::EraseInLine(erase) => self.erase_line(erase),
                ControlFunction::EraseCharacters(columns) => {
                    let row: usize = self.cursor.at.row;
                    let from: usize = self.cursor.at.column;
                    let style: Style = self.cursor.style;
                    self.grid
                        .line_mut(row)
                        .erase(from..from + count(columns), style);
                }
                ControlFunction::InsertCharacters(columns) => {
                    let row: usize = self.cursor.at.row;
                    let at: usize = self.cursor.at.column;
                    let style: Style = self.cursor.style;
                    self.grid.line_mut(row).insert(at, count(columns), style);
                }
                ControlFunction::DeleteCharacters(columns) => {
                    let row: usize = self.cursor.at.row;
                    let at: usize = self.cursor.at.column;
                    let style: Style = self.cursor.style;
                    self.grid.line_mut(row).delete(at, count(columns), style);
                }
                ControlFunction::InsertLines(rows) => self.insert_lines(count(rows)),
                ControlFunction::DeleteLines(rows) => self.delete_lines(count(rows)),

                ControlFunction::ScrollUp(rows) => {
                    self.scroll(count(rows).try_into().unwrap_or(isize::MAX))
                }
                ControlFunction::ScrollDown(rows) => {
                    self.scroll(-count(rows).try_into().unwrap_or(isize::MAX))
                }
                ControlFunction::SetScrollingRegion { top, bottom } => {
                    self.set_region(at(top), bottom.map_or(self.size.rows, usize::from));
                }

                ControlFunction::SetTabStop => {
                    self.tabs.insert(self.cursor.at.column);
                }
                ControlFunction::ClearTabStops(clear) => match clear {
                    ClearTabStops::AtCursor => {
                        self.tabs.remove(&self.cursor.at.column);
                    }
                    ClearTabStops::All => self.tabs.clear(),
                    ClearTabStops::Unknown(_) => {}
                },

                ControlFunction::SelectGraphicRendition(renditions) => {
                    for rendition in &renditions {
                        self.cursor.style.apply(rendition);
                    }
                }
                ControlFunction::SetMode { modes, set } => {
                    for mode in &modes {
                        self.set_mode(mode, set);
                    }
                }
                ControlFunction::OperatingSystemCommand(command) => self.command(command),
                ControlFunction::DesignateCharacterSet {
                    slot,
                    character_set,
                } => {
                    if let (Some(designated), Some(slot)) = (
                        CharacterSet::of(character_set),
                        self.character_sets.get_mut(usize::from(slot)),
                    ) {
                        *slot = designated;
                    }
                }
                ControlFunction::Reset => self.reset(),
                _ => {}
            }
        }

        /// Return the first row the cursor moves within, which is the top of the rows the text
        /// scrolls between only when the cursor is already inside them.
        fn top(&self) -> usize {
            return match self.cursor.at.row >= self.region.start {
                true => self.region.start,
                false => 0,
            };
        }

        /// Return the last row the cursor moves within, which is the bottom of the rows the text
        /// scrolls between only when the cursor is already inside them.
        fn bottom(&self) -> usize {
            return match self.cursor.at.row < self.region.end {
                true => self.region.end - 1,
                false => self.size.rows - 1,
            };
        }

        /// Set the rows which the text scrolls between and put the cursor back at the start.
        ///
        /// A region of fewer than two rows is no region at all, which is what a terminal makes of
        /// one, so the text goes back to scrolling between every row of the screen.
        fn set_region(&mut self, top: usize, bottom: usize) {
            let bottom: usize = bottom.min(self.size.rows);

            self.region = match top + 1 < bottom {
                true => top..bottom,
                false => 0..self.size.rows,
            };

            self.move_to(0, 0);
        }

        /// Make room for the given number of rows where the cursor is, pushing the ones below it
        /// down and off the bottom of the rows the text scrolls between.
        fn insert_lines(&mut self, rows: usize) {
            if !self.region.contains(&self.cursor.at.row) {
                return;
            }

            let region: Range<usize> = self.cursor.at.row..self.region.end;
            self.grid.scroll(
                region,
                -isize::try_from(rows).unwrap_or(isize::MAX),
                self.cursor.style,
            );
            self.cursor.at.column = 0;
        }

        /// Take the given number of rows out where the cursor is, pulling the ones below it up and
        /// bringing blank rows in at the bottom of the rows the text scrolls between.
        fn delete_lines(&mut self, rows: usize) {
            if !self.region.contains(&self.cursor.at.row) {
                return;
            }

            let region: Range<usize> = self.cursor.at.row..self.region.end;
            // NOTE: Nothing is kept from this even though rows leave, because the rows a program
            // takes out of the middle of the screen have not scrolled away.
            self.grid.scroll(
                region,
                isize::try_from(rows).unwrap_or(isize::MAX),
                self.cursor.style,
            );
            self.cursor.at.column = 0;
        }

        /// Blank the part of the screen the erase says.
        fn erase_display(&mut self, erase: EraseInDisplay) {
            let row: usize = self.cursor.at.row;
            let column: usize = self.cursor.at.column;
            let columns: usize = self.size.columns;
            let rows: usize = self.size.rows;
            let style: Style = self.cursor.style;

            match erase {
                EraseInDisplay::ToEnd => {
                    self.grid.line_mut(row).erase(column..columns, style);
                    self.grid.erase(row + 1..rows, style);
                }
                EraseInDisplay::ToStart => {
                    self.grid.erase(0..row, style);
                    self.grid.line_mut(row).erase(0..column + 1, style);
                }
                EraseInDisplay::All => self.grid.erase(0..rows, style),
                EraseInDisplay::AllAndScrollback => {
                    self.grid.erase(0..rows, style);
                    self.scrollback.clear();
                }
                EraseInDisplay::Unknown(_) => {}
            }
        }

        /// Blank the part of the row the cursor is in which the erase says.
        fn erase_line(&mut self, erase: EraseInLine) {
            let row: usize = self.cursor.at.row;
            let column: usize = self.cursor.at.column;
            let columns: usize = self.size.columns;
            let style: Style = self.cursor.style;

            let erased: Range<usize> = match erase {
                EraseInLine::ToEnd => column..columns,
                EraseInLine::ToStart => 0..column + 1,
                EraseInLine::All => 0..columns,
                EraseInLine::Unknown(_) => return,
            };
            self.grid.line_mut(row).erase(erased, style);
        }

        /// Turn the mode on, or off when it is being unset.
        fn set_mode(&mut self, mode: &Mode, set: bool) {
            match mode {
                // NOTE: The screen which is written on when the other one is put aside starts out
                // blank, and what was on the first one is given back untouched when it comes back.
                Mode::AlternateScreen
                | Mode::AlternateScreenAndClear
                | Mode::AlternateScreenAndSaveCursor => {
                    let saves: bool = matches!(mode, Mode::AlternateScreenAndSaveCursor);
                    self.set_alternate(set, saves);
                }
                Mode::SaveCursor => match set {
                    true => self.saved = Some(self.cursor),
                    false => {
                        if let Some(saved) = self.saved {
                            self.cursor = saved;
                        }
                    }
                },
                Mode::Origin => {
                    self.modes.set(Modes::ORIGIN, set);
                    // Counting the rows another way moves where the first one is, so the cursor
                    // goes back to it.
                    self.move_to(0, 0);
                }
                mode => {
                    if let Some(modes) = Modes::of(mode) {
                        self.modes.set(modes, set);
                    }
                }
            }
        }

        /// Write on the other screen, or go back to the first one.
        fn set_alternate(&mut self, set: bool, saves: bool) {
            if set == self.other.is_some() {
                return;
            }

            match set {
                true => {
                    if saves {
                        self.saved = Some(self.cursor);
                    }
                    let put_aside: Grid = mem::replace(&mut self.grid, Grid::blank(self.size));
                    self.other = Some(put_aside);
                }
                false => {
                    if let Some(other) = self.other.take() {
                        self.grid = other;
                    }
                    if saves {
                        if let Some(saved) = self.saved.take() {
                            self.cursor = saved;
                        }
                    }
                }
            }

            // The whole screen is a different one now, so all of it has to be sent again.
            self.grid.damage_all();
            self.cursor.wrapping = false;
        }

        /// Do what the operating system command asks.
        fn command(&mut self, command: OperatingSystemCommand) {
            match command {
                OperatingSystemCommand::SetIconNameAndWindowTitle(title)
                | OperatingSystemCommand::SetWindowTitle(title) => self.title = Some(title),
                _ => {}
            }
        }

        /// Put the screen back to the way it was when it was new, keeping its size.
        fn reset(&mut self) {
            let size: Size = self.size;

            self.grid = Grid::blank(size);
            self.other = None;
            self.cursor = Cursor::default();
            self.saved = None;
            self.region = 0..size.rows;
            self.tabs = Self::default_tabs(size.columns);
            self.modes = Modes::default();
            self.character_sets = [CharacterSet::default(); 4];
            self.character_set = 0;
        }
    }

    impl From<&Screen> for Fabric {
        fn from(screen: &Screen) -> Self {
            let mut fabric = Fabric::from(&screen.grid);

            if screen.is_cursor_shown() {
                fabric.set_cursor(Some(screen.cursor.at));
            }

            return fabric;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use crate::parser::Parser;

        use ansi::Color;
        use rend::Attributes;

        use test_case::test_case;

        /// How many rows the screen which is written on for testing has.
        const ROWS: usize = 4;

        /// How many columns each row of it has.
        const COLUMNS: usize = 6;

        /// Return a screen of the size which is written on for testing.
        fn screen() -> Screen {
            return Screen::from(
                Props::builder()
                    .size(Size::new(ROWS, COLUMNS))
                    .scrollback(2)
                    .build(),
            );
        }

        /// Return what is written in the line, which is how a row of the history is read.
        fn line(line: &Line) -> String {
            return (0..line.columns())
                .map(|column| line.cell(column).to_string())
                .collect();
        }

        /// Write the bytes on the screen the way a program would.
        fn write(screen: &mut Screen, bytes: &[u8]) {
            let actions = Parser::default().feed(bytes);
            screen.feed(actions);
        }

        /// Return the screen which the bytes mean to show.
        fn shown(bytes: &[u8]) -> Vec<String> {
            let mut screen = screen();
            write(&mut screen, bytes);
            return rows(&screen);
        }

        /// Return what each row of the screen is showing.
        fn rows(screen: &Screen) -> Vec<String> {
            let fabric = Fabric::from(screen);
            return (0..screen.size().rows)
                .map(|row| {
                    (0..screen.size().columns)
                        .map(|column| fabric.spot(row, column).cell().to_string())
                        .collect()
                })
                .collect();
        }

        #[test_case(b"abc", &["abc   ", "      ", "      ", "      "]; "plain characters")]
        #[test_case(b"ab\r\ncd", &["ab    ", "cd    ", "      ", "      "]; "a carriage return and a line feed")]
        #[test_case(b"ab\rc", &["cb    ", "      ", "      ", "      "]; "a carriage return on its own goes back over what was written")]
        #[test_case(b"abc\x08X", &["abX   ", "      ", "      ", "      "]; "a backspace")]
        #[test_case(b"a\tb", &["a    b", "      ", "      ", "      "]; "a tab with no stop to go to lands in the last column")]
        #[test_case(b"abcdefgh", &["abcdef", "gh    ", "      ", "      "]; "text which does not fit wraps onto the next row")]
        #[test_case(b"\x1b[2;3Hx", &["      ", "  x   ", "      ", "      "]; "the cursor moved to a row and column")]
        #[test_case(b"abc\x1b[2Dx", &["axc   ", "      ", "      ", "      "]; "the cursor moved back")]
        #[test_case(b"abc\x1b[1;2H\x1b[K", &["a     ", "      ", "      ", "      "]; "erasing to the end of a row")]
        #[test_case(b"ab\r\ncd\x1b[1;1H\x1b[J", &["      ", "      ", "      ", "      "]; "erasing to the end of the screen")]
        #[test_case(b"abc\x1b[2J", &["      ", "      ", "      ", "      "]; "erasing all of the screen")]
        #[test_case(b"abcd\x1b[1;2H\x1b[2P", &["ad    ", "      ", "      ", "      "]; "taking characters out")]
        #[test_case(b"abcd\x1b[1;2H\x1b[2@", &["a  bcd", "      ", "      ", "      "]; "making room for characters")]
        #[test_case(b"abcd\x1b[1;2H\x1b[2X", &["a  d  ", "      ", "      ", "      "]; "blanking characters")]
        fn test_what_the_bytes_mean_to_show(bytes: &[u8], expected: &[&str]) {
            assert_eq!(shown(bytes), expected);
        }

        #[test]
        fn test_a_row_written_right_across_wraps_only_once_the_next_cluster_arrives() {
            let mut screen = screen();

            write(&mut screen, b"abcdef");

            // The cursor stays in the last column, because there is no column past it to be in.
            assert_eq!(
                screen.cursor(),
                Location::builder().row(0).column(5).build()
            );

            write(&mut screen, b"g");
            assert_eq!(rows(&screen)[1], "g     ");
        }

        #[test]
        fn test_text_which_does_not_fit_is_dropped_when_the_screen_has_been_asked_not_to_wrap() {
            let mut screen = screen();

            write(&mut screen, b"\x1b[?7labcdefgh");

            // Everything which does not fit is written over the last column again and again.
            assert_eq!(rows(&screen)[0], "abcdeh");
            assert_eq!(rows(&screen)[1], "      ");
        }

        #[test]
        fn test_a_line_feed_at_the_bottom_scrolls_and_keeps_the_row_which_went() {
            let mut screen = screen();

            write(&mut screen, b"a\r\nb\r\nc\r\nd\r\ne");

            assert_eq!(rows(&screen), vec!["b     ", "c     ", "d     ", "e     "]);
            assert_eq!(screen.scrollback().len(), 1);
        }

        #[test]
        fn test_no_more_rows_are_kept_than_there_is_room_for() {
            let mut screen = screen();

            write(&mut screen, b"a\r\nb\r\nc\r\nd\r\ne\r\nf\r\ng");

            // Three rows scrolled off and there is only room for two of them.
            assert_eq!(screen.scrollback().len(), 2);
        }

        #[test]
        fn test_the_text_only_scrolls_between_the_rows_it_was_told_to() {
            let mut screen = screen();

            // The text scrolls between the second and third rows only.
            write(&mut screen, b"\x1b[2;3r\x1b[2;1Ha\r\nb\r\nc");

            assert_eq!(rows(&screen), vec!["      ", "b     ", "c     ", "      "]);
            // Nothing left the screen, so nothing was kept.
            assert_eq!(screen.scrollback().len(), 0);
        }

        #[test]
        fn test_the_other_screen_is_blank_and_gives_the_first_one_back_untouched() {
            let mut screen = screen();
            write(&mut screen, b"abc");

            // NOTE: Going to the other screen leaves the cursor where it was, which is what xterm
            // does, so a program which wants it somewhere else says so. Vim does.
            write(&mut screen, b"\x1b[?1049h");
            assert_eq!(rows(&screen)[0], "      ");
            write(&mut screen, b"\x1b[Hxyz");
            assert_eq!(rows(&screen)[0], "xyz   ");

            write(&mut screen, b"\x1b[?1049l");
            assert_eq!(rows(&screen)[0], "abc   ");
        }

        #[test]
        fn test_the_cursor_is_put_aside_and_come_back_to() {
            let mut screen = screen();

            write(&mut screen, b"\x1b[2;3H\x1b7\x1b[1;1H\x1b8");

            assert_eq!(
                screen.cursor(),
                Location::builder().row(1).column(2).build()
            );
        }

        #[test]
        fn test_the_way_text_is_written_is_put_aside_along_with_the_cursor() {
            let mut screen = screen();

            write(&mut screen, b"\x1b[31m\x1b7\x1b[32m\x1b8x");

            let fabric = Fabric::from(&screen);
            assert_eq!(fabric.spot(0, 0).style().color(), Some(Color::Red));
        }

        #[test]
        fn test_text_is_written_the_way_the_renditions_say() {
            let mut screen = screen();

            write(&mut screen, b"\x1b[1;4;31ma");

            let style = Fabric::from(&screen).spot(0, 0).style();
            assert_eq!(style.color(), Some(Color::Red));
            assert_eq!(style.attributes().contains(Attributes::BOLD), true);
            assert_eq!(style.underline(), ansi::Underline::Single);
        }

        #[test]
        fn test_the_cursor_is_only_shown_when_the_screen_says_it_is() {
            let mut screen = screen();
            assert_eq!(Fabric::from(&screen).cursor(), Some(Location::default()));

            write(&mut screen, b"\x1b[?25l");

            assert_eq!(Fabric::from(&screen).cursor(), None);
        }

        #[test]
        fn test_the_title_is_taken_from_what_the_program_asked_for() {
            let mut screen = screen();

            write(&mut screen, b"\x1b]0;a title\x07");

            assert_eq!(screen.title(), Some("a title"));
        }

        #[test]
        fn test_the_bell_is_counted_and_then_forgotten() {
            let mut screen = screen();

            write(&mut screen, b"a\x07b\x07");

            assert_eq!(screen.take_bells(), 2);
            assert_eq!(screen.take_bells(), 0);
        }

        #[test]
        fn test_a_wide_cluster_which_does_not_fit_goes_onto_the_next_row() {
            let mut screen = screen();

            // Five columns are written in, so the crab does not fit in the one which is left.
            write(&mut screen, "abcde🦀".as_bytes());

            assert_eq!(rows(&screen)[0], "abcde ");
            assert_eq!(rows(&screen)[1], "🦀    ");
        }

        #[test]
        fn test_a_combining_accent_belongs_to_the_cluster_before_it() {
            let mut screen = screen();

            write(&mut screen, "e\u{301}x".as_bytes());

            assert_eq!(rows(&screen)[0], "e\u{301}x    ");
            assert_eq!(
                screen.cursor(),
                Location::builder().row(0).column(2).build()
            );
        }

        #[test]
        fn test_the_screen_is_made_smaller_below_what_was_written() {
            let mut screen = screen();
            write(&mut screen, b"abc\r\ndef");

            screen.resize(Size::new(2, 3));

            // The rows which no longer fit were the blank ones below the cursor, so what was
            // written stayed where it was and none of it went to the history.
            assert_eq!(rows(&screen), vec!["abc", "def"]);
            assert_eq!(screen.scrollback().len(), 0);
        }

        #[test]
        fn test_the_screen_is_made_smaller_than_what_was_written() {
            let mut screen = screen();
            write(&mut screen, b"abc\r\ndef\r\nghi\r\njkl");

            screen.resize(Size::new(2, 3));

            // There are no blank rows below the cursor to lose, so the rows which no longer fit
            // scrolled off the top and what was written last stayed on the screen.
            assert_eq!(rows(&screen), vec!["ghi", "jkl"]);
            assert_eq!(
                screen
                    .scrollback()
                    .iter()
                    .map(line)
                    .collect::<Vec<String>>(),
                vec!["abc   ", "def   "]
            );
        }

        #[test]
        fn test_the_cursor_goes_up_with_the_rows_which_scrolled_off() {
            let mut screen = screen();
            write(&mut screen, b"abc\r\ndef\r\nghi\r\njkl");

            screen.resize(Size::new(2, 3));

            // The cursor was on the last row and two rows scrolled off above it, so it is on the
            // last row still rather than having been dragged back up the screen.
            assert_eq!(
                screen.cursor(),
                Location::builder().row(1).column(2).build()
            );
        }

        #[test]
        fn test_the_screen_is_made_bigger() {
            let mut screen = screen();
            write(&mut screen, b"abc\r\ndef");

            screen.resize(Size::new(6, 3));

            assert_eq!(
                rows(&screen),
                vec!["abc", "def", "   ", "   ", "   ", "   "]
            );
            assert_eq!(screen.scrollback().len(), 0);
        }

        #[test]
        fn test_the_cursor_stays_on_the_screen_when_it_is_made_smaller() {
            let mut screen = screen();
            write(&mut screen, b"\x1b[4;6H");

            screen.resize(Size::new(2, 3));

            assert_eq!(
                screen.cursor(),
                Location::builder().row(1).column(2).build()
            );
        }

        #[test]
        fn test_rows_are_moved_about_within_the_rows_the_text_scrolls_between() {
            let mut screen = screen();
            write(&mut screen, b"a\r\nb\r\nc\r\nd");

            // Take the second row out, which pulls the ones below it up.
            write(&mut screen, b"\x1b[2;1H\x1b[M");

            assert_eq!(rows(&screen), vec!["a     ", "c     ", "d     ", "      "]);
        }

        #[test]
        fn test_letters_stand_for_lines_once_the_line_drawing_set_is_asked_for() {
            let mut screen = screen();

            // The set is designated in the first slot and then written in.
            write(&mut screen, b"\x1b(0lqk\x1b(Bx");

            assert_eq!(rows(&screen)[0], "┌─┐x  ");
        }

        #[test]
        fn test_which_slot_is_being_written_in_is_shifted_between() {
            let mut screen = screen();

            // The line drawing set goes in the second slot, which is shifted to and back from.
            write(&mut screen, b"\x1b)0q\x0eq\x0fq");

            assert_eq!(rows(&screen)[0], "q─q   ");
        }

        #[test]
        fn test_only_the_rows_which_were_written_on_are_waiting_to_be_sent() {
            let mut screen = screen();
            screen.take_damage();

            write(&mut screen, b"\x1b[3;1Hx");

            assert_eq!(screen.take_damage(), BTreeSet::from([2]));
            // Asking for them takes them as having been sent.
            assert_eq!(screen.take_damage(), BTreeSet::new());
        }

        #[test]
        fn test_every_row_is_waiting_to_be_sent_after_scrolling() {
            let mut screen = screen();
            write(&mut screen, b"a\r\nb\r\nc\r\nd");
            screen.take_damage();

            write(&mut screen, b"\r\ne");

            assert_eq!(screen.take_damage(), BTreeSet::from([0, 1, 2, 3]));
        }

        #[test_case(b"\x08"; "moving the cursor")]
        #[test_case(b"\x1b[?1h"; "changing the way it behaves")]
        #[test_case(b"\x1b]2;title\x07"; "changing the title")]
        fn test_a_change_which_writes_on_no_row_is_waiting_to_be_sent(bytes: &[u8]) {
            let mut screen = screen();
            write(&mut screen, b"abc");
            screen.take_damage();
            screen.take_status_changed();

            write(&mut screen, bytes);

            assert_eq!(screen.take_damage(), BTreeSet::new());
            assert_eq!(screen.take_status_changed(), true);
            // Asking for it takes it as having been sent.
            assert_eq!(screen.take_status_changed(), false);
        }

        #[test]
        fn test_nothing_is_waiting_to_be_sent_when_nothing_changed() {
            let mut screen = screen();

            write(&mut screen, b"");

            assert_eq!(screen.take_status_changed(), false);
        }

        #[test]
        fn test_a_reset_puts_the_screen_back_to_the_way_it_started() {
            let mut screen = screen();
            write(&mut screen, b"\x1b[31mabc\x1b[2;3r\x1b[?25l");

            write(&mut screen, b"\x1bc");

            assert_eq!(rows(&screen)[0], "      ");
            assert_eq!(screen.cursor(), Location::default());
            assert_eq!(screen.is_cursor_shown(), true);
        }
    }
}
pub use screen::Screen;
