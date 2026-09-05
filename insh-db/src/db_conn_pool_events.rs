//! Logging of how many connections to the database are in use.
use std::fmt::{Debug, Error as FmtError, Formatter};
use std::sync::atomic::{AtomicU32, Ordering};

use diesel::r2d2::event::{CheckinEvent, CheckoutEvent, HandleEvent, TimeoutEvent};

/// Logs how many connections to the database are in use whenever that changes.
pub struct DbConnPoolEventHandler {
    /// The number of connections which are checked out of the pool.
    in_use: AtomicU32,
    /// The maximum number of connections that the pool holds.
    size: u32,
}

impl DbConnPoolEventHandler {
    /// Return a new handler for a pool which holds at most `size` connections.
    pub fn new(size: u32) -> Self {
        Self {
            in_use: AtomicU32::new(0),
            size,
        }
    }

    /// Log how many connections are in use.
    fn log(&self, in_use: u32) {
        log::debug!("{}/{} database connections in use.", in_use, self.size);
    }
}

impl Debug for DbConnPoolEventHandler {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        write!(
            formatter,
            "{}/{} database connections in use",
            self.in_use.load(Ordering::Relaxed),
            self.size
        )
    }
}

impl HandleEvent for DbConnPoolEventHandler {
    fn handle_checkout(&self, _event: CheckoutEvent) {
        self.log(self.in_use.fetch_add(1, Ordering::Relaxed) + 1);
    }

    fn handle_checkin(&self, _event: CheckinEvent) {
        self.log(self.in_use.fetch_sub(1, Ordering::Relaxed) - 1);
    }

    fn handle_timeout(&self, _event: TimeoutEvent) {
        log::warn!(
            "Timed out waiting for one of the {} database connections.",
            self.size
        );
    }
}
