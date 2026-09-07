//! Arguments for inshd.
use std::error::Error;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::num::ParseFloatError;
use std::path::PathBuf;
use std::time::Duration;

use crate::logging::{Color, LogOptions};

use common::args::ModuleLogLevelFilter;

use clap::{Args as ClapArgs, Parser, Subcommand};
use flexi_logger::{LevelFilter as LogLevelFilter, LogSpecification as LogSpec};

/// Arguments for inshd.
#[derive(Parser, Debug)]
#[command(name = "inshd", author, version, about)]
pub struct Args {
    /// File to write logs to (can be a unix socket)
    #[arg(long = "log-file", display_order = 0)]
    log_file_path: Option<PathBuf>,

    /// Default log level for all modules.
    #[arg(display_order = 1, long = "log-level", id = "LOG_LEVEL", default_value_t = LogLevelFilter::Info)]
    log_level_filter: LogLevelFilter,

    /// Log level for a particular module (<module-name>=<log-level>).
    #[arg(display_order = 2, long = "module-log-level", id = "MODULE_LOG_LEVEL")]
    module_log_level_filters: Vec<ModuleLogLevelFilter>,

    /// Color the logs.
    #[arg(display_order = 3, long = "color", conflicts_with = "no_color")]
    color: bool,

    /// Do not color the logs.
    #[arg(display_order = 4, long = "no-color")]
    no_color: bool,

    /// The command to run.
    #[command(subcommand)]
    command: Command,
}

impl Args {
    /// Return the command.
    pub fn command(&self) -> &Command {
        &self.command
    }

    /// Return whether or not to color the logs.
    pub fn color(&self) -> Color {
        if self.color {
            Color::Always
        } else if self.no_color {
            Color::Never
        } else {
            Color::Auto
        }
    }
}

impl Args {
    /// Return options for logging.
    pub fn log_options(&self) -> LogOptions {
        LogOptions::builder()
            .level(self.log_level_filter)
            .log_spec(self.log_spec())
            .log_file_path(self.log_file_path.clone())
            .stdout_only(matches!(self.command, Command::Logs | Command::Database(_)))
            .color(self.color())
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
    /// Stream the logs of the daemon.
    Logs,
    /// Daemon database subcommands.
    #[command(alias = "db")]
    Database(DatabaseArgs),
}

/// Arguments for starting the daemon.
#[derive(ClapArgs, Debug, Clone)]
pub struct StartArgs {
    /// Start even if already running.
    #[arg(short = 'f')]
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
    #[arg(short = 'f')]
    pub force: bool,
    /// How long to wait for the inshd main process to stop.
    #[arg(default_value = "10", value_parser = parse_duration)]
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
    #[arg(short = 'f')]
    pub force: bool,
    /// How long to wait for the inshd main process to stop.
    #[arg(default_value = "10", value_parser = parse_duration)]
    pub timeout: Duration,
}

/// Arguments for working with the database of the daemon.
#[derive(ClapArgs, Debug, Clone)]
pub struct DatabaseArgs {
    /// The command to run.
    #[command(subcommand)]
    pub command: DatabaseCommand,
}

/// The database command to run.
#[derive(Subcommand, Clone, Debug)]
pub enum DatabaseCommand {
    /// Open a shell for the database.
    Shell(ShellArgs),
}

/// Arguments for opening a shell for the database.
#[derive(ClapArgs, Debug, Clone)]
pub struct ShellArgs {
    /// Run a command and exit instead of prompting.
    #[arg(short = 'c', long = "command")]
    pub command: Option<String>,
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
