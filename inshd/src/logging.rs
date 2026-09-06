//! Logging.
use std::fmt::{Display, Error as FmtError, Formatter};
use std::io::{stdout, Error as IOError, IsTerminal, Write};
use std::path::PathBuf;
use std::thread;

use crate::paths::INSHD_LOGS_DIR;

use ansi::{Color as AnsiColor, ControlFunction, GraphicRendition};
use common::paths::make_private_dir;
use insh_api::{LogLevel, LogRecord};

use crossbeam::channel::{self, Receiver, Sender};
use flexi_logger::writers::LogWriter;
use flexi_logger::{
    AdaptiveFormat, Age, Cleanup, Criterion, DeferredNow, FileSpec, Level,
    LevelFilter as LogLevelFilter, LogSpecification as LogSpec, Logger, LoggerHandle, Naming,
    Record,
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
    pub records_rx: Receiver<LogRecord>,
}

/// Configure logging.
pub fn configure_logging(options: &LogOptions) -> ConfiguredLogging {
    let LogOptions {
        level,
        log_file_path,
        log_spec,
        stdout_only,
        color,
    } = options;

    let (records_tx, records_rx): (Sender<LogRecord>, Receiver<LogRecord>) =
        channel::bounded(LOG_RECORD_BUFFER_SIZE);
    let record_sender: Box<dyn LogWriter> = Box::new(LogRecordSender::new(records_tx));

    // NOTE: The format is set for every writer here and then overridden for stdout below, so that
    // the log files are never colored.
    let mut logger = Logger::with(log_spec.clone()).format(log_format);
    logger = match color {
        Color::Always => logger.format_for_stdout(colored_log_format),
        Color::Never => logger,
        Color::Auto => logger
            .adaptive_format_for_stdout(AdaptiveFormat::Custom(log_format, colored_log_format)),
    };

    if *stdout_only {
        logger = logger.log_to_stdout();
    } else if let Some(log_file_path) = log_file_path {
        logger =
            logger.log_to_file_and_writer(FileSpec::try_from(log_file_path).unwrap(), record_sender)
    } else {
        // The logger makes the directories it needs with permissions that let the group and others
        // in, so make them first. Logging is not configured yet, so there is nothing to log to.
        if let Err(error) = make_private_dir(&INSHD_LOGS_DIR) {
            eprintln!("Failed to create the log directory: {}.", error);
        }

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
    /// Whether or not to color the logs.
    #[builder(default = Color::Auto)]
    color: Color,
}

/// Whether or not to color the logs.
#[derive(Clone, Copy, Debug)]
pub enum Color {
    /// Always color the logs.
    Always,
    /// Never color the logs.
    Never,
    /// Color the logs only when they are being written to a terminal.
    Auto,
}

impl Color {
    /// Return whether or not the logs written to standard output should be colored.
    pub fn color_stdout(&self) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => stdout().is_terminal(),
        }
    }
}

/// Sends log records to be streamed to clients.
struct LogRecordSender {
    /// A sender of log records.
    records_tx: Sender<LogRecord>,
}

impl LogRecordSender {
    /// Return a new sender of log records.
    fn new(records_tx: Sender<LogRecord>) -> Self {
        Self { records_tx }
    }
}

impl LogWriter for LogRecordSender {
    // NOTE: This must never block and must never log. It is called by every thread which logs, so
    // blocking here would stall all logging in the daemon, and logging here would be recursive.
    // Records are dropped if no one is receiving them or if the receiver has fallen behind.
    fn write(&self, now: &mut DeferredNow, record: &Record) -> Result<(), IOError> {
        let _ = self.records_tx.try_send(log_record(now, record));
        Ok(())
    }

    fn flush(&self) -> Result<(), IOError> {
        Ok(())
    }
}

/// Return the log record which a record of the logging framework corresponds to.
fn log_record(now: &mut DeferredNow, record: &Record) -> LogRecord {
    LogRecord::builder()
        .timestamp(now.now().format("%d-%m-%Y %H:%M.%S").to_string())
        .level(record.level().log_level())
        .module(record.module_path().unwrap_or("<unnamed>").to_string())
        .thread(thread::current().name().unwrap_or("<unnamed>").to_string())
        .message(record.args().to_string())
        .build()
}

/// Conversion to a log level.
trait ToLogLevel {
    /// Return the log level which this corresponds to.
    fn log_level(&self) -> LogLevel;
}

impl ToLogLevel for Level {
    fn log_level(&self) -> LogLevel {
        match self {
            Self::Error => LogLevel::Error,
            Self::Warn => LogLevel::Warn,
            Self::Info => LogLevel::Info,
            Self::Debug => LogLevel::Debug,
            Self::Trace => LogLevel::Trace,
        }
    }
}

/// Format log records.
pub fn log_format(
    writer: &mut dyn Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), IOError> {
    write!(writer, "{}", log_record(now, record))
}

/// Format log records with color.
pub fn colored_log_format(
    writer: &mut dyn Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), IOError> {
    write!(
        writer,
        "{}",
        ColoredLogRecord::new(&log_record(now, record))
    )
}

/// A log record which is displayed with color.
pub struct ColoredLogRecord<'a> {
    /// The log record.
    record: &'a LogRecord,
}

impl<'a> ColoredLogRecord<'a> {
    /// Return a new colored log record.
    pub fn new(record: &'a LogRecord) -> Self {
        Self { record }
    }
}

impl Display for ColoredLogRecord<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        let level = self.record.level();
        let level_color: AnsiColor = match level {
            LogLevel::Error => AnsiColor::BrightRed,
            LogLevel::Warn => AnsiColor::Yellow,
            LogLevel::Info => AnsiColor::BrightGreen,
            LogLevel::Debug => AnsiColor::BrightBlue,
            LogLevel::Trace => AnsiColor::BrightBlack,
        };

        write!(
            formatter,
            "{} {} {} {} {}",
            Colored::new(self.record.timestamp(), AnsiColor::BrightBlue),
            Colored::new(level, level_color),
            Colored::new(
                format!("[{}]", self.record.module()),
                AnsiColor::BrightBlack
            ),
            Colored::new(
                format!("[{}]", self.record.thread()),
                AnsiColor::BrightBlack
            ),
            self.record.message()
        )
    }
}

/// Text which is written in a color.
struct Colored<T: Display> {
    /// The text.
    text: T,
    /// The color to write it in.
    color: AnsiColor,
}

impl<T: Display> Colored<T> {
    /// Return text which is written in the given color.
    fn new(text: T, color: AnsiColor) -> Self {
        Self { text, color }
    }
}

impl<T: Display> Display for Colored<T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        let start =
            ControlFunction::SelectGraphicRendition(vec![GraphicRendition::Foreground(self.color)]);
        let end = ControlFunction::SelectGraphicRendition(vec![GraphicRendition::Reset]);

        write!(formatter, "{}{}{}", start, self.text, end)
    }
}
