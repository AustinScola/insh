//! Contains the [`Editor`] component.

/// Contains the [`Props`] struct.
mod props {
    use std::path::PathBuf;

    use insh_api::Position;
    use rend::Size;

    use typed_builder::TypedBuilder;

    /// The properties of the editor.
    #[derive(TypedBuilder)]
    pub struct Props {
        /// The file to edit.
        pub path: PathBuf,
        /// How big the editor is.
        pub size: Size,
        /// Where the cursor starts, which is the start of the file when it is nowhere in
        /// particular.
        #[builder(default)]
        pub at: Option<Position>,
    }
}
pub use props::Props;

/// Contains the [`Effect`] enum.
mod effect {
    use crate::programs::VimArgs;

    use insh_api::Request;

    /// Something which the editor asks for.
    pub enum Effect {
        /// Ask the daemon for something.
        Request(Request),
        /// Stop editing the file.
        Quit,
        /// Stop editing the file and open it in vim instead.
        OpenVim(VimArgs),
    }
}
pub use effect::Effect;

/// Contains the [`Editor`] component.
mod editor {
    use std::path::Path;

    use super::{Action, Effect, Props, State};
    use crate::color::Color;
    use crate::components::common::{Footer, FooterProps, Hover, PaneContentsEvent as Event};
    use crate::stateful::Stateful;

    use insh_api::{EditorKey, Request};
    use rend::{Fabric, Location, Size, Yarn};
    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use til::Component;

    /// A modal editor, whose file and cursor are kept by the daemon.
    ///
    /// Nothing about the file is worked out here. What is typed is sent to the daemon as it is
    /// pressed, and what the daemon sends back is what is drawn, which is what lets other cursors
    /// be in the file at the same time.
    pub struct Editor {
        /// The state of the editor.
        state: State,
    }

    impl Component<Props, Event, Effect> for Editor {
        fn new(props: Props) -> Self {
            Self {
                state: State::from(props),
            }
        }

        fn name(&self) -> String {
            String::from("editor")
        }

        fn handle(&mut self, event: Event) -> Option<Effect> {
            let action: Action = match event {
                Event::SetFocus { focused } => Action::SetFocus { focused },
                Event::Response(response) => Action::Respond(Box::new(response)),
                Event::TermEvent(TermEvent::Resize(size)) => Action::Resize(size),
                Event::TermEvent(TermEvent::KeyEvent(key_event)) => {
                    Action::Press(Self::key(key_event)?)
                }
                Event::TermEvent(TermEvent::Paste(text)) => Action::Paste(text),
                Event::TermEvent(_) => return None,
            };

            self.state.perform(action)
        }

        fn render(&self, size: Size) -> Fabric {
            if size.rows == 0 || size.columns == 0 {
                return Fabric::new(size);
            }

            let mut bar: Yarn = Yarn::from(self.state.path_string());
            bar.resize(size.columns);
            bar.color(Color::InvertedText.into());
            bar.background(Color::InvertedBackground.into());
            let mut fabric: Fabric = Fabric::from(bar);
            if size.rows == 1 {
                return fabric;
            }

            if size.rows > 2 {
                let text: Size = Size::new(size.rows - 2, size.columns);
                fabric = fabric.quilt_bottom(self.state.render_text(text));
            }

            let footer: Fabric = match self.state.render_command(size.columns) {
                Some(command) => command,
                None => {
                    let footer_props = FooterProps::builder()
                        .name(self.name())
                        .info(&self.state)
                        .build();
                    Footer::new(footer_props).render(Size::new(1, size.columns))
                }
            };
            fabric.quilt_bottom(footer)
        }
    }

    impl Editor {
        /// Return the request which opens the file, which is what everything about it is told on.
        pub fn open(&self) -> Request {
            self.state.open()
        }

        /// Return the request which is done with the cursor, which is nothing once the daemon is
        /// done with it already.
        pub fn close(&self) -> Option<Request> {
            self.state.close()
        }

        /// Return the file.
        pub fn path(&self) -> &Path {
            self.state.path()
        }

        /// Return where the cursor is, which is below the file bar.
        pub fn cursor(&self) -> Option<Location> {
            let at: Location = self.state.cursor()?;
            Some(
                Location::builder()
                    .row(at.row + 1)
                    .column(at.column)
                    .build(),
            )
        }

        /// Return whether the row is shown as being under the mouse when it is, which is only the
        /// footer while what is typed goes to another pane, since pressing on it goes to this one.
        pub fn hovers(&self, row: usize) -> Option<Hover> {
            self.state.hovers(row)
        }

