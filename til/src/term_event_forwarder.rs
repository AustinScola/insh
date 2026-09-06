use std::time::Duration;

use term::{Term, TermEvent};

use crossbeam::channel::Sender;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub struct TermEventForwarder {
    term_event_tx: Sender<TermEvent>,
    /// How long to wait for the rest of an escape sequence before deciding that the escape key was
    /// pressed on its own.
    #[builder(default=Term::DEFAULT_ESCAPE_TIMEOUT)]
    escape_timeout: Duration,
}

impl TermEventForwarder {
    pub fn run(&self) {
        #[cfg(feature = "logging")]
        log::info!("Terminal event forwarder running...");

        // NOTE: The terminal is read from on this thread, so it is made here rather than being
        // taken as a prop, which would make it on whichever thread the forwarder was built on.
        let mut term: Term = Term::builder().escape_timeout(self.escape_timeout).build();

        loop {
            let term_event: TermEvent = match term.read() {
                Ok(term_event) => term_event,
                #[allow(unused_variables)]
                Err(error) => {
                    #[cfg(feature = "logging")]
                    log::error!(
                        "Terminal event forwarder failed to read terminal event: {}",
                        error
                    );
                    continue;
                }
            };

            #[allow(unused_variables)]
            if let Err(error) = self.term_event_tx.send(term_event) {
                #[cfg(feature = "logging")]
                log::error!("Failed to send term event: {}", error);
                break;
            }
        }

        #[cfg(feature = "logging")]
        log::info!("Terminal event forwarder stopping...");
    }
}
