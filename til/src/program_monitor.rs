use crossbeam::channel::Sender;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use nix::Result as NixResult;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub struct ProgramMonitor {
    child: Pid,
    program_event_tx: Sender<ProgramEvent>,
}

impl ProgramMonitor {
    pub fn run(&self) {
        #[cfg(feature = "logging")]
        log::info!(
            "Running program monitor for program with pid {}.",
            self.child
        );

        // let options: Option<WaitPidFlag> = Some(WaitPidFlag::WEXITED);
        let options: Option<WaitPidFlag> = None;
        loop {
            let result: NixResult<WaitStatus> = waitpid(self.child, options);
            let status: WaitStatus = match result {
                Ok(status) => status,
                #[allow(unused_variables)]
                Err(error) => {
                    #[cfg(feature = "logging")]
                    log::info!("Program monitor encounted error: {}.", error);
                    break;
                }
            };

            match status {
                #[allow(unused_variables)]
                WaitStatus::Exited(pid, status) => {
                    #[cfg(feature = "logging")]
                    log::info!("Program with pid {} exited with status {}.", pid, status);
                    self.program_event_tx.send(ProgramEvent::Done).unwrap();
                    break;
                }
                _ => {}
            }
        }

        #[cfg(feature = "logging")]
        log::info!(
            "Done running program monitor for program with pid {}.",
            self.child
        );
    }
}

pub enum ProgramEvent {
    Done,
}