        /// Return the key which was pressed, the way the daemon is told it.
        fn key(key_event: KeyEvent) -> Option<EditorKey> {
            let KeyEvent { key, mods } = key_event;

            let key: EditorKey = match key {
                Key::Char(character) if mods.contains(KeyMods::CONTROL) => {
                    EditorKey::Control(character.to_ascii_lowercase())
                }
                Key::SynchronousIdle => EditorKey::Control('v'),
                Key::Char(character) => EditorKey::Char(character),
                Key::Escape => EditorKey::Escape,
                Key::CarriageReturn => EditorKey::Enter,
                Key::HorizontalTab => EditorKey::Tab,
                Key::Backspace => EditorKey::Backspace,
                _ => return None,
            };

            Some(key)
        }
    }
}
pub use editor::Editor;

/// Contains the [`State`] struct.
mod state {
    use std::path::{Path, PathBuf};

    use super::{Action, Effect, Props};
    use crate::clipboard::Clipboard;
    use crate::color::Color;
    use crate::components::common::{FooterInfo, Hover};
    use crate::programs::VimArgsBuilder;
    use crate::stateful::Stateful;
    use crate::typed_path::TypedPath;

    use highlighter::Run;
    use insh_api::{
        CloseEditorRequestParams, EditRequestParams, EditorCursor, EditorKey,
        EditorKeyRequestParams, EditorMode, EditorView, PasteEditorRequestParams, Position,
        Request, RequestParams, ResizeEditorRequestParams, Response, ResponseParams,
    };
    use rend::{Cell, Fabric, Location, Size, Yarn};

    use uuid::Uuid;

    /// How many columns apart the stops a tab goes to are.
    const TAB_STOP: usize = 8;

