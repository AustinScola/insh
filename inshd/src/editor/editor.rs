//! Owns every file being edited, and tells the clients moving a cursor in one about it.

use std::collections::HashMap;
use std::fs::canonicalize;
use std::io::ErrorKind;
use std::ops::Range;
use std::path::{Path, PathBuf};

use super::buffer::{Asked, Buffer, Scroll};
use crate::contexted_request::ContextedRequest;
use crate::contexted_response::ContextedResponse;
use crate::disconnected_client::DisconnectedClient;
use crate::stop::Stop;

use highlighter::Run;
use insh_api::{
    EditorClipboardResponseParams, EditorClosedResponseParams, EditorKey, EditorMode, EditorView,
    EditorViewResponseParams, Position, RequestParams, Response, ResponseParams,
};
use rend::Size;

use crossbeam::channel::{select, Receiver, Sender};
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// Which client is moving a cursor, and what of the file it is shown.
#[derive(Debug, Clone, TypedBuilder)]
struct Viewer {
    /// The client.
    client: Uuid,
    /// The request whose responses tell it about the file.
    request: Uuid,
    /// The file the cursor is in.
    path: PathBuf,
    /// How much text it is shown.
    size: Size,
    /// Which line the first one it is shown is.
    #[builder(default)]
    top: usize,
}

/// Owns every file being edited, and tells the clients moving a cursor in one about it.
///
/// This is where the text of a file is kept rather than in a client, which is what lets more than
/// one cursor be in a file at once and each see what the others do to it. A cursor need not have a
/// client moving it at all; one which does has a viewer, which is what says what it is shown.
#[derive(TypedBuilder)]
pub struct Editor {
    /// A receiver of the requests which are about files being edited.
    requests_rx: Receiver<ContextedRequest>,
    /// A receiver of information about clients disconnecting.
    disconnected_clients_rx: Receiver<DisconnectedClient>,
    /// A sender of responses.
    contexted_responses_tx: Sender<ContextedResponse>,
    /// A receiver of a stop sentinel.
    stop_rx: Receiver<Stop>,

    /// The files being edited, by where each is.
    #[builder(setter(skip), default)]
    buffers: HashMap<PathBuf, Buffer>,
    /// The client moving each cursor which has one, by which cursor it is.
    #[builder(setter(skip), default)]
    viewers: HashMap<Uuid, Viewer>,
}

impl Editor {
    /// Run the editor.
    pub fn run(&mut self) {
        log::info!("Editor running.");

        loop {
            select! {
                recv(self.stop_rx) -> _stop => {
                    break;
                }
                recv(self.requests_rx) -> request => {
                    match request {
                        Ok(request) => self.handle_request(request),
                        Err(error) => {
                            log::error!("Error receiving a request: {}", error);
                        }
                    }
                }
                recv(self.disconnected_clients_rx) -> client => {
                    match client {
                        Ok(client) => self.leave(&client.client_uuid),
                        Err(error) => {
                            log::error!("Error receiving info about a disconnected client: {}", error);
                        }
                    }
                }
            }
        }

        log::info!("Editor stopping.");
    }

    /// Do what the request asks.
    fn handle_request(&mut self, request: ContextedRequest) {
        let client: Uuid = *request.client_uuid();
        let uuid: Uuid = *request.request().uuid();

        match request.request().params() {
            RequestParams::Edit(params) => {
                let at: Position = params.at().unwrap_or_default();
                self.edit(client, uuid, params.path(), params.size(), at);
            }
            RequestParams::EditorKey(params) => {
                let key: EditorKey = params.key();
                self.act(client, params.cursor(), |buffer, cursor| {
                    buffer.press(cursor, key)
                });
            }
            RequestParams::PasteEditor(params) => {
                self.act(client, params.cursor(), |buffer, cursor| {
                    buffer.paste_in(cursor, params.text());
                    None
                });
            }
            RequestParams::ResizeEditor(params) => {
                let cursor: Uuid = *params.cursor();
                match self.viewers.get_mut(&cursor) {
                    Some(viewer) if viewer.client == client => viewer.size = params.size(),
                    _ => return,
                }
                self.show(&cursor);
            }
            RequestParams::CloseEditor(params) => {
                let cursor: Option<Uuid> = self
                    .viewers
                    .iter()
                    .find(|(_, viewer)| viewer.client == client && viewer.request == *params.edit())
                    .map(|(cursor, _)| *cursor);
                if let Some(cursor) = cursor {
                    self.close(&cursor, None);
                }
            }
            params => {
                log::error!("The editor does not know what to do with {:?}.", params);
            }
        }
    }

