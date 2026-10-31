/*!
The requests and responses which carry the panes, windows and sessions a client is showing.

A client does not own any of this. The daemon runs the programs, keeps the screens they write on
and works out where every pane goes; a client says what was typed and draws what it is sent. That
is what lets a client go away and come back to find its programs still running, and what lets more
than one of them show the one session at once.
*/
use std::path::PathBuf;

use rend::{Location, Size};
use terminal_emulator::{Line, Modes};

use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// Which way a pane is split in two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Split {
    /// The new pane goes alongside it, so the two of them are side by side.
    Beside,
    /// The new pane goes under it, so one of them is above the other.
    Below,
}

/// What a pane holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneKind {
    /// One of insh's own components, which the client draws for itself.
    ///
    /// The daemon keeps one of these only so that it has somewhere to be in the window. There is
    /// no program running in it and no screen to send, because there is nothing in it which the
    /// daemon knows anything about.
    Component,
    /// A program running on a pseudoterminal, whose screen the daemon keeps.
    Program,
}

/// Where a pane is in the window it is in, and how big it is.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct PaneLayout {
    /// Which pane it is.
    pane: Uuid,
    /// Where its top left corner is in the window.
    at: Location,
    /// How big it is.
    size: Size,
    /// What it holds.
    #[builder(default = PaneKind::Component)]
    kind: PaneKind,
}

impl PaneLayout {
    /// Return which pane it is.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }

    /// Return where its top left corner is in the window.
    pub fn at(&self) -> Location {
        return self.at;
    }

    /// Return how big it is.
    pub fn size(&self) -> Size {
        return self.size;
    }

    /// Return what it holds.
    pub fn kind(&self) -> PaneKind {
        return self.kind;
    }
}

/// One of the windows of a session.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct Window {
    /// Which window it is.
    window: Uuid,
    /// What it is called.
    #[builder(setter(into))]
    name: String,
}

impl Window {
    /// Return which window it is.
    pub fn window(&self) -> &Uuid {
        return &self.window;
    }

    /// Return what it is called.
    pub fn name(&self) -> &str {
        return &self.name;
    }
}

/// Everything a client needs in order to know what to draw where.
///
/// Only the panes of the window which is being looked at are here, since those are the only ones
/// which are drawn. The rest of the windows are only named, which is all the footer shows of them.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct Layout {
    /// Which session it is.
    session: Uuid,
    /// What the session is called.
    #[builder(setter(into))]
    name: String,
    /// The windows of it, in the order they are shown in.
    windows: Vec<Window>,
    /// Which window is being looked at.
    window: Uuid,
    /// Where each pane of that window is.
    panes: Vec<PaneLayout>,
    /// Which pane what is typed goes to.
    pane: Uuid,
    /// Whether one pane has been made to fill the window.
    #[builder(default)]
    zoomed: bool,
    /// Whether what is typed goes to every pane of the window at once.
    #[builder(default)]
    synchronized: bool,
}

impl Layout {
    /// Return which session it is.
    pub fn session(&self) -> &Uuid {
        return &self.session;
    }

    /// Return what the session is called.
    pub fn name(&self) -> &str {
        return &self.name;
    }

    /// Return the windows of it, in the order they are shown in.
    pub fn windows(&self) -> &[Window] {
        return &self.windows;
    }

    /// Return which window is being looked at.
    pub fn window(&self) -> &Uuid {
        return &self.window;
    }

    /// Return where each pane of that window is.
    pub fn panes(&self) -> &[PaneLayout] {
        return &self.panes;
    }

    /// Return which pane what is typed goes to.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }

    /// Return whether one pane has been made to fill the window.
    pub fn zoomed(&self) -> bool {
        return self.zoomed;
    }

    /// Return whether what is typed goes to every pane of the window at once.
    pub fn synchronized(&self) -> bool {
        return self.synchronized;
    }
}

/// One row of the screen of a pane.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct PaneRow {
    /// Which row of the pane it is.
    row: usize,
    /// What is written in it.
    line: Line,
}

