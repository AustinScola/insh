//! Owns every session, and tells the clients showing one about whatever happens to it.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::thread::{self, JoinHandle};

use super::output::{Output, Reader, Writer};
use super::pane::{Pane, Props as PaneProps};
use super::session::Session;
use crate::contexted_request::ContextedRequest;
use crate::contexted_response::ContextedResponse;
use crate::disconnected_client::DisconnectedClient;
use crate::stop::Stop;

use insh_api::{
    DetachedResponseParams, LayoutResponseParams, PaneExitedResponseParams, PaneScreen,
    PaneScreenResponseParams, RequestParams, Response, ResponseParams, SessionSummary,
    SessionsResponseParams, Split,
};
use rend::Size;

use crossbeam::channel::{select, Receiver, Sender};
use nix::unistd::Pid;
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// Which session a client is showing, and the request to tell it about the session on.
#[derive(Debug, Clone, Copy)]
struct Attachment {
    /// Which session it is showing.
    session: Uuid,
    /// The request whose responses tell it about the session.
    request: Uuid,
    /// How big the terminal the client is in is.
    ///
    /// NOTE: This is kept rather than worked out from the panes of the session it is showing,
    /// because the panes of a session are however big the arrangement of them makes them and say
    /// nothing about how big the client is.
    size: Size,
}

/// Owns every session, and tells the clients showing one about whatever happens to it.
///
/// This is where the panes live rather than in a client, which is what lets a client go away and
/// come back to find the programs in its session still running, and what lets more than one client
/// show the one session at once.
#[derive(TypedBuilder)]
pub struct Multiplexer {
    /// A receiver of the requests which are about panes, windows and sessions.
    requests_rx: Receiver<ContextedRequest>,
    /// A receiver of what the programs running in panes write.
    output_rx: Receiver<Output>,
    /// A sender of what the programs running in panes write, to give to each new reader.
    output_tx: Sender<Output>,
    /// A receiver of information about clients disconnecting.
    disconnected_clients_rx: Receiver<DisconnectedClient>,
    /// A sender of responses.
    contexted_responses_tx: Sender<ContextedResponse>,
    /// A receiver of a stop sentinel.
    stop_rx: Receiver<Stop>,
    /// How many rows of a pane are kept after they have scrolled off the top of it.
    buffer_size: usize,

    /// The sessions, by which one each is.
    #[builder(setter(skip), default)]
    sessions: HashMap<Uuid, Session>,
    /// Which session each client which is showing one is showing.
    #[builder(setter(skip), default)]
    attachments: HashMap<Uuid, Attachment>,
    /// The threads reading what the programs running in panes write.
    ///
    /// These are kept only so that they are not let go of while they are still running. None of
    /// them is ever waited for, since each one waits for a program of its own.
    #[builder(setter(skip), default)]
    readers: HashMap<Uuid, JoinHandle<()>>,
    /// The threads sending what is typed to the programs running in panes.
    ///
    /// These are kept for the same reason the readers are, and are never waited for either.
    #[builder(setter(skip), default)]
    writers: HashMap<Uuid, JoinHandle<()>>,
    /// Which session was shown last, which is the one a client is given when it does not say.
    #[builder(setter(skip), default)]
    last: Option<Uuid>,
    /// How many sessions have been made, which is what says which order they were made in.
    #[builder(setter(skip), default)]
    made: usize,
}

impl Multiplexer {
    /// How big a session is until a client says how big the terminal it is in is.
    const STARTING_SIZE: Size = Size {
        rows: 24,
        columns: 80,
    };

