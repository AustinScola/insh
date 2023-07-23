use term::{Term, TermEvent};

use crossbeam::channel::Sender;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub struct TermEventForwarder {
    #[builder(setter(skip), default=Term::new())]
    term: Term,
    term_event_tx: Sender<TermEvent>,
}

impl TermEventForwarder {
    pub fn run(&mut self) {
        #[cfg(feature = "logging")]
        log::info!("Terminal event forwarder running...");

        loop {
            let term_event: TermEvent = match self.term.read() {
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
