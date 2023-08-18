//! Arguments for inshd.
use crate::logging::LogOptions;
use common::args::ModuleLogLevelFilter;

use std::error::Error;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::num::ParseFloatError;
use std::path::PathBuf;
use std::time::Duration;

use clap::{Args as ClapArgs, Parser, Subcommand};
use flexi_logger::{LevelFilter as LogLevelFilter, LogSpecification as LogSpec};

/// Arguments for inshd.
#[derive(Parser, Debug)]
#[clap(name = "inshd", author, version, about)]
pub struct Args {
    /// File to write logs to (can be a unix socket)
    #[clap(long = "log-file", display_order = 0)]
    log_file_path: Option<PathBuf>,

    /// Default log level for all modules.
    #[clap(display_order = 1, long = "log-level", id = "LOG_LEVEL", default_value_t = LogLevelFilter::Info)]
    log_level_filter: LogLevelFilter,

    /// Log level for a particular module (<module-name>=<log-level>).
    #[clap(display_order = 2, long = "module-log-level", id = "MODULE_LOG_LEVEL")]
    module_log_level_filters: Vec<ModuleLogLevelFilter>,

    /// The command to run.
    #[clap(subcommand)]
    command: Command,
}

impl Args {
    /// Return the command.
    pub fn command(&self) -> &Command {
        &self.command
    }
}

impl Args {
    /// Return options for logging.
    pub fn log_options(&self) -> LogOptions {
        LogOptions::builder()
            .level(self.log_level_filter)
            .log_spec(self.log_spec())
            .log_file_path(self.log_file_path.clone())
            .build()
    }

    /// Return the log specification.
    fn log_spec(&self) -> LogSpec {
        let mut log_spec_builder = LogSpec::builder();

        log_spec_builder.default(self.log_level_filter);

        for module_log_level_filter in &self.module_log_level_filters {
            log_spec_builder.module(
                module_log_level_filter.module_name(),
                *module_log_level_filter.log_level_filter(),
            );
        }

        log_spec_builder.finalize()
    }
}

/// The command to run.
#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    /// Start the daemon.
    Start(StartArgs),
    /// Stop the daemon.
    Stop(StopArgs),
    /// Restart the daemon.
    Restart(RestartArgs),
    /// Check the status of the daemon.
    Status,
}

/// Arguments for starting the daemon.
#[derive(ClapArgs, Debug, Clone)]
pub struct StartArgs {
    /// Start even if already running.
    #[clap(short = 'f')]
    pub force: bool,
}

impl From<&RestartArgs> for StartArgs {
    fn from(restart_args: &RestartArgs) -> Self {
        Self {
            force: restart_args.force,
        }
    }
}

/// Arguments for stopping the daemon.
#[derive(ClapArgs, Debug, Clone)]
pub struct StopArgs {
    /// Force stop (with SIGKILL).
    #[clap(short = 'f')]
    pub force: bool,
    /// How long to wait for the inshd main process to stop.
    #[clap(default_value = "10", value_parser = parse_duration)]
    pub timeout: Duration,
}

impl From<&RestartArgs> for StopArgs {
    fn from(restart_args: &RestartArgs) -> Self {
        Self {
            force: restart_args.force,
            timeout: restart_args.timeout,
        }
    }
}

/// Arguments for restarting the daemon.
#[derive(ClapArgs, Debug, Clone)]
pub struct RestartArgs {
    /// Force stop (with SIGKILL) and force start.
    #[clap(short = 'f')]
    pub force: bool,
    /// How long to wait for the inshd main process to stop.
    #[clap(default_value = "10", value_parser = parse_duration)]
    pub timeout: Duration,
}

/// Parse a duration.
fn parse_duration(string: &str) -> Result<Duration, ParseDurationError> {
    let secs: f64 = match string.parse() {
        Ok(secs) => secs,
        Err(error) => {
            return Err(ParseDurationError::InvalidFloat(error));
        }
    };
    Ok(Duration::from_secs_f64(secs))
}

/// An error parsing a duration.
#[derive(Debug)]
pub enum ParseDurationError {
    /// An invalid float value.
    InvalidFloat(ParseFloatError),
}

impl Display for ParseDurationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            Self::InvalidFloat(error) => {
                write!(formatter, "Failed to parse the duration: {}", error)
            }
        }
    }
}

impl Error for ParseDurationError {}