    /// The state of the editor.
    pub struct State {
        /// The file.
        path: PathBuf,
        /// How big the editor is.
        size: Size,
        /// Where the cursor starts.
        at: Option<Position>,
        /// The request which the file is told about on.
        request: Uuid,
        /// What the daemon last said is shown, which is nothing until it has said.
        view: Option<EditorView>,
        /// Why the file could not be edited, when it could not be.
        error: Option<String>,
        /// Whether what is typed goes to some pane other than the one this is in.
        pane_unfocused: bool,
        /// Whether the daemon is done with the cursor.
        closed: bool,
        /// How many columns the text is scrolled along by.
        left: usize,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            Self {
                path: props.path,
                size: props.size,
                at: props.at,
                request: Uuid::new_v4(),
                view: None,
                error: None,
                pane_unfocused: false,
                closed: false,
                left: 0,
            }
        }
    }

    impl State {
        /// Return the request which opens the file.
        pub fn open(&self) -> Request {
            Request::builder()
                .uuid(self.request)
                .params(RequestParams::Edit(
                    EditRequestParams::builder()
                        .path(self.path.clone())
                        .size(Self::text_size(self.size))
                        .at(self.at)
                        .build(),
                ))
                .build()
        }

        /// Return the request which is done with the cursor.
        pub fn close(&self) -> Option<Request> {
            if self.closed {
                return None;
            }

            Some(
                Request::builder()
                    .params(RequestParams::CloseEditor(
                        CloseEditorRequestParams::builder()
                            .edit(self.request)
                            .build(),
                    ))
                    .build(),
            )
        }

        /// Return the file.
        pub fn path(&self) -> &Path {
            &self.path
        }

        /// Return the file as it is shown, which is with the home directory as a tilde.
        pub fn path_string(&self) -> String {
            TypedPath::builder()
                .home(dirs::home_dir())
                .from(self.path.clone())
                .build()
                .as_typed_from_home(&self.path.to_string_lossy())
        }

        /// Return the mode of the cursor, which is nothing until the daemon has said.
        fn mode(&self) -> Option<EditorMode> {
            self.view
                .as_ref()
                .and_then(EditorView::own)
                .map(EditorCursor::mode)
        }

        /// Draw the command or search being typed, when one is, which is one row where the footer
        /// usually is.
        ///
        /// It is drawn the way the prompt of the multiplexer is, so that it is plain that what is
        /// typed is going to the command or search and not to the file.
        pub fn render_command(&self, columns: usize) -> Option<Fabric> {
            let view: &EditorView = self.view.as_ref()?;
            let prompt: String = match (view.command(), view.search()) {
                (Some(command), _) => format!(":{}", command),
                (None, Some(search)) => format!("/{}", search),
                (None, None) => return None,
            };

            let mut yarn = Yarn::from(prompt);
            yarn.resize(columns);
            yarn.color(Color::InvertedText.into());
            yarn.background(Color::Highlight.into());
            yarn.background_before(Color::LabelHighlight.into(), 1);

            Some(Fabric::from(yarn))
        }

        /// Return whether the row is shown as being under the mouse when it is.
        ///
        /// Only the footer is, and only while what is typed goes to another pane. It is not while a
        /// command or search is being typed in its place.
        pub fn hovers(&self, row: usize) -> Option<Hover> {
            let footer: bool = self.size.rows >= 2 && row + 1 == self.size.rows;
            let typing: bool = self
                .view
                .as_ref()
                .is_some_and(|view| view.command().is_some() || view.search().is_some());
            (self.pane_unfocused && footer && !typing).then_some(Hover::Whitened)
        }

        /// Return how much of the editor the text is shown in, which is all but the file bar and
        /// the footer.
        fn text_size(size: Size) -> Size {
            Size::new(size.rows.saturating_sub(2), size.columns)
        }

        /// Send the key to the daemon.
        fn press(&mut self, key: EditorKey) -> Option<Effect> {
            if self.error.is_some() {
                return Some(Effect::Quit);
            }

            let cursor: Uuid = *self.view.as_ref()?.cursor();
            Some(Effect::Request(
                Request::builder()
                    .params(RequestParams::EditorKey(
                        EditorKeyRequestParams::builder()
                            .cursor(cursor)
                            .key(key)
                            .build(),
                    ))
                    .build(),
            ))
        }

        /// Send the pasted text to the daemon.
        fn paste(&mut self, text: String) -> Option<Effect> {
            if self.error.is_some() {
                return None;
            }

            let cursor: Uuid = *self.view.as_ref()?.cursor();
            Some(Effect::Request(
                Request::builder()
                    .params(RequestParams::PasteEditor(
                        PasteEditorRequestParams::builder()
                            .cursor(cursor)
                            .text(text)
                            .build(),
                    ))
                    .build(),
            ))
        }

        /// Take note of how big the editor is, and tell the daemon.
        fn resize(&mut self, size: Size) -> Option<Effect> {
            self.size = size;
            self.scroll();

            let cursor: Uuid = *self.view.as_ref()?.cursor();
            Some(Effect::Request(
                Request::builder()
                    .params(RequestParams::ResizeEditor(
                        ResizeEditorRequestParams::builder()
                            .cursor(cursor)
                            .size(Self::text_size(size))
                            .build(),
                    ))
                    .build(),
            ))
        }

        /// Take note of what the daemon said.
        fn respond(&mut self, response: Response) -> Option<Effect> {
            // NOTE: Every pane is told about every response, and these are the only ones wanted.
            if *response.uuid() != self.request {
                return None;
            }

            match response.params() {
                ResponseParams::EditorView(params) => {
                    self.view = Some(params.view().clone());
                    self.scroll();
                    None
                }
                ResponseParams::EditorClipboard(params) => {
                    let mut clipboard = Clipboard::new();
                    clipboard.copy(params.text().to_string());
                    None
                }
                ResponseParams::EditorClosed(params) => {
                    self.closed = true;
                    match (params.error(), params.vim()) {
                        (Some(error), _) => {
                            self.error = Some(error.to_string());
                            None
                        }
                        // NOTE: Vim counts lines and columns from one.
                        (None, Some(at)) => Some(Effect::OpenVim(
                            VimArgsBuilder::new()
                                .path(&self.path)
                                .line(at.line + 1)
                                .column(at.column + 1)
                                .build(),
                        )),
                        (None, None) => Some(Effect::Quit),
                    }
                }
                _ => None,
            }
        }

        /// Return where the cursor is, which is nowhere at all when it is not shown.
        pub fn cursor(&self) -> Option<Location> {
            if self.pane_unfocused {
                return None;
            }

            let (row, column): (usize, usize) = self.own()?;
            Some(
                Location::builder()
                    .row(row)
                    .column(column - self.left)
                    .build(),
            )
        }

        /// Return which of the rows shown the cursor is in, and which column of the line it is in
        /// before the text is scrolled along.
        fn own(&self) -> Option<(usize, usize)> {
            let view: &EditorView = self.view.as_ref()?;
            let at: Position = view.own()?.at();
            let row: usize = at.line.checked_sub(view.top())?;
            let line: &str = view.lines().get(row)?;
            Some((row, Shown::of(line).column(at.column)))
        }

        /// Scroll the text along only as far as it takes for the column the cursor is in to be
        /// shown.
        fn scroll(&mut self) {
            let column: usize = match self.own() {
                Some((_, column)) => column,
                None => return,
            };

            if column < self.left {
                self.left = column;
            } else if column >= self.left + self.size.columns {
                self.left = (column + 1).saturating_sub(self.size.columns);
            }
        }

        /// Draw the text.
        pub fn render_text(&self, size: Size) -> Fabric {
            if let Some(error) = &self.error {
                return Fabric::center(error, size);
            }

            let view: &EditorView = match &self.view {
                Some(view) => view,
                None => return Fabric::new(size),
            };

            let left: usize = self.left;

            let mut yarns: Vec<Yarn> = Vec::new();
            for line in view.lines().iter().take(size.rows) {
                let mut cells: Vec<Cell> = Cell::all(&Shown::of(line).text);
                let kept: usize = cells.len().saturating_sub(left);
                Cell::keep_last(&mut cells, kept);
                let mut yarn: Yarn = Yarn::from(cells);
                yarn.resize(size.columns);
                yarns.push(yarn);
            }
            let mut fabric: Fabric = Fabric::from(yarns);
            fabric.pad_bottom(size.rows);

            for (row, (line, runs)) in view.lines().iter().zip(view.runs()).enumerate() {
                self.color(&mut fabric, row, line, runs, left);
            }

            // NOTE: Everyone else's cursor is dark yellow, and gray while what is typed goes to
            // another pane.
            let others: Color = match self.pane_unfocused {
                true => Color::UnfocusedHighlight,
                false => Color::OtherCursor,
            };
            for cursor in view.cursors() {
                let own: bool = cursor.cursor() == view.cursor();
                if own && cursor.mode().selects() {
                    self.select(&mut fabric, view, cursor, left);
                }
                if !own {
                    self.mark(&mut fabric, view, cursor.at(), left, others);
                }
            }

            // NOTE: The cursor is drawn as a yellow cell rather than being the one the terminal
            // shows, since the color of that one is the terminal's to pick. It goes gray while a
            // command or search is being typed, since what is typed is going to it then, and white
            // while the pane is not focused, so that it still stands apart from everyone else's.
            if let Some(own) = view.own() {
                let background: Color = match (self.pane_unfocused, own.mode()) {
                    (true, _) => Color::InvertedBackground,
                    (false, EditorMode::Command | EditorMode::Search) => Color::UnfocusedHighlight,
                    (false, _) => Color::Highlight,
                };
                self.mark(&mut fabric, view, own.at(), left, background);
            }

            fabric
        }

        /// Color each piece of the line in the row the way code is.
        fn color(&self, fabric: &mut Fabric, row: usize, line: &str, runs: &[Run], left: usize) {
            if row >= fabric.size().rows {
                return;
            }

            let shown: Shown = Shown::of(line);
            let columns: usize = fabric.size().columns;

            let mut start: usize = 0;
            for run in runs {
                let end: usize = start + run.chars;
                if let Some(color) = Color::of_code(run.kind) {
                    let (from, to): (usize, usize) = (
                        shown.column(start).saturating_sub(left).min(columns),
                        shown.column(end).saturating_sub(left).min(columns),
                    );
                    if from < to {
                        fabric.set_color(Some(color.into()), row, from..to);
                    }
                }
                start = end;
            }
        }

        /// Color what the cursor has selected.
        fn select(
            &self,
            fabric: &mut Fabric,
            view: &EditorView,
            cursor: &EditorCursor,
            left: usize,
        ) {
            let anchor: Position = cursor.anchor().unwrap_or(cursor.at());
            let (first, last): (Position, Position) =
                (anchor.min(cursor.at()), anchor.max(cursor.at()));
            let columns: usize = fabric.size().columns;

            for (row, line) in view.lines().iter().enumerate().take(fabric.size().rows) {
                let number: usize = view.top() + row;
                if number < first.line || number > last.line {
                    continue;
                }

                let shown: Shown = Shown::of(line);
                let (from, to): (usize, usize) = match cursor.mode() {
                    EditorMode::VisualLine => (0, shown.width().max(1)),
                    EditorMode::VisualBlock => {
                        let from: usize = anchor.column.min(cursor.at().column);
                        let to: usize = anchor.column.max(cursor.at().column) + 1;
                        (shown.column(from), shown.column(to))
                    }
                    _ => {
                        let from: usize = match number == first.line {
                            true => first.column,
                            false => 0,
                        };
                        // NOTE: The newline at the end of a line is selected along with it, and is
                        // shown as the column after the last character.
                        let to: usize = match number == last.line {
                            true => last.column + 1,
                            false => shown.chars() + 1,
                        };
                        (shown.column(from), shown.column(to))
                    }
                };

                let (from, to): (usize, usize) = (
                    from.saturating_sub(left).min(columns),
                    to.saturating_sub(left).min(columns),
                );
                if from < to {
                    fabric.set_background(Some(Color::Highlight.into()), row, from..to);
                    fabric.set_color(Some(Color::InvertedText.into()), row, from..to);
                }
            }
        }

        /// Mark where a cursor is in the background color.
        fn mark(
            &self,
            fabric: &mut Fabric,
            view: &EditorView,
            at: Position,
            left: usize,
            background: Color,
        ) {
            let row: usize = match at.line.checked_sub(view.top()) {
                Some(row) if row < fabric.size().rows => row,
                _ => return,
            };
            let line: &str = match view.lines().get(row) {
                Some(line) => line,
                None => return,
            };

            let column: usize = match Shown::of(line).column(at.column).checked_sub(left) {
                Some(column) if column < fabric.size().columns => column,
                _ => return,
            };
            fabric.set_background(Some(background.into()), row, column..column + 1);
            fabric.set_color(Some(Color::InvertedText.into()), row, column..column + 1);
        }
    }

    impl Stateful<Action, Effect> for State {
        fn perform(&mut self, action: Action) -> Option<Effect> {
            match action {
                Action::Press(key) => self.press(key),
                Action::Paste(text) => self.paste(text),
                Action::Resize(size) => self.resize(size),
                Action::Respond(response) => self.respond(*response),
                Action::SetFocus { focused } => {
                    self.pane_unfocused = !focused;
                    None
                }
            }
        }
    }

    impl FooterInfo for State {
        fn text(&self) -> String {
            if let Some(error) = &self.error {
                return error.clone();
            }

            let view: &EditorView = match &self.view {
                Some(view) => view,
                None => return String::from("Opening..."),
            };

            match view.message() {
                Some(message) => message.to_string(),
                None => String::new(),
            }
        }

        /// Return where the cursor is and then the mode, so that the mode is just before the name.
        fn position(&self) -> String {
            let mode: &str = match self.mode() {
                Some(EditorMode::Insert) => "insert",
                Some(EditorMode::Visual) => "visual",
                Some(EditorMode::VisualLine) => "visual line",
                Some(EditorMode::VisualBlock) => "visual block",
                Some(EditorMode::Normal) => "normal",
                Some(EditorMode::Command) => "command",
                Some(EditorMode::Search) => "search",
                None => "",
            };

            let at: String = match self.view.as_ref().and_then(EditorView::own) {
                Some(own) => format!("{}:{}", own.at().line + 1, own.at().column + 1),
                None => String::new(),
            };

            [at.as_str(), mode]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<&str>>()
                .join("  ")
        }
    }

    /// A line the way it is shown, with its tabs spread out to the next tab stop.
    struct Shown {
        /// The text which is shown.
        text: String,
        /// The column each character of the line starts in, along with the column after the last.
        columns: Vec<usize>,
    }

    impl Shown {
        /// Return the line the way it is shown.
        fn of(line: &str) -> Self {
            let mut text: String = String::with_capacity(line.len());
            let mut columns: Vec<usize> = Vec::with_capacity(line.len() + 1);
            let mut column: usize = 0;

            for character in line.chars() {
                columns.push(column);
                match character {
                    '\t' => {
                        let spaces: usize = TAB_STOP - column % TAB_STOP;
                        text.extend(std::iter::repeat_n(' ', spaces));
                        column += spaces;
                    }
                    character => {
                        text.push(character);
                        column += Cell::columns(character.encode_utf8(&mut [0; 4]));
                    }
                }
            }
            columns.push(column);

            Self { text, columns }
        }

        /// Return how many characters the line has.
        fn chars(&self) -> usize {
            self.columns.len() - 1
        }

        /// Return how many columns the line is shown in.
        fn width(&self) -> usize {
            self.columns[self.columns.len() - 1]
        }

        /// Return the column the character starts in, counting a column for each character past
        /// the end of the line.
        fn column(&self, character: usize) -> usize {
            match self.columns.get(character) {
                Some(column) => *column,
                None => self.width() + character - self.chars(),
            }
        }
    }
}
use state::State;