    /// Put a new cursor in the file at the position for the client, opening it if it is not open
    /// already.
    fn edit(&mut self, client: Uuid, request: Uuid, path: &Path, size: Size, at: Position) {
        // NOTE: The one file can be reached by more than one path, and every cursor in it has to
        // be in the one buffer for them to be editing the one text.
        let path: PathBuf = match canonicalize(path) {
            Ok(path) => path,
            // NOTE: A file which does not exist yet is made once it is written, so it is the
            // directory it goes in which has to be found.
            Err(error) if error.kind() == ErrorKind::NotFound => {
                match (path.parent().map(canonicalize), path.file_name()) {
                    (Some(Ok(dir)), Some(name)) => dir.join(name),
                    (Some(Err(error)), _) if error.kind() == ErrorKind::NotFound => {
                        self.refuse(request, String::from("Directory does not exist."));
                        return;
                    }
                    _ => {
                        self.refuse(request, format!("Failed to find the file: {}", error));
                        return;
                    }
                }
            }
            Err(error) => {
                self.refuse(request, format!("Failed to find the file: {}", error));
                return;
            }
        };

        if !self.buffers.contains_key(&path) {
            log::info!("Opening {:?} to be edited...", path);
            match Buffer::open(&path) {
                Ok(buffer) => {
                    self.buffers.insert(path.clone(), buffer);
                }
                Err(error) => {
                    self.refuse(request, error.to_string());
                    return;
                }
            }
        }

        let cursor: Uuid = Uuid::new_v4();
        let mut line: usize = 0;
        if let Some(buffer) = self.buffers.get_mut(&path) {
            buffer.add(cursor, at);
            line = buffer.cursor(&cursor).map_or(0, |cursor| cursor.at.line);
        }
        // NOTE: A cursor which starts somewhere in particular starts in the middle of what is
        // shown, so that what is around it can be seen.
        let viewer: Viewer = Viewer::builder()
            .client(client)
            .request(request)
            .path(path.clone())
            .size(size)
            .top(line.saturating_sub(size.rows / 2))
            .build();
        self.viewers.insert(cursor, viewer);

        self.show_all(&path);
    }

    /// Do what the client asked of its cursor to the file it is in.
    fn act<F>(&mut self, client: Uuid, cursor: &Uuid, act: F)
    where
        F: FnOnce(&mut Buffer, &Uuid) -> Option<Asked>,
    {
        if !self.owns(&client, cursor) {
            return;
        }
        let path: PathBuf = match self.viewers.get(cursor) {
            Some(viewer) => viewer.path.clone(),
            None => return,
        };
        let asked: Option<Asked> = match self.buffers.get_mut(&path) {
            Some(buffer) => act(buffer, cursor),
            None => return,
        };

        match asked {
            Some(Asked::Clipboard(text)) => {
                if let Some(viewer) = self.viewers.get(cursor) {
                    let params = ResponseParams::EditorClipboard(
                        EditorClipboardResponseParams::builder().text(text).build(),
                    );
                    self.send(viewer.request, params, false);
                }
            }
            Some(Asked::Quit) => {
                self.close(cursor, None);
                return;
            }
            Some(Asked::Vim(at)) => {
                self.close(cursor, Some(at));
                return;
            }
            Some(Asked::Scroll(scroll)) => self.scroll(cursor, scroll),
            None => {}
        }

        self.show_all(&path);
    }

