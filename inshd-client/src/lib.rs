/*!
The client side of the inshd protocol.

Requests and responses are sent over a unix socket, each one prefixed with how many bytes long it
is. Both insh and inshd talk to the daemon, but they do it in different shapes: insh reads and
writes on separate threads so that it can keep drawing, and the inshd commands block. So the two
directions are separate here, and [`InshdClient`] puts them back together for callers which do not
need them apart.
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::needless_return)]

mod client;
mod request_writer;
mod response_reader;

pub use client::{ConnectError, InshdClient};
pub use request_writer::{RequestWriter, SendError};
pub use response_reader::{ReceiveError, ResponseReader};

/// The number of bytes the length is encoded in.
const LENGTH_SIZE: usize = 8;
