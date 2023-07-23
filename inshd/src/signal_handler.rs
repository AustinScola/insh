//! Handles signals.
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crossbeam::sync::Unparker;
use signal_hook::consts::signal::{SIGINT, SIGQUIT, SIGTERM};
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::flag;
use signal_hook::iterator::exfiltrator::origin::WithOrigin;
use signal_hook::iterator::SignalsInfo;
use typed_builder::TypedBuilder;

/// Handles signals.
#[derive(TypedBuilder)]
pub struct SignalHandler {
    /// Unparks the main thread.
    main_unparker: Unparker,
}

impl SignalHandler {
    /// Run the signal handler.
    pub fn run(&mut self) {
        log::info!("Signal handler running...");

        // Make sure double <Ctrl>-C and similar causes the program to terminate.
        let term_now = Arc::new(AtomicBool::new(false));
        for sig in TERM_SIGNALS {
            // When terminated by a second term signal, exit with exit code 1.
            // This will do nothing the first time (because term_now is false).
            flag::register_conditional_shutdown(*sig, 1, Arc::clone(&term_now)).unwrap();
            // But this will "arm" the above for the second time, by setting it to true.
            // The order of registering these is important, if you put this one first, it will
            // first arm and then terminate ‒ all in the first round.
            flag::register(*sig, Arc::clone(&term_now)).unwrap();
        }

        let mut signals = SignalsInfo::<WithOrigin>::new(TERM_SIGNALS).unwrap();

        for info in &mut signals {
            match info.signal {
                SIGTERM | SIGQUIT | SIGINT => {
                    log::info!(
                        "Received signal {} from process with pid {}.",
                        info.signal,
                        match info.process {
                            Some(process) => process.pid.to_string(),
                            None => String::from("unknown"),
                        }
                    );

                    self.main_unparker.unpark();

                    break;
                }
                _ => {}
            }
        }

        log::info!("Signal handler stopping...");
    }
}