    /// Scroll what the client moving the cursor is shown, moving the cursor just far enough to
    /// still be shown.
    ///
    /// NOTE: The same as in vim, it scrolls down no further than the last line being the first one
    /// shown.
    fn scroll(&mut self, cursor: &Uuid, scroll: Scroll) {
        let viewer: &mut Viewer = match self.viewers.get_mut(cursor) {
            Some(viewer) => viewer,
            None => return,
        };
        let buffer: &mut Buffer = match self.buffers.get_mut(&viewer.path) {
            Some(buffer) => buffer,
            None => return,
        };

        let last: usize = buffer.text().len() - 1;
        viewer.top = match scroll {
            Scroll::Down(lines) => viewer.top.saturating_add(lines).min(last),
            Scroll::Up(lines) => viewer.top.saturating_sub(lines),
        };
        let rows: usize = viewer.size.rows.max(1);
        buffer.keep(cursor, viewer.top, viewer.top + rows - 1);
    }

    /// Be done with the cursor, and tell the client moving it so, along with where to open the file
    /// in vim when it is done with for that.
    fn close(&mut self, cursor: &Uuid, vim: Option<Position>) {
        let viewer: Viewer = match self.viewers.remove(cursor) {
            Some(viewer) => viewer,
            None => return,
        };

        self.drop_cursor(cursor, &viewer.path);
        let params =
            ResponseParams::EditorClosed(EditorClosedResponseParams::builder().vim(vim).build());
        self.send(viewer.request, params, true);
        self.show_all(&viewer.path);
    }

    /// Be done with every cursor the client was moving, since it has gone away.
    fn leave(&mut self, client: &Uuid) {
        let cursors: Vec<(Uuid, PathBuf)> = self
            .viewers
            .iter()
            .filter(|(_, viewer)| viewer.client == *client)
            .map(|(cursor, viewer)| (*cursor, viewer.path.clone()))
            .collect();

        for (cursor, path) in cursors {
            self.viewers.remove(&cursor);
            self.drop_cursor(&cursor, &path);
            self.show_all(&path);
        }
    }

    /// Take the cursor out of the file it is in, closing the file once there are no cursors left
    /// in it.
    ///
    /// NOTE: Changes which have not been written are thrown away along with the file.
    fn drop_cursor(&mut self, cursor: &Uuid, path: &Path) {
        let buffer: &mut Buffer = match self.buffers.get_mut(path) {
            Some(buffer) => buffer,
            None => return,
        };

        buffer.remove(cursor);
        if buffer.is_empty() {
            log::info!("Closing {:?}.", path);
            self.buffers.remove(path);
        }
    }

    /// Return whether the cursor is the client's to move.
    fn owns(&self, client: &Uuid, cursor: &Uuid) -> bool {
        return matches!(self.viewers.get(cursor), Some(viewer) if viewer.client == *client);
    }

    /// Tell the client that the file could not be edited.
    fn refuse(&self, request: Uuid, error: String) {
        log::warn!("Refusing to edit a file: {}", error);
        let params = ResponseParams::EditorClosed(
            EditorClosedResponseParams::builder()
                .error(Some(error))
                .build(),
        );
        self.send(request, params, true);
    }

    /// Show every client moving a cursor in the file what it now looks like.
    fn show_all(&mut self, path: &Path) {
        let cursors: Vec<Uuid> = self
            .viewers
            .iter()
            .filter(|(_, viewer)| viewer.path == path)
            .map(|(cursor, _)| *cursor)
            .collect();

        for cursor in cursors {
            self.show(&cursor);
        }
    }

