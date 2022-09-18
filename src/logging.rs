/// Logging for debug purposes.
use std::fmt::{Display, Error as FormatError, Formatter};
use std::path::PathBuf;

use flexi_logger::writers::FileLogWriter;
use flexi_logger::{FileSpec, LogSpecification, Logger, LoggerHandle};

/// Configure logging.
pub fn configure_logging(path: PathBuf) -> ConfigureLoggingResult {
    let log_specification: LogSpecification = LogSpecification::info();
    let mut logger = Logger::with(log_specification);

    let file_log_writer = FileLogWriter::builder(FileSpec::try_from(path).unwrap())
        .try_build()
        .unwrap();
    logger = logger.log_to_writer(Box::new(file_log_writer));

    let logger_handle = logger.start().unwrap();

    Ok(logger_handle)
}

pub type ConfigureLoggingResult = Result<LoggerHandle, ConfigureLoggingError>;

pub enum ConfigureLoggingError {}

impl Display for ConfigureLoggingError {
    fn fmt(&self, _formatter: &mut Formatter<'_>) -> Result<(), FormatError> {
        todo!();
    }
}
