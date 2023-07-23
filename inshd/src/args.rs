//! Arguments for inshd.
use crate::logging::LogOptions;
use common::args::ModuleLogLevelFilter;

use std::path::PathBuf;

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
        let mut log_spec_builder = LogSpec::builder();

        log_spec_builder.default(self.log_level_filter);

        for module_log_level_filter in &self.module_log_level_filters {
            log_spec_builder.module(
                module_log_level_filter.module_name(),
                *module_log_level_filter.log_level_filter(),
            );
        }

        let log_spec: LogSpec = log_spec_builder.finalize();

        LogOptions::builder()
            .log_spec(log_spec)
            .log_file_path(self.log_file_path.clone())
            .build()
    }
}

/// The command to run.
#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    /// Start the daemon.
    Start,
    /// Stop the daemon.
    Stop(StopArgs),
    /// Check the status of the daemon.
    Status,
}

/// Arguments for stopping the daemon.
#[derive(ClapArgs, Debug, Clone)]
pub struct StopArgs {
    /// Force stop (with SIGKILL).
    #[clap(short = 'f')]
    pub force: bool,
}
