mod module_log_level_filter {
    use super::ModuleLevelFilterParseError;

    use std::str::FromStr;

    use flexi_logger::LevelFilter as LogLevelFilter;

    /// A log level filter for a particular module.
    #[derive(Debug)]
    pub struct ModuleLogLevelFilter {
        /// The name of the module.
        module_name: String,
        /// The log level filter for the module.
        log_level_filter: LogLevelFilter,
    }

    impl ModuleLogLevelFilter {
        pub fn module_name(&self) -> &str {
            &self.module_name
        }

        pub fn log_level_filter(&self) -> &LogLevelFilter {
            &self.log_level_filter
        }
    }

    impl FromStr for ModuleLogLevelFilter {
        type Err = ModuleLevelFilterParseError;

        fn from_str(string: &str) -> Result<Self, Self::Err> {
            let split_result: Option<(&str, &str)> = string.split_once('=');
            let (module_name, log_level_filter_string) = match split_result {
                None => {
                    return Err(ModuleLevelFilterParseError::NoEqualsSign {
                        bad_module_log_level: string.to_string(),
                    })
                }
                Some(strings) => strings,
            };

            let log_level_filter = match LogLevelFilter::from_str(log_level_filter_string) {
                Err(_) => {
                    return Err(ModuleLevelFilterParseError::BadLogLevel {
                        bad_module_log_level: string.to_string(),
                        bad_log_level: log_level_filter_string.to_string(),
                    })
                }
                Ok(log_level_filter) => log_level_filter,
            };

            Ok(ModuleLogLevelFilter {
                module_name: module_name.to_string(),
                log_level_filter,
            })
        }
    }
}
pub use module_log_level_filter::ModuleLogLevelFilter;

mod module_log_level_filter_parse_error {
    use std::error::Error;
    use std::fmt::{Display, Error as FmtError, Formatter};

    /// An error for parsing a module level filter from a string.
    #[derive(Debug)]
    pub enum ModuleLevelFilterParseError {
        NoEqualsSign {
            bad_module_log_level: String,
        },
        BadLogLevel {
            bad_module_log_level: String,
            bad_log_level: String,
        },
    }

    impl Display for ModuleLevelFilterParseError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::NoEqualsSign {
                    bad_module_log_level,
                } => {
                    write!(formatter, "Failed to parse the module log level \"{}\" because it contains no equals sign. Module log levels should be of the form \"<module-name>=<log-level>\".", bad_module_log_level)
                }
                Self::BadLogLevel {
                    bad_module_log_level,
                    bad_log_level,
                } => {
                    write!(formatter, "Failed to parse the module log level \"{}\" because \"{}\" is not a valid log level.", bad_module_log_level, bad_log_level)
                }
            }
        }
    }

    impl Error for ModuleLevelFilterParseError {}
}
use module_log_level_filter_parse_error::ModuleLevelFilterParseError;