impl PaneRow {
    /// Return which row of the pane it is.
    pub fn row(&self) -> usize {
        return self.row;
    }

    /// Return what is written in it.
    pub fn line(&self) -> &Line {
        return &self.line;
    }
}

/// What has changed about the screen of a pane.
///
/// Only the rows which have been written on are here, so that a program writing one character does
/// not have its whole screen sent again.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct PaneScreen {
    /// Which pane it is.
    pane: Uuid,
    /// The rows which have been written on.
    rows: Vec<PaneRow>,
    /// Where the cursor is, which is nowhere at all when it is not shown.
    #[builder(default)]
    cursor: Option<Location>,
    /// The ways the screen has been asked to behave, which is what says how what is typed is sent
    /// to the program.
    modes: Modes,
    /// What the program has asked the title to be.
    #[builder(default)]
    title: Option<String>,
    /// How many times the bell has rung since the last of these.
    #[builder(default)]
    bells: usize,
    /// How many rows back through what has scrolled off the top is being looked.
    #[builder(default)]
    looking_back: usize,
}

impl PaneScreen {
    /// Return which pane it is.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }

    /// Return the rows which have been written on.
    pub fn rows(&self) -> &[PaneRow] {
        return &self.rows;
    }

    /// Return where the cursor is, which is nowhere at all when it is not shown.
    pub fn cursor(&self) -> Option<Location> {
        return self.cursor;
    }

    /// Return the ways the screen has been asked to behave.
    pub fn modes(&self) -> Modes {
        return self.modes;
    }

    /// Return what the program has asked the title to be.
    pub fn title(&self) -> Option<&str> {
        return self.title.as_deref();
    }

    /// Return how many times the bell has rung since the last of these.
    pub fn bells(&self) -> usize {
        return self.bells;
    }

    /// Return how many rows back through what has scrolled off the top is being looked.
    pub fn looking_back(&self) -> usize {
        return self.looking_back;
    }
}

/// One of the sessions there are.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Which session it is.
    session: Uuid,
    /// What it is called.
    #[builder(setter(into))]
    name: String,
    /// The windows of it, in the order they are shown in.
    windows: Vec<WindowSummary>,
    /// How many clients are showing it.
    clients: usize,
    /// Whether the client which asked is the one showing it.
    #[builder(default)]
    showing: bool,
}

/// One of the windows of a session which is being listed.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct WindowSummary {
    /// Which window it is.
    window: Uuid,
    /// What it is called.
    #[builder(setter(into))]
    name: String,
    /// The panes of it, in the order they are shown in.
    panes: Vec<PaneSummary>,
    /// Whether it is the one being looked at in its session.
    #[builder(default)]
    active: bool,
}

impl WindowSummary {
    /// Return which window it is.
    pub fn window(&self) -> &Uuid {
        return &self.window;
    }

    /// Return what it is called.
    pub fn name(&self) -> &str {
        return &self.name;
    }

    /// Return the panes of it, in the order they are shown in.
    pub fn panes(&self) -> &[PaneSummary] {
        return &self.panes;
    }

    /// Return whether it is the one being looked at in its session.
    pub fn active(&self) -> bool {
        return self.active;
    }
}

/// One of the panes of a window which is being listed.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct PaneSummary {
    /// Which pane it is.
    pane: Uuid,
    /// Whether what is typed in its window goes to it.
    #[builder(default)]
    active: bool,
}

impl PaneSummary {
    /// Return which pane it is.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }

    /// Return whether what is typed in its window goes to it.
    pub fn active(&self) -> bool {
        return self.active;
    }
}

impl SessionSummary {
    /// Return which session it is.
    pub fn session(&self) -> &Uuid {
        return &self.session;
    }

    /// Return what it is called.
    pub fn name(&self) -> &str {
        return &self.name;
    }

    /// Return the windows of it, in the order they are shown in.
    pub fn windows(&self) -> &[WindowSummary] {
        return &self.windows;
    }

    /// Return how many clients are showing it.
    pub fn clients(&self) -> usize {
        return self.clients;
    }