    /// Show the client moving the cursor what the file it is in now looks like.
    fn show(&mut self, cursor: &Uuid) {
        let viewer: &mut Viewer = match self.viewers.get_mut(cursor) {
            Some(viewer) => viewer,
            None => return,
        };
        let buffer: &mut Buffer = match self.buffers.get_mut(&viewer.path) {
            Some(buffer) => buffer,
            None => return,
        };
        let at: Position = match buffer.cursor(cursor) {
            Some(own) => own.at,
            None => return,
        };

        // Scroll just far enough for the cursor to be shown.
        let rows: usize = viewer.size.rows.max(1);
        if at.line < viewer.top {
            viewer.top = at.line;
        } else if at.line >= viewer.top + rows {
            viewer.top = at.line + 1 - rows;
        }
        let line_count: usize = buffer.text().len();
        viewer.top = viewer.top.min(line_count.saturating_sub(1));
        let range: Range<usize> = viewer.top..(viewer.top + rows).min(line_count);
        let shown: Vec<String> = buffer.text().lines()[range.clone()].to_vec();
        let runs: Vec<Vec<Run>> = buffer.runs(range);

        let own = match buffer.cursor(cursor) {
            Some(own) => own,
            None => return,
        };
        let view: EditorView = EditorView::builder()
            .path(buffer.path().to_path_buf())
            .cursor(*cursor)
            .top(viewer.top)
            .lines(shown)
            .runs(runs)
            .line_count(line_count)
            .cursors(buffer.cursors())
            .command((own.mode == EditorMode::Command).then(|| own.command.clone()))
            .search((own.mode == EditorMode::Search).then(|| own.search.clone()))
            .message(own.message.clone())
            .modified(buffer.modified())
            .build();

        let request: Uuid = viewer.request;
        let params =
            ResponseParams::EditorView(EditorViewResponseParams::builder().view(view).build());
        self.send(request, params, false);
    }

