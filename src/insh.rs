use std::cmp;
use std::convert::TryInto;
use std::io::{self, Stdout, Write};

use crate::action::Action;
use crate::bash_shell::BashShell;
use crate::color::Color;
use crate::effect::Effect;
use crate::state::{Mode, PatternState, State};
use crate::vim::Vim;

extern crate crossterm;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    style::{
        Attribute::Bold, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
    },
    terminal::{self, ClearType},
    QueueableCommand,
};

pub struct Insh {
    stdout: Stdout,

    state: State,
}

impl From<State> for Insh {
    fn from(state: State) -> Self {
        let stdout = io::stdout();
        Insh { stdout, state }
    }
}
impl Insh {
    pub fn run(&mut self) {
        self.set_up();

        self.display();

        loop {
            let action = self.next_action();

            let effect: Option<Effect> = self.state.perform(&action);

            if let Some(effect) = effect {
                self.perform(effect);
            }

            if action == Action::Exit {
                break;
            }

            self.display();
        }

        self.clean_up();
    }

    fn next_action(&self) -> Action {
        loop {
            let event = event::read().unwrap();

            if let Event::Key(KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::CONTROL,
            }) = event
            {
                return Action::Exit;
            }

            match self.state.mode {
                Mode::Browse => match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => return Action::Exit,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('j'),
                        ..
                    }) => return Action::BrowseScrollDown,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('k'),
                        ..
                    }) => return Action::BrowseScrollUp,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('G'),
                        ..
                    }) => return Action::BrowseGoToBottom,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('g'),
                        ..
                    }) => return Action::BrowseGoToTop,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('l'),
                        ..
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => return Action::BrowseDrillDown,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('h'),
                        ..
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    }) => return Action::BrowseDrillUp,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('e'),
                        ..
                    }) => return Action::BrowseEdit,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('b'),
                        ..
                    }) => return Action::RunBash,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('f'),
                        ..
                    }) => return Action::EnterFindMode,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('s'),
                        ..
                    }) => {
                        return Action::EnterSearchMode;
                    }
                    _ => {}
                },
                Mode::Find => match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => return Action::EnterBrowseMode,
                    Event::Key(KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    }) => {
                        return Action::FindDeletePreviousCharacter;
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => return Action::EnterBrowseFindMode,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char(character),
                        ..
                    }) => {
                        return Action::FindAppendCharacter(character);
                    }
                    _ => {}
                },
                Mode::BrowseFind => match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => {
                        return Action::EnterFindMode;
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('j'),
                        ..
                    }) => return Action::FindScrollDown,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('k'),
                        ..
                    }) => return Action::FindScrollUp,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('g'),
                        ..
                    }) => {
                        return Action::FindBrowseSelectedParent;
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('e'),
                        ..
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Char('l'),
                        ..
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => {
                        return Action::FindEditFile;
                    }
                    _ => {}
                },
                Mode::Search => match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => return Action::EnterBrowseMode,
                    Event::Key(KeyEvent {
                        code: KeyCode::Char(character),
                        ..
                    }) => {
                        return Action::SearchAppendCharacter(character);
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    }) => {
                        return Action::SearchDeletePreviousCharacter;
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => return Action::EnterBrowseSearchMode,
                    _ => {}
                },
                Mode::BrowseSearch => match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => {
                        return Action::EnterSearchMode;
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('j'),
                        ..
                    }) => {
                        return Action::SearchScrollDown;
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('k'),
                        ..
                    }) => {
                        return Action::SearchScrollUp;
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('e'),
                        ..
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Char('l'),
                        ..
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    }) => {
                        return Action::SearchEditFile;
                    }
                    _ => {
                        continue;
                    }
                },
            }
        }
    }

    fn perform(&mut self, effect: Effect) {
        match effect {
            Effect::RunBash(directory) => {
                self.disable_raw_terminal();
                self.lazy_clear_screen();
                self.lazy_move_cursor(0, 0);
                self.lazy_show_cursor();
                self.update_terminal();

                BashShell::run(&directory);

                self.enable_raw_terminal();
                self.lazy_hide_cursor();
            }
            Effect::RunVim(filename) => {
                Vim::run(&filename);
                self.lazy_hide_cursor();
            }
            Effect::RunVimAtLine(filename, line_number) => {
                Vim::run_at_line(&filename, line_number);
                self.lazy_hide_cursor();
            }
            Effect::RunVimWithCommand(filename, command) => {
                Vim::run_with_command(&filename, command);
                self.lazy_hide_cursor();
            }
        }
    }

    fn set_up(&mut self) {
        self.lazy_enable_alternate_terminal();
        self.enable_raw_terminal();
        self.lazy_hide_cursor();

        self.change_panic_hook();
    }

    fn clean_up(&mut self) {
        self.lazy_disable_alternate_terminal();
        self.disable_raw_terminal();
        self.lazy_show_cursor();
    }

    fn display(&mut self) {
        match self.state.mode {
            Mode::Browse => {
                self.lazy_display_browse();
            }
            Mode::Find | Mode::BrowseFind => {
                self.lazy_display_find();
            }
            Mode::Search | Mode::BrowseSearch => {
                self.lazy_display_search();
            }
        }

        self.update_terminal();
    }

    fn lazy_display_browse(&mut self) {
        self.lazy_clear_screen();

        let start = self.state.browse.offset;
        let end: usize = cmp::min(
            self.state.browse.offset + (self.state.terminal_size.height as usize),
            self.state.browse.entries.len(),
        );
        for (line_number, entry_number) in (start..end).enumerate() {
            let entry = &self.state.browse.entries[entry_number];
            let file_name = entry.file_name();
            let entry_name = file_name.to_string_lossy();

            self.lazy_move_cursor(0, line_number as u16);

            let mut reset = false;
            if line_number == self.state.browse.selected {
                // Named arguments (not in Rust?) would be nice for lazy_color! Make a macro?
                self.lazy_start_color(Color::InvertedText, Color::Highlight);
                reset = true;
            }
            self.lazy_print(&entry_name);

            let is_dir = self.state.browse.entries[entry_number].path().is_dir();
            if is_dir {
                self.lazy_print("/");
            }

            if reset {
                self.lazy_reset_color()
            }
        }
    }

    fn lazy_display_find(&mut self) {
        self.lazy_clear_screen();

        // Display the search bar.
        self.lazy_move_cursor(0, 0);

        self.lazy_start_background_color(Color::InvertedBackground);
        let mut pattern = self.state.find.pattern.clone();
        pattern.truncate(self.state.terminal_size.width.into());
        pattern = format!(
            "{:width$}",
            pattern,
            width = self.state.terminal_size.width as usize
        );

        let text_color = match self.state.find.pattern_state {
            PatternState::NotCompiled => Color::NotCompiledRegex,
            PatternState::BadRegex => Color::BadRegex,
            PatternState::GoodRegex => Color::InvertedText,
        };

        self.lazy_start_text_color(text_color);
        self.lazy_print(&pattern);
        self.lazy_reset_color();

        // Display found entries
        for entry_number in 0..(self.state.terminal_size.height as usize - 1) {
            self.lazy_move_cursor(0, (entry_number + 1).try_into().unwrap());
            let entry_index = self.state.find.offset + entry_number;
            if entry_index == self.state.find.found.len() {
                break;
            }
            let file_name = self.state.find.found[entry_index].file_name();
            let entry_name = file_name.to_string_lossy();

            let reset;
            if entry_number == self.state.find.selected && self.state.mode == Mode::BrowseFind {
                self.lazy_start_color(Color::InvertedText, Color::Highlight);
                reset = true;
            } else {
                reset = false;
            }

            self.lazy_print(&entry_name);

            if reset {
                self.lazy_reset_color()
            }
        }
    }

    fn lazy_display_search(&mut self) {
        self.lazy_clear_screen();

        // Display the search phrase.
        self.lazy_move_cursor(0, 0);

        self.lazy_start_background_color(Color::InvertedBackground);

        let mut search = self.state.search.search.clone();
        search.truncate(self.state.terminal_size.width.into());
        search = format!(
            "{:width$}",
            search,
            width = self.state.terminal_size.width as usize
        );

        self.lazy_start_text_color(Color::InvertedText);
        self.lazy_print(&search);
        self.lazy_reset_color();

        let selected_line = self.state.search_selected_line();

        let mut lines = 0;

        // Display the first hit.

        let mut file_hit_number = self.state.search.file_offset;

        if self.state.search.hits.is_empty() {
            return;
        }

        // Print the first file name.
        if self.state.search.line_offset == None {
            self.lazy_move_cursor(0, 1);
            let file_hit = self.state.search.hits[file_hit_number].clone();
            if usize::from(lines) == selected_line {
                self.lazy_start_color(Color::InvertedText, Color::Highlight);
            }
            self.lazy_start_bold();
            self.lazy_print(&file_hit.file.to_string_lossy());
            self.lazy_reset_color();
            lines += 1;
            if lines == (self.state.terminal_size.height - 1) {
                return;
            }
        }

        // Print the line hits of the first file hit.
        let mut line_hit_number = self.state.search.line_offset.unwrap_or(0);
        let file_hit = self.state.search.hits[file_hit_number].clone();
        loop {
            if line_hit_number == file_hit.hits.len() {
                // Add a blank line between the first hit and the rest of the hits.
                lines += 1;
                if lines == (self.state.terminal_size.height - 1) {
                    return;
                }
                break;
            }
            if line_hit_number > file_hit.hits.len() {
                break;
            }

            let line_hit = file_hit.hits[line_hit_number].clone();

            self.lazy_move_cursor(0, lines + 1);

            let mut reset_color = false;
            if usize::from(lines) == selected_line {
                self.lazy_start_color(Color::InvertedText, Color::Highlight);
                reset_color = true;
            }

            let mut string = String::new();
            string.push_str(&line_hit.line_number.to_string());
            string.push(':');
            string.push_str(&line_hit.line);
            self.lazy_print(&string);

            if reset_color {
                self.lazy_reset_color();
            }

            lines += 1;
            if lines == (self.state.terminal_size.height - 1) {
                return;
            }

            line_hit_number += 1;
        }

        file_hit_number += 1;
        line_hit_number = 0;

        // Display the rest of the hits.
        loop {
            // Print the file name.
            self.lazy_move_cursor(0, lines + 1);
            if file_hit_number >= self.state.search.hits.len() {
                break;
            }
            let file_hit = self.state.search.hits[file_hit_number].clone();
            if usize::from(lines) == selected_line {
                self.lazy_start_color(Color::InvertedText, Color::Highlight);
            }
            self.lazy_start_bold();
            self.lazy_print(&file_hit.file.to_string_lossy());
            self.lazy_reset_color();
            lines += 1;
            if lines == (self.state.terminal_size.height - 1) {
                break;
            }
            file_hit_number += 1;

            // Print the lines.
            loop {
                let line_hit = file_hit.hits[line_hit_number].clone();

                self.lazy_move_cursor(0, lines + 1);

                let mut reset_color = false;
                if usize::from(lines) == selected_line {
                    self.lazy_start_color(Color::InvertedText, Color::Highlight);
                    reset_color = true;
                }

                let mut string = String::new();
                string.push_str(&line_hit.line_number.to_string());
                string.push(':');
                string.push_str(&line_hit.line);
                self.lazy_print(&string);

                if reset_color {
                    self.lazy_reset_color();
                }

                lines += 1;
                if lines == (self.state.terminal_size.height - 1) {
                    return;
                }

                line_hit_number += 1;
                if line_hit_number == file_hit.hits.len() {
                    break;
                }
            }

            line_hit_number = 0;

            // Skip a line.
            lines += 1;
            if lines == (self.state.terminal_size.height - 1) {
                break;
            }
        }
    }

    fn change_panic_hook(&mut self) {
        let hook_before = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let mut stdout = io::stdout();
            stdout.queue(terminal::LeaveAlternateScreen).unwrap();
            stdout.queue(cursor::Show).unwrap();
            stdout.flush().unwrap();
            terminal::disable_raw_mode().unwrap();
            hook_before(info);
        }));
    }

    fn enable_raw_terminal(&mut self) {
        terminal::enable_raw_mode().unwrap();
    }

    fn disable_raw_terminal(&mut self) {
        terminal::disable_raw_mode().unwrap();
    }

    fn lazy_enable_alternate_terminal(&mut self) {
        self.stdout.queue(terminal::EnterAlternateScreen).unwrap();
    }

    fn lazy_disable_alternate_terminal(&mut self) {
        self.stdout.queue(terminal::LeaveAlternateScreen).unwrap();
    }

    fn lazy_hide_cursor(&mut self) {
        self.stdout.queue(cursor::Hide).unwrap();
    }

    fn lazy_show_cursor(&mut self) {
        self.stdout.queue(cursor::Show).unwrap();
    }

    fn lazy_move_cursor(&mut self, x: u16, y: u16) {
        self.stdout.queue(cursor::MoveTo(x, y)).unwrap();
    }

    fn lazy_clear_screen(&mut self) {
        self.stdout.queue(terminal::Clear(ClearType::All)).unwrap();
    }

    fn lazy_start_color(&mut self, foreground: Color, background: Color) {
        self.stdout
            .queue(SetForegroundColor(foreground.into()))
            .unwrap();
        self.stdout
            .queue(SetBackgroundColor(background.into()))
            .unwrap();
    }

    fn lazy_start_text_color(&mut self, color: Color) {
        self.stdout.queue(SetForegroundColor(color.into())).unwrap();
    }

    fn lazy_start_background_color(&mut self, color: Color) {
        self.stdout.queue(SetBackgroundColor(color.into())).unwrap();
    }

    fn lazy_reset_color(&mut self) {
        self.stdout.queue(ResetColor).unwrap();
    }

    fn lazy_start_bold(&mut self) {
        self.stdout.queue(SetAttribute(Bold)).unwrap();
    }

    fn lazy_print(&mut self, string: &str) {
        self.stdout.queue(Print(string)).unwrap();
    }

    fn update_terminal(&mut self) {
        self.stdout.flush().unwrap();
    }
}