    /// Return whether the client which asked is the one showing it.
    pub fn showing(&self) -> bool {
        return self.showing;
    }
}

/// Request parameters for showing a session.
///
/// The responses to this go on for as long as the client is showing it rather than ending, the way
/// the ones carrying log records do: everything which happens to the session is sent as another
/// response to this one request.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct AttachRequestParams {
    /// Which session to show, or the one which was last used when there is none.
    #[builder(default)]
    session: Option<Uuid>,
    /// How big the terminal the client is in is.
    size: Size,
}

impl AttachRequestParams {
    /// Return which session to show, or the one which was last used when there is none.
    pub fn session(&self) -> Option<&Uuid> {
        return self.session.as_ref();
    }

    /// Return how big the terminal the client is in is.
    pub fn size(&self) -> Size {
        return self.size;
    }
}

/// Request parameters for no longer showing a session.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct DetachRequestParams {}

/// Request parameters for sending what was typed to a pane.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct WritePaneRequestParams {
    /// Which pane to send it to.
    pane: Uuid,
    /// The bytes to send.
    bytes: Vec<u8>,
}

impl WritePaneRequestParams {
    /// Return which pane to send it to.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }

    /// Return the bytes to send.
    pub fn bytes(&self) -> &[u8] {
        return &self.bytes;
    }
}

/// Request parameters for splitting a pane in two.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct SplitPaneRequestParams {
    /// Which pane to split.
    pane: Uuid,
    /// Which way to split it.
    split: Split,
    /// The directory to run the program of the new pane in.
    #[builder(default)]
    dir: Option<PathBuf>,
}

impl SplitPaneRequestParams {
    /// Return which pane to split.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }

    /// Return which way to split it.
    pub fn split(&self) -> Split {
        return self.split;
    }

    /// Return the directory to run the program of the new pane in.
    pub fn dir(&self) -> Option<&PathBuf> {
        return self.dir.as_ref();
    }
}

/// Request parameters for running a program in a pane.
///
/// The pane goes from being one which a client draws for itself to being one with a program in it,
/// and goes back to being the first sort when the program has finished.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct RunInPaneRequestParams {
    /// Which pane to run it in.
    pane: Uuid,
    /// The program to run.
    #[builder(setter(into))]
    program: String,
    /// The arguments to give it.
    #[builder(default)]
    args: Vec<String>,
    /// The directory to run it in.
    #[builder(default)]
    dir: Option<PathBuf>,
}

impl RunInPaneRequestParams {
    /// Return which pane to run it in.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }

    /// Return the program to run.
    pub fn program(&self) -> &str {
        return &self.program;
    }

    /// Return the arguments to give it.
    pub fn args(&self) -> &[String] {
        return &self.args;
    }

    /// Return the directory to run it in.
    pub fn dir(&self) -> Option<&PathBuf> {
        return self.dir.as_ref();
    }
}

/// Request parameters for making one pane fill the window, or putting it back.
///
/// The panes of the window are left arranged the way they were, so putting it back shows them
/// where they were before.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct ZoomPaneRequestParams {
    /// The pane to make fill the window.
    pane: Uuid,
}

impl ZoomPaneRequestParams {
    /// Return the pane to make fill the window.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }
}

/// Request parameters for swapping a pane with the one before or after it.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct SwapPaneRequestParams {
    /// Which pane to swap.
    pane: Uuid,
    /// Whether to swap it with the one after it rather than the one before it.
    after: bool,
}

impl SwapPaneRequestParams {
    /// Return which pane to swap.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }

    /// Return whether to swap it with the one after it.
    pub fn after(&self) -> bool {
        return self.after;
    }
}

/// Request parameters for looking back through what has scrolled off the top of a pane.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct ScrollPaneRequestParams {
    /// Which pane to look back through.
    pane: Uuid,
    /// How many rows further back to look, or fewer rows back when it is negative.
    rows: isize,
}

impl ScrollPaneRequestParams {
    /// Return which pane to look back through.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }

    /// Return how many rows further back to look.
    pub fn rows(&self) -> isize {
        return self.rows;
    }
}

/// Request parameters for calling the session a client is showing something else.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct RenameSessionRequestParams {
    /// What to call it.
    #[builder(setter(into))]
    name: String,
}

impl RenameSessionRequestParams {
    /// Return what to call it.
    pub fn name(&self) -> &str {
        return &self.name;
    }
}

/// Request parameters for calling a window something else.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct RenameWindowRequestParams {
    /// Which window to rename, or the one being looked at when there is none.
    #[builder(default)]
    window: Option<Uuid>,
    /// What to call it.
    #[builder(setter(into))]
    name: String,
}

impl RenameWindowRequestParams {
    /// Return which window to rename.
    pub fn window(&self) -> Option<&Uuid> {
        return self.window.as_ref();
    }

    /// Return what to call it.
    pub fn name(&self) -> &str {
        return &self.name;
    }
}

/// Request parameters for sending what is typed to every pane of the window at once, or stopping.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct SyncPanesRequestParams {}

/// Request parameters for moving the line on the far side of a pane.
///
/// What the pane gains the one on the other side of the line loses, so the rest of the window
/// stays where it is.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct DragPaneRequestParams {
    /// The pane whose far side the line is on.
    pane: Uuid,
    /// Which of its sides, where beside is the one on the right and below is the one underneath.
    split: Split,
    /// How many rows or columns to move it, which is up or left when it is negative.
    by: isize,
}

impl DragPaneRequestParams {
    /// Return the pane whose far side the line is on.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }

    /// Return which of its sides the line is on.
    pub fn split(&self) -> Split {
        return self.split;
    }

    /// Return how many rows or columns to move it.
    pub fn by(&self) -> isize {
        return self.by;
    }
}

/// Request parameters for sending what is typed to another pane.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct SelectPaneRequestParams {
    /// Which pane to send it to.
    pane: Uuid,
}

impl SelectPaneRequestParams {
    /// Return which pane to send it to.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }
}

/// Request parameters for closing a pane.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct KillPaneRequestParams {
    /// Which pane to close.
    pane: Uuid,
}

impl KillPaneRequestParams {
    /// Return which pane to close.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }
}

/// Request parameters for making a new window.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct NewWindowRequestParams {
    /// What to call it, or a name made up from the ones already taken when there is none.
    #[builder(default)]
    name: Option<String>,
    /// The directory to run the program of its first pane in.
    #[builder(default)]
    dir: Option<PathBuf>,
}

impl NewWindowRequestParams {
    /// Return what to call it.
    pub fn name(&self) -> Option<&str> {
        return self.name.as_deref();
    }

    /// Return the directory to run the program of its first pane in.
    pub fn dir(&self) -> Option<&PathBuf> {
        return self.dir.as_ref();
    }
}

/// Request parameters for looking at another window.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct SelectWindowRequestParams {
    /// Which window to look at.
    window: Uuid,
}

impl SelectWindowRequestParams {
    /// Return which window to look at.
    pub fn window(&self) -> &Uuid {
        return &self.window;
    }
}

/// Request parameters for making a new session.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct NewSessionRequestParams {
    /// What to call it, or a name made up from the ones already taken when there is none.
    #[builder(default)]
    name: Option<String>,
    /// The directory to run the program of its first pane in.
    #[builder(default)]
    dir: Option<PathBuf>,
    /// Whether the client should show it rather than the one it is showing.
    #[builder(default = true)]
    attach: bool,
}

impl NewSessionRequestParams {
    /// Return what to call it.
    pub fn name(&self) -> Option<&str> {
        return self.name.as_deref();
    }

    /// Return the directory to run the program of its first pane in.
    pub fn dir(&self) -> Option<&PathBuf> {
        return self.dir.as_ref();
    }

    /// Return whether the client should show it rather than the one it is showing.
    pub fn attach(&self) -> bool {
        return self.attach;
    }
}