/// Contains the [`Action`] enum.
mod action {
    use insh_api::{EditorKey, Response};
    use rend::Size;

    /// An action the editor performs.
    pub enum Action {
        /// Press a key.
        Press(EditorKey),
        /// Paste text.
        Paste(String),
        /// Take note of how big the editor is.
        Resize(Size),
        /// Take note of what the daemon said.
        Respond(Box<Response>),
        /// Take note of whether what is typed goes to the pane it is in.
        SetFocus {
            /// Whether it does.
            focused: bool,
        },
    }
}
use action::Action;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Editor, Effect, Props};
    use crate::color::Color;
    use crate::components::common::{Hover, PaneContentsEvent as Event};

    use highlighter::{Kind, Run};
    use insh_api::{
        EditorClosedResponseParams, EditorCursor, EditorKey, EditorMode, EditorView,
        EditorViewResponseParams, Position, Request, RequestParams, Response, ResponseParams,
    };
    use rend::{Fabric, Location, Size};
    use term::{Key, KeyEvent, KeyMods, TermEvent};
    use til::Component;

    use test_case::test_case;
    use uuid::Uuid;

    /// The client's own cursor.
    const CURSOR: Uuid = Uuid::from_u128(1);

    /// How big the editor is.
    const SIZE: Size = Size {
        rows: 4,
        columns: 20,
    };

    /// Return an editor which has been shown the lines with its cursor at the position.
    fn showing(lines: &[&str], at: Position) -> (Editor, Request) {
        let mut editor: Editor = Editor::new(
            Props::builder()
                .path(PathBuf::from("/tmp/file"))
                .size(SIZE)
                .build(),
        );
        let open: Request = editor.open();

        let view: EditorView = EditorView::builder()
            .path(PathBuf::from("/tmp/file"))
            .cursor(CURSOR)
            .top(0)
            .lines(lines.iter().map(|line| line.to_string()).collect())
            .line_count(lines.len())
            .cursors(vec![EditorCursor::builder()
                .cursor(CURSOR)
                .at(at)
                .mode(EditorMode::Normal)
                .build()])
            .build();
        editor.handle(Event::Response(
            Response::builder()
                .uuid(*open.uuid())
                .params(ResponseParams::EditorView(
                    EditorViewResponseParams::builder().view(view).build(),
                ))
                .build(),
        ));

        (editor, open)
    }

    /// Show the editor its cursor moved to the position.
    fn moving(editor: &mut Editor, open: &Request, lines: &[&str], at: Position) {
        let view: EditorView = EditorView::builder()
            .path(PathBuf::from("/tmp/file"))
            .cursor(CURSOR)
            .top(0)
            .lines(lines.iter().map(|line| line.to_string()).collect())
            .line_count(lines.len())
            .cursors(vec![EditorCursor::builder()
                .cursor(CURSOR)
                .at(at)
                .mode(EditorMode::Normal)
                .build()])
            .build();
        editor.handle(Event::Response(
            Response::builder()
                .uuid(*open.uuid())
                .params(ResponseParams::EditorView(
                    EditorViewResponseParams::builder().view(view).build(),
                ))
                .build(),
        ));
    }

    #[test_case(28, 18, "klmnopqrstuvwxyz0123"; "one to the left")]
    #[test_case(10, 0, "klmnopqrstuvwxyz0123"; "up against the left")]
    #[test_case(9, 0, "jklmnopqrstuvwxyz012"; "past the left")]
    fn test_the_text_is_only_scrolled_once_the_cursor_is_against_the_edge(
        column: usize,
        expected_column: usize,
        expected_text: &str,
    ) {
        let lines: &[&str] = &["abcdefghijklmnopqrstuvwxyz0123"];
        let (mut editor, open): (Editor, Request) = showing(lines, Position::new(0, 29));

        moving(&mut editor, &open, lines, Position::new(0, column));
        let fabric: Fabric = editor.render(SIZE);

        assert_eq!(fabric.text(1, 0..20), expected_text);
        assert_eq!(
            editor.cursor(),
            Some(Location::builder().row(1).column(expected_column).build())
        );
    }

    #[test]
    fn test_the_cursor_is_after_a_tab_spread_out() {
        let (editor, _): (Editor, Request) = showing(&["\tab"], Position::new(0, 1));

        let fabric: Fabric = editor.render(SIZE);

        assert_eq!(fabric.text(1, 0..10), "        ab");
        assert_eq!(
            editor.cursor(),
            Some(Location::builder().row(1).column(8).build())
        );
        assert_eq!(fabric.cursor(), None);
        assert_eq!(
            fabric.spot(1, 8).style().background(),
            Some(Color::Highlight.into())
        );
    }

    #[test]
    fn test_the_file_is_shown_at_the_top() {
        let (editor, _): (Editor, Request) = showing(&["abc"], Position::new(0, 0));

        let fabric: Fabric = editor.render(SIZE);

        assert_eq!(fabric.text(0, 0..9), "/tmp/file");
        assert_eq!(fabric.text(1, 0..3), "abc");
    }

    #[test_case(0, Color::LabelHighlight; "the colon")]
    #[test_case(1, Color::Highlight; "what is typed")]
    #[test_case(15, Color::Highlight; "the rest of the row")]
    fn test_a_command_is_drawn_the_way_the_prompt_is_and_grays_the_cursor(
        column: usize,
        expected: Color,
    ) {
        let (mut editor, open): (Editor, Request) = showing(&["abc"], Position::new(0, 0));
        let view: EditorView = EditorView::builder()
            .path(PathBuf::from("/tmp/file"))
            .cursor(CURSOR)
            .top(0)
            .lines(vec![String::from("abc")])
            .line_count(1)
            .cursors(vec![EditorCursor::builder()
                .cursor(CURSOR)
                .at(Position::new(0, 0))
                .mode(EditorMode::Command)
                .build()])
            .command(Some(String::from("wq")))
            .build();
        editor.handle(Event::Response(
            Response::builder()
                .uuid(*open.uuid())
                .params(ResponseParams::EditorView(
                    EditorViewResponseParams::builder().view(view).build(),
                ))
                .build(),
        ));

        let fabric: Fabric = editor.render(SIZE);

        assert_eq!(fabric.text(SIZE.rows - 1, 0..3), ":wq");
        assert_eq!(
            fabric.spot(SIZE.rows - 1, column).style().background(),
            Some(expected.into())
        );
        assert_eq!(
            fabric.spot(1, 0).style().background(),
            Some(Color::UnfocusedHighlight.into())
        );
    }

    #[test_case(true, Color::Highlight, Color::OtherCursor; "while the pane is focused")]
    #[test_case(false, Color::InvertedBackground, Color::UnfocusedHighlight; "while it is not")]
    fn test_the_cursor_is_white_and_the_others_gray_while_the_pane_is_not_focused(
        focused: bool,
        own: Color,
        other: Color,
    ) {
        let (mut editor, open): (Editor, Request) = showing(&["abc"], Position::new(0, 0));
        let view: EditorView = EditorView::builder()
            .path(PathBuf::from("/tmp/file"))
            .cursor(CURSOR)
            .top(0)
            .lines(vec![String::from("abc")])
            .line_count(1)
            .cursors(vec![
                EditorCursor::builder()
                    .cursor(CURSOR)
                    .at(Position::new(0, 0))
                    .mode(EditorMode::Normal)
                    .build(),
                EditorCursor::builder()
                    .cursor(Uuid::from_u128(2))
                    .at(Position::new(0, 2))
                    .mode(EditorMode::Normal)
                    .build(),
            ])
            .build();
        editor.handle(Event::Response(
            Response::builder()
                .uuid(*open.uuid())
                .params(ResponseParams::EditorView(
                    EditorViewResponseParams::builder().view(view).build(),
                ))
                .build(),
        ));
        editor.handle(Event::SetFocus { focused });

        let fabric: Fabric = editor.render(SIZE);

        assert_eq!(fabric.spot(1, 0).style().background(), Some(own.into()));
        assert_eq!(fabric.spot(1, 2).style().background(), Some(other.into()));
    }

    #[test_case(false, SIZE.rows - 1, Some(Hover::Whitened); "the footer while the pane is not focused")]
    #[test_case(false, 0, None; "the file bar while the pane is not focused")]
    #[test_case(false, 1, None; "the text while the pane is not focused")]
    #[test_case(true, SIZE.rows - 1, None; "the footer while the pane is focused")]
    fn test_only_the_footer_hovers_and_only_while_the_pane_is_not_focused(
        focused: bool,
        row: usize,
        expected: Option<Hover>,
    ) {
        let (mut editor, _): (Editor, Request) = showing(&["abc"], Position::new(0, 0));
        editor.handle(Event::SetFocus { focused });

        assert_eq!(editor.hovers(row), expected);
    }

    #[test_case(0, Some(Color::Keyword); "a keyword")]
    #[test_case(2, None; "a space")]
    #[test_case(8, Some(Color::Keyword); "a keyword after a tab spread out")]
    fn test_each_piece_of_the_code_is_colored(column: usize, expected: Option<Color>) {
        let (mut editor, open): (Editor, Request) = showing(&["fn x\tfn"], Position::new(0, 3));
        let view: EditorView = EditorView::builder()
            .path(PathBuf::from("/tmp/file"))
            .cursor(CURSOR)
            .top(0)
            .lines(vec![String::from("fn x\tfn")])
            .runs(vec![vec![
                Run {
                    chars: 2,
                    kind: Some(Kind::Keyword),
                },
                Run {
                    chars: 3,
                    kind: None,
                },
                Run {
                    chars: 2,
                    kind: Some(Kind::Keyword),
                },
            ]])
            .line_count(1)
            .cursors(vec![EditorCursor::builder()
                .cursor(CURSOR)
                .at(Position::new(0, 3))
                .mode(EditorMode::Normal)
                .build()])
            .build();
        editor.handle(Event::Response(
            Response::builder()
                .uuid(*open.uuid())
                .params(ResponseParams::EditorView(
                    EditorViewResponseParams::builder().view(view).build(),
                ))
                .build(),
        ));

        let fabric: Fabric = editor.render(SIZE);

        assert_eq!(
            fabric.spot(1, column).style().color(),
            expected.map(|color| color.into())
        );
    }

    #[test]
    fn test_a_key_is_sent_for_the_cursor() {
        let (mut editor, _): (Editor, Request) = showing(&["abc"], Position::new(0, 0));

        let effect: Option<Effect> =
            editor.handle(Event::TermEvent(TermEvent::KeyEvent(KeyEvent {
                key: Key::Char('x'),
                mods: KeyMods::NONE,
            })));

        match effect {
            Some(Effect::Request(request)) => match request.params() {
                RequestParams::EditorKey(params) => {
                    assert_eq!(*params.cursor(), CURSOR);
                    assert_eq!(params.key(), EditorKey::Char('x'));
                }
                _ => panic!("Expected the key to be sent."),
            },
            _ => panic!("Expected a request."),
        }
    }

    #[test]
    fn test_pasted_text_is_sent_for_the_cursor() {
        let (mut editor, _): (Editor, Request) = showing(&["abc"], Position::new(0, 0));

        let effect: Option<Effect> =
            editor.handle(Event::TermEvent(TermEvent::Paste(String::from("xy"))));

        match effect {
            Some(Effect::Request(request)) => match request.params() {
                RequestParams::PasteEditor(params) => {
                    assert_eq!(*params.cursor(), CURSOR);
                    assert_eq!(params.text(), "xy");
                }
                _ => panic!("Expected the text to be sent."),
            },
            _ => panic!("Expected a request."),
        }
    }

    #[test]
    fn test_a_search_is_drawn_where_the_footer_is() {
        let (mut editor, open): (Editor, Request) = showing(&["abc"], Position::new(0, 0));
        let view: EditorView = EditorView::builder()
            .path(PathBuf::from("/tmp/file"))
            .cursor(CURSOR)
            .top(0)
            .lines(vec![String::from("abc")])
            .line_count(1)
            .cursors(vec![EditorCursor::builder()
                .cursor(CURSOR)
                .at(Position::new(0, 0))
                .mode(EditorMode::Search)
                .build()])
            .search(Some(String::from("bc")))
            .build();
        editor.handle(Event::Response(
            Response::builder()
                .uuid(*open.uuid())
                .params(ResponseParams::EditorView(
                    EditorViewResponseParams::builder().view(view).build(),
                ))
                .build(),
        ));

        let fabric: Fabric = editor.render(SIZE);

        assert_eq!(fabric.text(SIZE.rows - 1, 0..3), "/bc");
        assert_eq!(
            fabric.spot(1, 0).style().background(),
            Some(Color::UnfocusedHighlight.into())
        );
    }

    #[test]
    fn test_opening_says_where_the_cursor_starts() {
        let editor: Editor = Editor::new(
            Props::builder()
                .path(PathBuf::from("/tmp/file"))
                .size(SIZE)
                .at(Some(Position::new(4, 2)))
                .build(),
        );

        match editor.open().params() {
            RequestParams::Edit(params) => assert_eq!(params.at(), Some(Position::new(4, 2))),
            _ => panic!("Expected the file to be edited."),
        }
    }

    #[test]
    fn test_closing_names_the_request_the_file_was_edited_on() {
        let (editor, open): (Editor, Request) = showing(&["abc"], Position::new(0, 0));

        let close: Request = editor.close().unwrap();

        match close.params() {
            RequestParams::CloseEditor(params) => assert_eq!(params.edit(), open.uuid()),
            _ => panic!("Expected the cursor to be closed."),
        }
    }

    #[test]
    fn test_nothing_is_closed_once_the_daemon_has_closed_it() {
        let (mut editor, open): (Editor, Request) = showing(&["abc"], Position::new(0, 0));
        editor.handle(Event::Response(
            Response::builder()
                .uuid(*open.uuid())
                .last(true)
                .params(ResponseParams::EditorClosed(
                    EditorClosedResponseParams::builder().build(),
                ))
                .build(),
        ));

        assert!(editor.close().is_none());
    }

    #[test]
    fn test_being_closed_quits() {
        let (mut editor, open): (Editor, Request) = showing(&["abc"], Position::new(0, 0));

        let effect: Option<Effect> = editor.handle(Event::Response(
            Response::builder()
                .uuid(*open.uuid())
                .last(true)
                .params(ResponseParams::EditorClosed(
                    EditorClosedResponseParams::builder().build(),
                ))
                .build(),
        ));

        assert!(matches!(effect, Some(Effect::Quit)));
    }

    #[test]
    fn test_being_closed_for_vim_opens_vim_where_the_cursor_was() {
        let (mut editor, open): (Editor, Request) = showing(&["abc"], Position::new(0, 0));

        let effect: Option<Effect> = editor.handle(Event::Response(
            Response::builder()
                .uuid(*open.uuid())
                .last(true)
                .params(ResponseParams::EditorClosed(
                    EditorClosedResponseParams::builder()
                        .vim(Some(Position::new(2, 3)))
                        .build(),
                ))
                .build(),
        ));

        match effect {
            Some(Effect::OpenVim(args)) => {
                assert_eq!(
                    args.path().as_deref(),
                    Some(PathBuf::from("/tmp/file").as_path())
                );
                assert_eq!(args.line(), Some(3));
                assert_eq!(args.column(), Some(4));
            }
            _ => panic!("Expected vim to be opened."),
        }
    }
}
