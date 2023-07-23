/*!
Configuration options loaded from the YAML file `~/.insh-config` if it exists.
*/

/// Configuration options.
mod config {
    use super::{GeneralConfig, SearcherConfig};

    use std::fmt::{Display, Formatter, Result as FormatResult};
    use std::fs::File;
    use std::io::{Error as IOError, ErrorKind as IOErrorKind};
    use std::path::PathBuf;

    use serde::Deserialize;
    use serde_yaml::Error as YamlParseError;

    /// Configuration options.
    #[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
    pub struct Config {
        /// General configuration.
        #[serde(default)]
        general: GeneralConfig,
        /// Configuration of the Searcher.
        #[serde(default)]
        searcher: SearcherConfig,
    }

    impl Config {
        /// Return the default path of the file that configuration is loaded from.
        pub fn default_path() -> ConfigDefaultPathResult {
            let mut path: PathBuf = match dirs::home_dir() {
                Some(path) => path,
                None => {
                    return Err(ConfigDefaultPathError::CannotDetermineHomeDirectory);
                }
            };
            path.push(".insh-config.yaml");
            Ok(path)
        }

        /// Return the `Config` loaded from the default file if it exists or the default config if the
        /// file does not exist. If there is an error then return a `ConfigLoadError`.
        pub fn load() -> ConfigLoadResult {
            let path: PathBuf = match Self::default_path() {
                Ok(path) => path,
                Err(error) => {
                    return Err(ConfigLoadError::ConfigDefaultPathError(error));
                }
            };

            let file: File = match File::open(path.clone()) {
                Ok(file) => file,
                Err(error) => match error.kind() {
                    IOErrorKind::NotFound => {
                        return Ok(Config::default());
                    }
                    IOErrorKind::PermissionDenied => {
                        return Err(ConfigLoadError::PermissionDeniedError(path));
                    }
                    _ => {
                        return Err(ConfigLoadError::OtherFileReadError { path, error });
                    }
                },
            };

            match serde_yaml::from_reader(file) {
                Ok(config) => Ok(config),
                Err(error) => Err(ConfigLoadError::ParseError { path, error }),
            }
        }

        /// Return the general configuration.
        pub fn general(&self) -> &GeneralConfig {
            &self.general
        }

        /// Return the searcher configuration.
        pub fn searcher(&self) -> &SearcherConfig {
            &self.searcher
        }
    }

    /// The result of trying to determine a default path.
    type ConfigDefaultPathResult = Result<PathBuf, ConfigDefaultPathError>;

    /// A problem with determining a default path.
    pub enum ConfigDefaultPathError {
        /// The home directory could not be determined.
        CannotDetermineHomeDirectory,
    }

    /// The result of trying to load the configuration file.
    type ConfigLoadResult = Result<Config, ConfigLoadError>;

    /// An error loading the configuration file.
    #[allow(clippy::enum_variant_names)]
    pub enum ConfigLoadError {
        /// An error when the there is a problem determining one of the default paths that is used
        /// in determining the configuration file. For example, if the home directory cannot be
        /// determined.
        ConfigDefaultPathError(ConfigDefaultPathError),
        /// An error when permission is denied while trying to read the configuration file.
        PermissionDeniedError(PathBuf),
        /// An generic error with reading the configuration file.
        OtherFileReadError {
            /// The path of the configuration file the there was a problem reading.
            path: PathBuf,
            /// The IO error that was encountered while attempting to read the configuration file.
            error: IOError,
        },
        /// An error parsing the configuration file.
        ParseError {
            /// The path of the configuration file the there was a problem parsing.
            path: PathBuf,
            /// An error parsing the configuration file as YAML.
            error: YamlParseError,
        },
    }

    impl Display for ConfigLoadError {
        fn fmt(&self, f: &mut Formatter<'_>) -> FormatResult {
            match self {
                Self::ConfigDefaultPathError(error) => match error {
                    ConfigDefaultPathError::CannotDetermineHomeDirectory => {
                        write!(f, "Failed to load the configuration because the home directory could not be determined.")
                    }
                },
                Self::PermissionDeniedError(path) => {
                    write!(
                        f,
                        "Failed to load the configuration file \"{}\" because permission was denied.",
                        path.display()
                    )
                }
                Self::OtherFileReadError { path, error } => {
                    write!(
                        f,
                        "Failed to load the configuration file \"{}\" because of an IO error: {}",
                        path.display(),
                        error
                    )
                }
                Self::ParseError { path, error } => {
                    write!(
                        f,
                        "Failed to parse the configuration file \"{}\": {}",
                        path.display(),
                        error
                    )
                }
            }
        }
    }
}
pub use config::{Config, ConfigLoadError};

/// Contains general configuration.
mod general {
    use serde::Deserialize;

    /// General configuration options.
    #[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
    pub struct GeneralConfig {
        /// The width of tab characters.
        #[serde(default)]
        tab_width: usize,

        /// Whether the bell sound should be made or not.
        #[serde(default)]
        bell: bool,
    }

    impl Default for GeneralConfig {
        fn default() -> Self {
            Self {
                tab_width: 4,
                bell: true,
            }
        }
    }

    impl GeneralConfig {
        /// Return the width of tab characters.
        pub fn tab_width(&self) -> usize {
            self.tab_width
        }

        /// Return whether the bell sound should be made or not.
        pub fn bell(&self) -> bool {
            self.bell
        }
    }
}
pub use general::GeneralConfig;

/// Contains search configuration.
mod search {
    use serde::Deserialize;

    /// Configuration for the Searcher.
    #[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
    pub struct SearcherConfig {
        /// Configuration for the Searcher history.
        #[serde(default)]
        history: SearcherHistoryConfig,
    }

    impl SearcherConfig {
        /// Return the searcher history configuration.
        pub fn history(&self) -> &SearcherHistoryConfig {
            &self.history
        }
    }

    /// Configuration for the Searcher history.
    #[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
    pub struct SearcherHistoryConfig {
        /// The maximum length of the searcher history.
        #[serde(default)]
        length: usize,
    }

    impl Default for SearcherHistoryConfig {
        fn default() -> Self {
            Self { length: 1000 }
        }
    }

    impl SearcherHistoryConfig {
        /// Return the maximum length of the searcher history.
        pub fn length(&self) -> usize {
            self.length
        }
    }
}
pub use search::SearcherConfig;
