//! Logging.
use crate::paths::INSHD_LOGS_DIR;

use std::io::{Error as IOError, Write};
use std::path::PathBuf;
use std::thread;

use flexi_logger::{
    Age, Cleanup, Criterion, DeferredNow, FileSpec, LevelFilter as LogLevelFilter,
    LogSpecification as LogSpec, Logger, LoggerHandle, Naming, Record,
};
use typed_builder::TypedBuilder;

/// Configure logging.
pub fn configure_logging(options: &LogOptions) -> LoggerHandle {
    let LogOptions {
        level,
        log_file_path,
        log_spec,
    } = options;

    let mut logger = Logger::with(log_spec.clone()).format(log_format);

    if let Some(log_file_path) = log_file_path {
        logger = logger.log_to_file(FileSpec::try_from(log_file_path).unwrap())
    } else {
        logger = logger
            .log_to_file(FileSpec::default().directory(&*INSHD_LOGS_DIR))
            .o_append(true)
            .rotate(
                Criterion::Age(Age::Day),
                Naming::Timestamps,
                Cleanup::KeepLogFiles(7),
            )
            .duplicate_to_stdout((*level).into());
    }

    logger.start().unwrap()
}

/// Options for logging.
#[derive(TypedBuilder)]
pub struct LogOptions {
    /// The log level.
    level: LogLevelFilter,
    /// A specification for logging.
    log_spec: LogSpec,
    /// The path for log files.
    log_file_path: Option<PathBuf>,
}

/// Format log records.
pub fn log_format(
    writer: &mut dyn Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), IOError> {
    write!(
        writer,
        "{} {} [{}] [{}] {}",
        now.now().format("%d-%m-%Y %H:%M.%S"),
        record.level(),
        record.module_path().unwrap_or("<unnamed>"),
        thread::current().name().unwrap_or("<unnamed>"),
        &record.args()
    )
}
