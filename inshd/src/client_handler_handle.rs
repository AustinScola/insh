//! A handle to a client handler.
use std::thread::JoinHandle;

use os_pipe::PipeWriter;
use typed_builder::TypedBuilder;
use uuid::Uuid;

/// A handle to a client handler.
#[derive(TypedBuilder)]
pub struct ClientHandlerHandle {
    /// The UUID of the client.
    pub client: Uuid,
    /// The handle to the client handler thread.
    pub handle: JoinHandle<()>,
    /// The write side of a pipe for telling the client handler to stop.
    pub stop_tx: PipeWriter,
}
