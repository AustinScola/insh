mod config {
    use super::{GeneralConfig, SearcherConfig};

    use std::fmt::{Display, Formatter, Result as FormatResult};
    use std::fs::File;
    use std::io::{Error as IOError, ErrorKind as IOErrorKind};
    use std::path::PathBuf;

    use serde::Deserialize;
    use serde_yaml::Error as YamlParseError;

    #[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
    pub struct Config {
        #[serde(default)]
        general: GeneralConfig,
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

    type ConfigDefaultPathResult = Result<PathBuf, ConfigDefaultPathError>;

    pub enum ConfigDefaultPathError {
        CannotDetermineHomeDirectory,
    }

    type ConfigLoadResult = Result<Config, ConfigLoadError>;

    #[allow(clippy::enum_variant_names)]
    pub enum ConfigLoadError {
        ConfigDefaultPathError(ConfigDefaultPathError),
        PermissionDeniedError(PathBuf),
        OtherFileReadError {
            path: PathBuf,
            error: IOError,
        },
        ParseError {
            path: PathBuf,
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

mod general {
    use serde::Deserialize;

    #[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
    pub struct GeneralConfig {
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

mod search {
    use serde::Deserialize;

    #[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
    pub struct SearcherConfig {
        #[serde(default)]
        history: SearcherHistoryConfig,
    }

    impl SearcherConfig {
        /// Return the searcher history configuration.
        pub fn history(&self) -> &SearcherHistoryConfig {
            &self.history
        }
    }

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
