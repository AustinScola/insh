//! Logging.
use crate::paths::INSHD_LOGS_DIR;

use std::io::{Error as IOError, Write};
use std::path::PathBuf;
use std::thread;

use crossbeam::channel::{self, Receiver, Sender};
use flexi_logger::writers::LogWriter;
use flexi_logger::{
    Age, Cleanup, Criterion, DeferredNow, FileSpec, LevelFilter as LogLevelFilter,
    LogSpecification as LogSpec, Logger, LoggerHandle, Naming, Record,
};
use typed_builder::TypedBuilder;

/// The number of log records which are buffered for streaming to clients before records start
/// being dropped.
const LOG_RECORD_BUFFER_SIZE: usize = 1024;

/// Configured logging.
pub struct ConfiguredLogging {
    /// A handle to the logger.
    pub logger_handle: LoggerHandle,
    /// A receiver of the log records which have been emitted.
    pub records_rx: Receiver<String>,
}

/// Configure logging.
pub fn configure_logging(options: &LogOptions) -> ConfiguredLogging {
    let LogOptions {
        level,
        log_file_path,
        log_spec,
        stdout_only,
    } = options;

    let (records_tx, records_rx): (Sender<String>, Receiver<String>) =
        channel::bounded(LOG_RECORD_BUFFER_SIZE);
    let record_sender: Box<dyn LogWriter> = Box::new(LogRecordSender::new(records_tx));

    let mut logger = Logger::with(log_spec.clone()).format(log_format);

    if *stdout_only {
        logger = logger.log_to_stdout();
    } else if let Some(log_file_path) = log_file_path {
        logger =
            logger.log_to_file_and_writer(FileSpec::try_from(log_file_path).unwrap(), record_sender)
    } else {
        logger = logger
            .log_to_file_and_writer(
                FileSpec::default().directory(&*INSHD_LOGS_DIR),
                record_sender,
            )
            .o_append(true)
            .rotate(
                Criterion::Age(Age::Day),
                Naming::Timestamps,
                Cleanup::KeepLogFiles(7),
            )
            .duplicate_to_stdout((*level).into());
    }

    ConfiguredLogging {
        logger_handle: logger.start().unwrap(),
        records_rx,
    }
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
    /// Whether or not to only log to stdout.
    ///
    /// Commands which do not run the daemon should not attach the log file writer of the daemon.
    /// Doing so can rotate the log file the running daemon is writing to out from under it.
    #[builder(default = false)]
    stdout_only: bool,
}

/// Sends log records to be streamed to clients.
struct LogRecordSender {
    /// A sender of log records.
    records_tx: Sender<String>,
}

impl LogRecordSender {
    /// Return a new sender of log records.
    fn new(records_tx: Sender<String>) -> Self {
        Self { records_tx }
    }
}

impl LogWriter for LogRecordSender {
    // NOTE: This must never block and must never log. It is called by every thread which logs, so
    // blocking here would stall all logging in the daemon, and logging here would be recursive.
    // Records are dropped if no one is receiving them or if the receiver has fallen behind.
    fn write(&self, now: &mut DeferredNow, record: &Record) -> Result<(), IOError> {
        let mut buffer: Vec<u8> = Vec::new();
        log_format(&mut buffer, now, record)?;
        let _ = self
            .records_tx
            .try_send(String::from_utf8_lossy(&buffer).into_owned());
        Ok(())
    }

    fn flush(&self) -> Result<(), IOError> {
        Ok(())
    }
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