/// Request parameters for listing the sessions there are.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct ListSessionsRequestParams {}

/// Request parameters for the terminal a client is in having changed size.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct ResizeClientRequestParams {
    /// How big it is now.
    size: Size,
}

impl ResizeClientRequestParams {
    /// Return how big it is now.
    pub fn size(&self) -> Size {
        return self.size;
    }
}

/// What the commands which can be typed for the multiplexer are called.
///
/// These are shared so that the daemon, which is what suggests a command, and the client, which is
/// what works out which one was typed, never disagree about which there are.
pub const COMMAND_NAMES: [&str; 22] = [
    "change-mode",
    "detach",
    "focus-down",
    "focus-left",
    "focus-right",
    "focus-up",
    "kill-pane",
    "list-sessions",
    "new-session",
    "new-window",
    "rename-session",
    "rename-window",
    "resize-pane-down",
    "resize-pane-left",
    "resize-pane-right",
    "resize-pane-up",
    "split-horizontally",
    "split-vertically",
    "switch-pane-next",
    "switch-pane-previous",
    "synchronize-panes",
    "toggle-zoom",
];

/// Request parameters for suggesting a command.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct SuggestCommandRequestParams {
    /// The command so far.
    partial: String,
}

impl SuggestCommandRequestParams {
    /// Return the command so far.
    pub fn partial(&self) -> &str {
        &self.partial
    }
}

/// Request parameters for taking note that a command was run.
#[derive(Debug, TypedBuilder, Serialize, Deserialize)]
pub struct RunCommandRequestParams {
    /// The command which was run.
    command: String,
}

impl RunCommandRequestParams {
    /// Return the command which was run.
    pub fn command(&self) -> &str {
        &self.command
    }
}

/// Response parameters for suggesting a command.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct SuggestCommandResponseParams {
    /// The suggested command.
    suggestion: Option<String>,
}

impl SuggestCommandResponseParams {
    /// Return the suggested command.
    pub fn suggestion(&self) -> &Option<String> {
        &self.suggestion
    }
}

/// Response parameters for taking note that a command was run. There is nothing to say about it,
/// but a response is what says that the request was handled.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct RunCommandResponseParams {}

/// Response parameters for where the panes and windows are.
///
/// This is sent whenever anything about them changes: a pane being split or closed, another window
/// being looked at, or the terminal the client is in changing size.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct LayoutResponseParams {
    /// Where everything is.
    layout: Layout,
    /// The whole of the screen of every pane, which is sent when a client starts showing a session
    /// and when a pane changes size, and is nothing at all otherwise.
    #[builder(default)]
    screens: Vec<PaneScreen>,
}

impl LayoutResponseParams {
    /// Return where everything is.
    pub fn layout(&self) -> &Layout {
        return &self.layout;
    }

    /// Return the screens which came with it.
    pub fn screens(&self) -> &[PaneScreen] {
        return &self.screens;
    }
}

/// Response parameters for part of the screen of a pane having been written on.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct PaneScreenResponseParams {
    /// What has changed about it.
    screen: PaneScreen,
}

impl PaneScreenResponseParams {
    /// Return what has changed about it.
    pub fn screen(&self) -> &PaneScreen {
        return &self.screen;
    }
}

/// Response parameters for the program of a pane having finished.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct PaneExitedResponseParams {
    /// Which pane it was.
    pane: Uuid,
}

impl PaneExitedResponseParams {
    /// Return which pane it was.
    pub fn pane(&self) -> &Uuid {
        return &self.pane;
    }
}

/// Response parameters for a client no longer showing a session.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct DetachedResponseParams {}

/// Response parameters for the sessions there are.
#[derive(Debug, Clone, TypedBuilder, Serialize, Deserialize)]
pub struct SessionsResponseParams {
    /// The sessions.
    sessions: Vec<SessionSummary>,
}

impl SessionsResponseParams {
    /// Return the sessions.
    pub fn sessions(&self) -> &[SessionSummary] {
        return &self.sessions;
    }
}