    /// Run the multiplexer.
    pub fn run(&mut self) {
        log::info!("Multiplexer running.");

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
                recv(self.output_rx) -> output => {
                    match output {
                        Ok(output) => self.handle_output(output),
                        Err(error) => {
                            log::error!("Error receiving what a pane wrote: {}", error);
                        }
                    }
                }
                recv(self.disconnected_clients_rx) -> client => {
                    match client {
                        Ok(client) => {
                            self.attachments.remove(&client.client_uuid);
                        }
                        Err(error) => {
                            log::error!("Error receiving info about a disconnected client: {}", error);
                        }
                    }
                }
            }
        }

        self.close();
        log::info!("Multiplexer stopping.");
    }

    /// Do what the request asks.
    fn handle_request(&mut self, request: ContextedRequest) {
        let client: Uuid = *request.client_uuid();
        let uuid: Uuid = *request.request().uuid();

        match request.request().params() {
            RequestParams::Attach(params) => {
                self.attach(client, uuid, params.session().copied(), params.size())
            }
            RequestParams::Detach(_) => self.detach(client, uuid),
            RequestParams::WritePane(params) => {
                // NOTE: Which panes it goes to is worked out here rather than taken from what the
                // client said, because typing in every pane of a window at once is something the
                // window is set to do and the daemon is what knows that.
                let panes: Vec<Uuid> = match self.session_of(&client) {
                    Some(session) => session.typed_in(),
                    None => return,
                };

                if let Some(session) = self.session_of_mut(&client) {
                    for pane in panes {
                        if let Some(pane) = session.pane_mut(&pane) {
                            pane.write(params.bytes());
                        }
                    }
                }
            }
            RequestParams::SplitPane(params) => self.split(
                &client,
                *params.pane(),
                params.split(),
                params.dir().cloned(),
            ),
            RequestParams::SelectPane(params) => {
                let pane: Uuid = *params.pane();
                if let Some(session) = self.session_of_mut(&client) {
                    session.window_mut().select(pane);
                }
                self.tell_about_layout(&client, Vec::new());
            }
            RequestParams::KillPane(params) => self.kill(&client, *params.pane()),
            RequestParams::DragPane(params) => {
                let (panes, split, by) = (params.panes(), params.split(), params.by());
                let screens: Vec<PaneScreen> = match self.session_of_mut(&client) {
                    Some(session) => {
                        if !session.drag(panes, split, by) {
                            return;
                        }
                        Self::whole_screens(session)
                    }
                    None => return,
                };

                let session: Uuid = match self.attachments.get(&client) {
                    Some(attachment) => attachment.session,
                    None => return,
                };
                self.tell_all_about_layout(session, screens);
            }
            RequestParams::ScrollPane(params) => {
                let (pane, rows) = (*params.pane(), params.rows());
                let session: Uuid = match self.attachments.get(&client) {
                    Some(attachment) => attachment.session,
                    None => return,
                };

                let moved: bool = self
                    .sessions
                    .get_mut(&session)
                    .and_then(|session| session.pane_mut(&pane))
                    .is_some_and(|pane| pane.look_back(rows));
                if !moved {
                    return;
                }

                self.tell_about_pane(session, pane);
            }
            RequestParams::SwapPane(params) => {
                let (pane, after) = (*params.pane(), params.after());
                let screens: Vec<PaneScreen> = match self.session_of_mut(&client) {
                    Some(session) => {
                        if !session.swap(&pane, after) {
                            return;
                        }
                        session.fit();
                        Self::whole_screens(session)
                    }
                    None => return,
                };

                let session: Uuid = match self.attachments.get(&client) {
                    Some(attachment) => attachment.session,
                    None => return,
                };
                self.tell_all_about_layout(session, screens);
            }
            RequestParams::ZoomPane(params) => {
                let pane: Uuid = *params.pane();
                let screens: Vec<PaneScreen> = match self.session_of_mut(&client) {
                    Some(session) => {
                        if !session.window_mut().zoom(pane) {
                            return;
                        }
                        session.fit();
                        Self::whole_screens(session)
                    }
                    None => return,
                };
                self.tell_about_layout(&client, screens);
            }
            RequestParams::RenameSession(params) => {
                let name: String = params.name().to_string();
                match self.session_of_mut(&client) {
                    Some(session) => session.rename(name),
                    None => return,
                }
                self.tell_about_layout(&client, Vec::new());
            }
            RequestParams::RenameWindow(params) => {
                let (window, name) = (params.window().copied(), params.name().to_string());
                match self.session_of_mut(&client) {
                    Some(session) => session.window_or_active_mut(window.as_ref()).rename(name),
                    None => return,
                }
                self.tell_about_layout(&client, Vec::new());
            }
            RequestParams::SyncPanes(_) => {
                match self.session_of_mut(&client) {
                    Some(session) => session.window_mut().synchronize(),
                    None => return,
                }
                self.tell_about_layout(&client, Vec::new());
            }
            RequestParams::RunInPane(params) => {
                let props = PaneProps::builder()
                    .program(params.program().into())
                    .args(params.args().iter().map(OsString::from).collect())
                    .dir(params.dir().cloned())
                    .buffer_size(self.buffer_size)
                    .build();
                self.run_in_pane(&client, *params.pane(), props);
            }
            RequestParams::NewWindow(params) => self.new_window(
                &client,
                params.name().map(ToString::to_string),
                params.dir().cloned(),
            ),
            RequestParams::SelectWindow(params) => {
                let window: Uuid = *params.window();
                let screens: Vec<PaneScreen> = match self.session_of_mut(&client) {
                    Some(session) => {
                        if !session.select_window(window) {
                            return;
                        }
                        session.fit();
                        Self::whole_screens(session)
                    }
                    None => return,
                };
                self.tell_about_layout(&client, screens);
            }
            RequestParams::NewSession(params) => self.new_session(
                client,
                uuid,
                params.name().map(ToString::to_string),
                params.dir().cloned(),
                params.attach(),
            ),
            RequestParams::ListSessions(_) => self.list_sessions(&client, uuid),
            RequestParams::ResizeClient(params) => {
                let size: Size = params.size();
                if let Some(attachment) = self.attachments.get_mut(&client) {
                    attachment.size = size;
                }
                let screens: Vec<PaneScreen> = match self.session_of_mut(&client) {
                    Some(session) => {
                        session.resize(size);
                        Self::whole_screens(session)
                    }
                    None => return,
                };
                self.tell_about_layout(&client, screens);
            }
            // NOTE: Everything else is answered by a request handler rather than here, and only
            // the ones which are not reach the multiplexer at all.
            _ => {}
        }
    }

    /// Take the bytes a program wrote as having been written on the screen of its pane, and tell
    /// whichever clients are showing it.
    fn handle_output(&mut self, output: Output) {
        match output {
            Output::Wrote { pane, child, bytes } => {
                let session: Uuid = match self.session_with(&pane) {
                    Some(session) => session,
                    None => return,
                };

                if let Some(running) = self
                    .sessions
                    .get_mut(&session)
                    .and_then(|session| session.pane_mut(&pane))
                {
                    // NOTE: A pane may have had another program in it since this was written, and
                    // what the one before wrote is not what the pane is showing any more.
                    if running.child() != Some(child) {
                        return;
                    }
                    running.wrote(&bytes);
                }
                self.tell_about_pane(session, pane);
            }
            Output::Finished { pane, child } => {
                let session: Uuid = match self.session_with(&pane) {
                    Some(session) => session,
                    None => return,
                };

                if let Some(finished) = self
                    .sessions
                    .get_mut(&session)
                    .and_then(|session| session.pane_mut(&pane))
                {
                    // The program which finished may be one the pane has already moved on from.
                    if finished.child() != Some(child) {
                        return;
                    }
                    finished.finished();
                }
                // NOTE: The thread which was reading it is left to finish on its own rather than
                // being waited for. Waiting for it here would be waiting on whichever program the
                // pane has in it now, which cannot finish while this thread is not running: it is
                // the one which would deliver what is typed at it.
                self.readers.remove(&pane);
                self.writers.remove(&pane);

                // NOTE: The pane stays where it is and goes back to holding nothing, so that what
                // it shows is the browser again. A pane only goes when it is asked to.
                let screens: Vec<PaneScreen> = match self.sessions.get_mut(&session) {
                    Some(session) => Self::whole_screens(session),
                    None => return,
                };
                self.tell_all_about_layout(session, screens);
            }
        }
    }

    /// Show the client the session it asked for, or the one which was shown last when it did not
    /// ask for one, making a session when there is none to show.
    fn attach(&mut self, client: Uuid, request: Uuid, session: Option<Uuid>, size: Size) {
        let session: Uuid = match session
            .or(self.last)
            .filter(|id| self.sessions.contains_key(id))
        {
            Some(session) => session,
            None => match self.open_session(None, None, size) {
                Some(session) => session,
                None => return,
            },
        };

        self.attachments.insert(
            client,
            Attachment {
                session,
                request,
                size,
            },
        );
        self.last = Some(session);

        let screens: Vec<PaneScreen> = match self.sessions.get_mut(&session) {
            Some(session) => {
                session.resize(size);
                Self::whole_screens(session)
            }
            None => return,
        };
        self.tell_about_layout(&client, screens);
    }

    /// Stop showing the client a session.
    fn detach(&mut self, client: Uuid, request: Uuid) {
        self.attachments.remove(&client);

        self.send(
            client,
            request,
            ResponseParams::Detached(DetachedResponseParams::builder().build()),
            true,
        );
    }

    /// Make a session with one window and one pane in it, and return which one it is.
    fn open_session(
        &mut self,
        name: Option<String>,
        dir: Option<PathBuf>,
        size: Size,
    ) -> Option<Uuid> {
        let pane: Pane = self.open_pane(Self::pane_size(size), dir)?;

        let name: String = name.unwrap_or_else(|| self.next_name());
        self.made += 1;
        let mut session = Session::builder()
            .name(name)
            .made(self.made)
            .windows(Vec::new())
            .active(Uuid::nil())
            .panes(HashMap::new())
            .size(size)
            .build();
        session.add_window(String::from("1"), pane);
        session.fit();

        let id: Uuid = session.id();
        self.sessions.insert(id, session);
        self.last = Some(id);

        return Some(id);
    }

    /// Return a name for a session which is not one already taken.
    fn next_name(&self) -> String {
        let mut number: usize = 0;
        loop {
            let name: String = number.to_string();
            if !self.sessions.values().any(|session| session.name() == name) {
                return name;
            }
            number += 1;
        }
    }

    /// Return a new pane of the given size with nothing running in it.
    ///
    /// A pane starts out holding nothing, which is what a client shows the browser in. A program
    /// is only started in it when something asks for one.
    fn open_pane(&mut self, size: Size, _dir: Option<PathBuf>) -> Option<Pane> {
        return Some(Pane::empty(size));
    }

    /// Start a program in the pane and read what it writes.
    fn run_in_pane(&mut self, client: &Uuid, pane: Uuid, props: PaneProps) {
        let session: Uuid = match self.attachments.get(client) {
            Some(attachment) => attachment.session,
            None => return,
        };

        let (started, child) = match self
            .sessions
            .get_mut(&session)
            .and_then(|session| session.pane_mut(&pane))
        {
            Some(running) => match running.run(props) {
                Ok(started) => (started, running.child()),
                Err(error) => {
                    log::error!("Failed to start a program in pane {}: {}", pane, error);
                    return;
                }
            },
            None => return,
        };
        let (master, write_to, typed_rx) = started;

        let child: Pid = match child {
            Some(child) => child,
            None => return,
        };

        let mut reader = Reader::builder()
            .pane(pane)
            .child(child)
            .master(master)
            .output_tx(self.output_tx.clone())
            .build();
        let mut writer = Writer::builder()
            .pane(pane)
            .master(write_to)
            .typed_rx(typed_rx)
            .build();
        match thread::Builder::new()
            .name(format!("pane-writer-{}", pane))
            .spawn(move || writer.run())
        {
            Ok(handle) => {
                self.writers.insert(pane, handle);
            }
            Err(error) => {
                log::error!("Failed to start writing to pane {}: {}", pane, error);
                return;
            }
        }

        match thread::Builder::new()
            .name(format!("pane-reader-{}", pane))
            .spawn(move || reader.run())
        {
            Ok(handle) => {
                if let Some(old) = self.readers.insert(pane, handle) {
                    // The reader of whichever program was in the pane before stops on its own once
                    // that program has gone, so it is only let go of here.
                    drop(old);
                }
            }
            Err(error) => {
                log::error!("Failed to start reading pane {}: {}", pane, error);
                return;
            }
        }

        let screens: Vec<PaneScreen> = match self.sessions.get_mut(&session) {
            Some(session) => Self::whole_screens(session),
            None => return,
        };
        self.tell_all_about_layout(session, screens);
    }

    /// Return how big a pane of a client of the given size starts out, which is the whole of it
    /// but for the footer.
    fn pane_size(size: Size) -> Size {
        return Size::new(
            size.rows.saturating_sub(Session::FOOTER_ROWS).max(1),
            size.columns.max(1),
        );
    }

    /// Split a pane of the session a client is showing.
    fn split(&mut self, client: &Uuid, pane: Uuid, split: Split, dir: Option<PathBuf>) {
        let session: Uuid = match self.attachments.get(client) {
            Some(attachment) => attachment.session,
            None => return,
        };

        // The new pane is given the size of the one it is splitting for now; fitting the window
        // around it straight afterwards is what gives it the size it really has.
        let size: Size = match self.sessions.get(&session) {
            Some(session) => session
                .layouts()
                .first()
                .map_or(Size::new(1, 1), |layout| layout.size()),
            None => return,
        };

        let new: Pane = match self.open_pane(size, dir) {
            Some(new) => new,
            None => return,
        };

        let screens: Vec<PaneScreen> = match self.sessions.get_mut(&session) {
            Some(session) => {
                if !session.split(&pane, split, new) {
                    return;
                }
                Self::whole_screens(session)
            }
            None => return,
        };

        self.tell_all_about_layout(session, screens);
    }

    /// Return how big the terminal the client is in is.
    fn client_size(&self, client: &Uuid) -> Size {
        return self
            .attachments
            .get(client)
            .map_or(Self::STARTING_SIZE, |attachment| attachment.size);
    }

    /// Make a new window in the session a client is showing.
    fn new_window(&mut self, client: &Uuid, name: Option<String>, dir: Option<PathBuf>) {
        let session: Uuid = match self.attachments.get(client) {
            Some(attachment) => attachment.session,
            None => return,
        };

        let size: Size = Self::pane_size(self.client_size(client));

        let pane: Pane = match self.open_pane(size, dir) {
            Some(pane) => pane,
            None => return,
        };

        let screens: Vec<PaneScreen> = match self.sessions.get_mut(&session) {
            Some(session) => {
                // NOTE: A window which was not given a name is called by its number, the way
                // tmux calls one, so that there is always something to tell it apart by.
                let name: String = name.unwrap_or_else(|| (session.window_count() + 1).to_string());
                session.add_window(name, pane);
                session.fit();
                Self::whole_screens(session)
            }
            None => return,
        };

        self.tell_all_about_layout(session, screens);
    }

    /// Make a session and show it to the client when it asked to be shown it.
    fn new_session(
        &mut self,
        client: Uuid,
        request: Uuid,
        name: Option<String>,
        dir: Option<PathBuf>,
        attach: bool,
    ) {
        let size: Size = self.client_size(&client);

        let session: Uuid = match self.open_session(name, dir, size) {
            Some(session) => session,
            None => return,
        };

        if !attach {
            return;
        }

        // The client goes on being told about the session on the request it attached with, so the
        // one it asked for a session on is only used when it was not showing one already.
        let request: Uuid = self
            .attachments
            .get(&client)
            .map_or(request, |attachment| attachment.request);
        self.attachments.insert(
            client,
            Attachment {
                session,
                request,
                size,
            },
        );

        let screens: Vec<PaneScreen> = match self.sessions.get_mut(&session) {
            Some(session) => Self::whole_screens(session),
            None => return,
        };
        self.tell_about_layout(&client, screens);
    }

    /// Tell the client which sessions there are.
    fn list_sessions(&mut self, client: &Uuid, request: Uuid) {
        let showing: Option<Uuid> = self.attachments.get(client).map(|at| at.session);

        let mut sessions: Vec<SessionSummary> = self
            .sessions
            .values()
            .map(|session| {
                SessionSummary::builder()
                    .session(session.id())
                    .name(session.name())
                    .windows(session.window_summaries())
                    .clients(
                        self.attachments
                            .values()
                            .filter(|attachment| attachment.session == session.id())
                            .count(),
                    )
                    .showing(showing == Some(session.id()))
                    .build()
            })
            .collect();

        // NOTE: The sessions are kept in no order of their own, so they are put in the order they
        // were made in rather than in whichever one they happen to come out in. Going by the
        // order they were made in is what puts a new one after the ones which were there before
        // it, whatever it ends up being called.
        let made = |session: &Uuid| self.sessions.get(session).map_or(0, Session::made);
        sessions.sort_by_key(|session| made(session.session()));

        self.send(
            *client,
            request,
            ResponseParams::Sessions(SessionsResponseParams::builder().sessions(sessions).build()),
            true,
        );
    }

    /// Close a pane of the session a client is showing.
    fn kill(&mut self, client: &Uuid, pane: Uuid) {
        let session: Uuid = match self.attachments.get(client) {
            Some(attachment) => attachment.session,
            None => return,
        };
        self.close_pane(session, pane);
    }

    /// Take a pane out of its session, closing the session when it was the last one.
    fn close_pane(&mut self, session: Uuid, pane: Uuid) {
        let left: bool = match self.sessions.get_mut(&session) {
            Some(session) => session.remove_pane(&pane),
            None => return,
        };

        self.tell_all(
            session,
            ResponseParams::PaneExited(PaneExitedResponseParams::builder().pane(pane).build()),
        );

        if !left {
            self.sessions.remove(&session);
            if self.last == Some(session) {
                self.last = self.sessions.keys().next().copied();
            }

            // The clients which were showing it have nothing to show any more.
            let showing: Vec<Uuid> = self.showing(session);
            for client in showing {
                if let Some(attachment) = self.attachments.remove(&client) {
                    self.send(
                        client,
                        attachment.request,
                        ResponseParams::Detached(DetachedResponseParams::builder().build()),
                        true,
                    );
                }
            }
            return;
        }

        let screens: Vec<PaneScreen> = match self.sessions.get_mut(&session) {
            Some(session) => {
                session.fit();
                Self::whole_screens(session)
            }
            None => return,
        };
        self.tell_all_about_layout(session, screens);
    }

    /// Return the whole of the screen of every pane of the window being looked at.
    fn whole_screens(session: &mut Session) -> Vec<PaneScreen> {
        let shown: Vec<Uuid> = session.window().tree().panes();

        // NOTE: A pane which holds nothing has no screen for the daemon to send, because what is
        // in it is something the client draws for itself.
        return session
            .panes_mut()
            .filter(|pane| shown.contains(&pane.id()))
            .filter_map(Pane::whole_screen)
            .collect();
    }

    /// Return the session the client is showing.
    fn session_of(&self, client: &Uuid) -> Option<&Session> {
        let session: Uuid = self.attachments.get(client)?.session;
        return self.sessions.get(&session);
    }

    /// Return the session the client is showing, to be changed.
    fn session_of_mut(&mut self, client: &Uuid) -> Option<&mut Session> {
        let session: Uuid = self.attachments.get(client)?.session;
        return self.sessions.get_mut(&session);
    }

    /// Return which session the pane is in.
    ///
    /// Every window is looked through rather than only the one being looked at, because a program
    /// in a window which is not being looked at goes on writing just the same.
    fn session_with(&self, pane: &Uuid) -> Option<Uuid> {
        return self
            .sessions
            .iter()
            .find(|(_, session)| session.has_pane(pane))
            .map(|(id, _)| *id);
    }

    /// Return the clients which are showing the session.
    fn showing(&self, session: Uuid) -> Vec<Uuid> {
        return self
            .attachments
            .iter()
            .filter(|(_, attachment)| attachment.session == session)
            .map(|(client, _)| *client)
            .collect();
    }

    /// Tell every client showing the session what has changed about the screen of one of its
    /// panes.
    fn tell_about_pane(&mut self, session: Uuid, pane: Uuid) {
        // NOTE: A pane of a window which is not being looked at is not drawn, so nothing is sent
        // about it until it is. What has changed about it is deliberately left alone rather than
        // asked for and thrown away, since asking takes it as having been sent.
        let shown: bool = self
            .sessions
            .get(&session)
            .is_some_and(|session| session.is_shown(&pane));
        if !shown {
            return;
        }

        let screen: PaneScreen = match self
            .sessions
            .get_mut(&session)
            .and_then(|session| session.pane_mut(&pane))
            .and_then(Pane::screen)
        {
            Some(screen) => screen,
            None => return,
        };

        self.tell_all(
            session,
            ResponseParams::PaneScreen(PaneScreenResponseParams::builder().screen(screen).build()),
        );
    }

    /// Tell the client where the panes and windows of the session it is showing are.
    fn tell_about_layout(&mut self, client: &Uuid, screens: Vec<PaneScreen>) {
        let attachment: Attachment = match self.attachments.get(client) {
            Some(attachment) => *attachment,
            None => return,
        };
        let layout = match self.sessions.get(&attachment.session) {
            Some(session) => session.layout(),
            None => return,
        };

        self.send(
            *client,
            attachment.request,
            ResponseParams::Layout(
                LayoutResponseParams::builder()
                    .layout(layout)
                    .screens(screens)
                    .build(),
            ),
            false,
        );
    }

    /// Tell every client showing the session where its panes and windows are.
    fn tell_all_about_layout(&mut self, session: Uuid, screens: Vec<PaneScreen>) {
        for client in self.showing(session) {
            self.tell_about_layout(&client, screens.clone());
        }
    }

    /// Tell every client showing the session the same thing.
    fn tell_all(&mut self, session: Uuid, params: ResponseParams) {
        for client in self.showing(session) {
            let request: Uuid = match self.attachments.get(&client) {
                Some(attachment) => attachment.request,
                None => continue,
            };
            self.send(client, request, params.clone(), false);
        }
    }

    /// Send the response to the client on the given request.
    #[allow(unused_variables)]
    fn send(&self, client: Uuid, request: Uuid, params: ResponseParams, last: bool) {
        let response = Response::builder()
            .uuid(request)
            .last(last)
            .params(params)
            .build();

        if let Err(error) = self
            .contexted_responses_tx
            .send(ContextedResponse::builder().response(response).build())
        {
            log::error!("Failed to send a response to client {}: {}", client, error);
        }
    }

    /// Ask the program of every pane to finish.
    fn close(&mut self) {
        for session in self.sessions.values_mut() {
            for pane in session.panes_mut() {
                pane.close();
            }
        }
    }
}