    /// Send the client which made the request a response to it.
    fn send(&self, request: Uuid, params: ResponseParams, last: bool) {
        let response: Response = Response::builder()
            .uuid(request)
            .last(last)
            .params(params)
            .build();
        let response: ContextedResponse = ContextedResponse::builder()
            .response(response)
            .log(false)
            .build();

        if let Err(error) = self.contexted_responses_tx.send(response) {
            log::error!("Failed to send a response: {}", error);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{remove_file, write};
    use std::path::PathBuf;

    use super::Editor;
    use crate::contexted_request::ContextedRequest;
    use crate::contexted_response::ContextedResponse;
    use crate::disconnected_client::DisconnectedClient;
    use crate::stop::Stop;

    use insh_api::{
        CloseEditorRequestParams, EditRequestParams, EditorKey, EditorKeyRequestParams, Position,
        Request, RequestParams, ResponseParams,
    };
    use rend::Size;

    use crossbeam::channel::{unbounded, Receiver, Sender};
    use test_case::test_case;
    use uuid::Uuid;

    /// The client editing the file.
    const CLIENT: Uuid = Uuid::from_u128(1);

    /// Some other client.
    const OTHER: Uuid = Uuid::from_u128(2);

    /// An editor, along with the ends of its channels which have to be kept for it to send on.
    struct Harness {
        /// The editor.
        editor: Editor,
        /// The file being edited, which is removed again once done with.
        path: PathBuf,
        /// The responses the editor sends.
        responses_rx: Receiver<ContextedResponse>,
        /// The other ends of the channels the editor receives on.
        _txs: (
            Sender<ContextedRequest>,
            Sender<DisconnectedClient>,
            Sender<Stop>,
        ),
    }

    impl Harness {
        /// Return an editor with nothing open in it, and a file for it to open.
        fn new() -> Self {
            let path: PathBuf = std::env::temp_dir().join(format!("inshd-{}", Uuid::new_v4()));
            let lines: String = (0..20).map(|line| format!("{}\n", line)).collect();
            write(&path, lines).unwrap();

            let (requests_tx, requests_rx) = unbounded();
            let (disconnected_tx, disconnected_rx) = unbounded();
            let (responses_tx, responses_rx) = unbounded();
            let (stop_tx, stop_rx) = unbounded();
            let editor: Editor = Editor::builder()
                .requests_rx(requests_rx)
                .disconnected_clients_rx(disconnected_rx)
                .contexted_responses_tx(responses_tx)
                .stop_rx(stop_rx)
                .build();

            Self {
                editor,
                path,
                responses_rx: responses_rx,
                _txs: (requests_tx, disconnected_tx, stop_tx),
            }
        }

        /// Have the client make the request.
        fn ask(&mut self, client: Uuid, request: Request) {
            self.editor.handle_request(
                ContextedRequest::builder()
                    .client_uuid(client)
                    .request(request)
                    .build(),
            );
        }

        /// Have the client edit the file, and return the request it was edited on.
        fn edit(&mut self, client: Uuid) -> Uuid {
            self.edit_at(client, None)
        }

        /// Have the client edit the file starting at the position, and return the request it was
        /// edited on.
        fn edit_at(&mut self, client: Uuid, at: Option<Position>) -> Uuid {
            let request: Request = Request::builder()
                .params(RequestParams::Edit(
                    EditRequestParams::builder()
                        .path(self.path.clone())
                        .size(Size::new(10, 10))
                        .at(at)
                        .build(),
                ))
                .build();
            let uuid: Uuid = *request.uuid();
            self.ask(client, request);
            uuid
        }

        /// Have the client press the key for the cursor.
        fn press(&mut self, client: Uuid, cursor: Uuid, key: EditorKey) {
            self.ask(
                client,
                Request::builder()
                    .params(RequestParams::EditorKey(
                        EditorKeyRequestParams::builder()
                            .cursor(cursor)
                            .key(key)
                            .build(),
                    ))
                    .build(),
            );
        }

        /// Return the cursor the client is moving, and the first line it is shown.
        fn shown(&self) -> (Position, usize) {
            let (cursor, viewer) = self.editor.viewers.iter().next().unwrap();
            let buffer = self.editor.buffers.get(&viewer.path).unwrap();
            (buffer.cursor(cursor).unwrap().at, viewer.top)
        }

        /// Have the client be done with the cursor made by the edit request.
        fn close(&mut self, client: Uuid, edit: Uuid) {
            self.ask(
                client,
                Request::builder()
                    .params(RequestParams::CloseEditor(
                        CloseEditorRequestParams::builder().edit(edit).build(),
                    ))
                    .build(),
            );
        }

        /// Return why editing was refused, if it was.
        fn refused(&self) -> Option<String> {
            self.responses_rx
                .try_iter()
                .find_map(|response| match response.response().params() {
                    ResponseParams::EditorClosed(params) => params.error().map(String::from),
                    _ => None,
                })
        }

        /// Return how many cursors there are.
        fn cursors(&self) -> usize {
            self.editor
                .buffers
                .values()
                .map(|buffer| buffer.cursors().len())
                .sum()
        }
    }

    impl Drop for Harness {
        fn drop(&mut self) {
            let _ = remove_file(&self.path);
        }
    }

    #[test_case(None, Position::new(0, 0), 0; "nowhere in particular")]
    #[test_case(Some(Position::new(15, 0)), Position::new(15, 0), 10; "in the middle of what is shown")]
    #[test_case(Some(Position::new(2, 0)), Position::new(2, 0), 0; "near the top")]
    #[test_case(Some(Position::new(99, 5)), Position::new(19, 1), 14; "past the end")]
    fn test_a_cursor_starts_where_it_is_asked_to(
        at: Option<Position>,
        cursor: Position,
        top: usize,
    ) {
        let mut harness: Harness = Harness::new();

        harness.edit_at(Uuid::new_v4(), at);

        assert_eq!(harness.shown(), (cursor, top));
    }

    #[test]
    fn test_a_file_which_does_not_exist_is_edited() {
        let mut harness: Harness = Harness::new();
        remove_file(&harness.path).unwrap();

        harness.edit(CLIENT);

        assert_eq!(harness.cursors(), 1);
        assert_eq!(harness.refused(), None);
    }

    #[test]
    fn test_a_file_in_a_directory_which_does_not_exist_is_refused() {
        let mut harness: Harness = Harness::new();
        remove_file(&harness.path).unwrap();
        harness.path = harness.path.join("new.txt");

        harness.edit(CLIENT);

        assert_eq!(harness.cursors(), 0);
        assert_eq!(
            harness.refused().as_deref(),
            Some("Directory does not exist.")
        );
    }

    #[test]
    fn test_closing_takes_out_only_the_cursor_edited_on() {
        let mut harness: Harness = Harness::new();
        let first: Uuid = harness.edit(CLIENT);
        harness.edit(CLIENT);

        harness.close(CLIENT, first);

        assert_eq!(harness.cursors(), 1);
        assert_eq!(harness.editor.viewers.len(), 1);
    }

    #[test_case(&[EditorKey::Control('e')], Position::new(1, 0), 1; "scrolling down moves the cursor onto the first line shown")]
    #[test_case(&[EditorKey::Char('3'), EditorKey::Control('e')], Position::new(3, 0), 3; "a count scrolls down more than one line")]
    #[test_case(&[EditorKey::Char('5'), EditorKey::Char('j'), EditorKey::Control('e')], Position::new(5, 0), 1; "scrolling down leaves a cursor which is still shown")]
    #[test_case(&[EditorKey::Char('9'), EditorKey::Char('9'), EditorKey::Control('e')], Position::new(19, 0), 19; "scrolling down stops at the last line")]
    #[test_case(&[EditorKey::Char('G'), EditorKey::Control('y')], Position::new(18, 0), 9; "scrolling up moves the cursor onto the last line shown")]
    #[test_case(&[EditorKey::Char('G'), EditorKey::Char('3'), EditorKey::Control('y')], Position::new(16, 0), 7; "a count scrolls up more than one line")]
    #[test_case(&[EditorKey::Control('y')], Position::new(0, 0), 0; "scrolling up stops at the first line")]
    fn test_scrolling(keys: &[EditorKey], cursor: Position, top: usize) {
        let mut harness: Harness = Harness::new();
        harness.edit(CLIENT);
        let id: Uuid = *harness.editor.viewers.keys().next().unwrap();

        for key in keys {
            harness.press(CLIENT, id, *key);
        }

        assert_eq!(harness.shown(), (cursor, top));
    }

    #[test]
    fn test_a_cursor_is_not_closed_by_another_client() {
        let mut harness: Harness = Harness::new();
        let edit: Uuid = harness.edit(CLIENT);

        harness.close(OTHER, edit);

        assert_eq!(harness.cursors(), 1);
    }

    #[test]
    fn test_the_file_is_closed_with_its_last_cursor_even_with_unwritten_changes() {
        let mut harness: Harness = Harness::new();
        let edit: Uuid = harness.edit(CLIENT);
        let cursor: Uuid = *harness.editor.viewers.keys().next().unwrap();
        harness.ask(
            CLIENT,
            Request::builder()
                .params(RequestParams::EditorKey(
                    EditorKeyRequestParams::builder()
                        .cursor(cursor)
                        .key(EditorKey::Char('x'))
                        .build(),
                ))
                .build(),
        );
        assert!(harness
            .editor
            .buffers
            .values()
            .all(|buffer| buffer.modified()));

        harness.close(CLIENT, edit);

        assert!(harness.editor.buffers.is_empty());
    }

    #[test]
    fn test_a_client_going_away_closes_its_files_even_with_unwritten_changes() {
        let mut harness: Harness = Harness::new();
        harness.edit(CLIENT);
        let cursor: Uuid = *harness.editor.viewers.keys().next().unwrap();
        harness.ask(
            CLIENT,
            Request::builder()
                .params(RequestParams::EditorKey(
                    EditorKeyRequestParams::builder()
                        .cursor(cursor)
                        .key(EditorKey::Char('x'))
                        .build(),
                ))
                .build(),
        );

        harness.editor.leave(&CLIENT);

        assert!(harness.editor.buffers.is_empty());
        assert!(harness.editor.viewers.is_empty());
    }
}
