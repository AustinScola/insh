/*!
Configuration options loaded from the YAML file `~/.inshd-config.yaml` if it exists.
*/
use std::fs::File;
use std::io::ErrorKind as IOErrorKind;
use std::path::PathBuf;

use common::paths::HOME_DIR;

use serde::Deserialize;

/// The number of request handlers to run.
const DEFAULT_NUM_REQUEST_HANDLERS: usize = 8;

/// The maximum length of the Browser history.
const DEFAULT_BROWSER_HISTORY_LENGTH: usize = 1000;

/// The maximum length of the Finder history.
const DEFAULT_FINDER_HISTORY_LENGTH: usize = 1000;

/// Configuration options.
#[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
pub struct Config {
    /// Configuration of the Browser.
    #[serde(default)]
    browser: BrowserConfig,
    /// Configuration of the Finder.
    #[serde(default)]
    finder: FinderConfig,
    /// Configuration of the Searcher.
    #[serde(default)]
    searcher: SearcherConfig,
    /// Configuration of the database.
    #[serde(default)]
    database: DatabaseConfig,
    /// Configuration of the server.
    #[serde(default)]
    server: ServerConfig,
}

impl Config {
    /// Return the default path of the file that configuration is loaded from.
    pub fn default_path() -> PathBuf {
        let mut path: PathBuf = HOME_DIR.clone();
        path.push(".inshd-config.yaml");
        path
    }

    /// Return the `Config` loaded from the default file if it exists or the default config if the
    /// file does not exist.
    pub fn load() -> Result<Self, LoadError> {
        let path: PathBuf = Self::default_path();

        let file: File = match File::open(&path) {
            Ok(file) => file,
            Err(error) => match error.kind() {
                IOErrorKind::NotFound => {
                    return Ok(Config::default());
                }
                _ => {
                    return Err(LoadError::ReadFailed { path, error });
                }
            },
        };

        let config: Config = match serde_yaml_ng::from_reader(file) {
            Ok(config) => config,
            Err(error) => {
                return Err(LoadError::ParseFailed { path, error });
            }
        };
        config.check()?;

        Ok(config)
    }

    /// Return an error if the configuration cannot be used.
    fn check(&self) -> Result<(), LoadError> {
        if self.server.request_handlers.num == 0 {
            return Err(LoadError::NoRequestHandlers);
        }
        if self.db_conn_pool_size() == 0 {
            return Err(LoadError::EmptyDbConnPool);
        }

        Ok(())
    }

    /// Return the browser configuration.
    pub fn browser(&self) -> &BrowserConfig {
        &self.browser
    }

    /// Return the finder configuration.
    pub fn finder(&self) -> &FinderConfig {
        &self.finder
    }

    /// Return the searcher configuration.
    pub fn searcher(&self) -> &SearcherConfig {
        &self.searcher
    }

    /// Return the server configuration.
    pub fn server(&self) -> &ServerConfig {
        &self.server
    }

    /// Return the maximum number of connections to the database.
    ///
    /// This defaults to the number of request handlers, so that every request handler is able to be
    /// using a connection at the same time. It is resolved here because it depends on another part
    /// of the configuration.
    pub fn db_conn_pool_size(&self) -> u32 {
        self.database
            .pool
            .size
            .unwrap_or(self.server.request_handlers.num as u32)
    }
}

/// Configuration for the server.
#[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
pub struct ServerConfig {
    /// Configuration for the request handlers.
    #[serde(default)]
    request_handlers: RequestHandlersConfig,
}

impl ServerConfig {
    /// Return the configuration for the request handlers.
    pub fn request_handlers(&self) -> &RequestHandlersConfig {
        &self.request_handlers
    }
}

/// Configuration for the request handlers.
#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct RequestHandlersConfig {
    /// The number of request handlers to run.
    #[serde(default = "RequestHandlersConfig::default_num")]
    num: usize,
}

impl Default for RequestHandlersConfig {
    fn default() -> Self {
        Self {
            num: Self::default_num(),
        }
    }
}

impl RequestHandlersConfig {
    /// Return the default number of request handlers to run.
    fn default_num() -> usize {
        DEFAULT_NUM_REQUEST_HANDLERS
    }

    /// Return the number of request handlers to run.
    pub fn num(&self) -> usize {
        self.num
    }
}

/// Configuration for the database.
#[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
pub struct DatabaseConfig {
    /// Configuration for the pool of connections to the database.
    #[serde(default)]
    pool: DatabasePoolConfig,
}

/// Configuration for the pool of connections to the database.
#[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
pub struct DatabasePoolConfig {
    /// The maximum number of connections to the database.
    ///
    /// When this is not set the number of request handlers is used, which
    /// [`Config::db_conn_pool_size`] resolves.
    #[serde(default)]
    size: Option<u32>,
}

/// Configuration for the Browser.
#[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
pub struct BrowserConfig {
    /// Configuration for the Browser history.
    #[serde(default)]
    history: BrowserHistoryConfig,
}

impl BrowserConfig {
    /// Return the browser history configuration.
    pub fn history(&self) -> &BrowserHistoryConfig {
        &self.history
    }
}

/// Configuration for the Browser history.
#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct BrowserHistoryConfig {
    /// The maximum length of the browser history.
    #[serde(default = "BrowserHistoryConfig::default_length")]
    length: usize,
}

impl Default for BrowserHistoryConfig {
    fn default() -> Self {
        Self {
            length: Self::default_length(),
        }
    }
}

impl BrowserHistoryConfig {
    /// Return the default maximum length of the browser history.
    fn default_length() -> usize {
        DEFAULT_BROWSER_HISTORY_LENGTH
    }

    /// Return the maximum length of the browser history.
    pub fn length(&self) -> usize {
        self.length
    }
}

/// Configuration for the Finder.
#[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
pub struct FinderConfig {
    /// Configuration for the Finder history.
    #[serde(default)]
    history: FinderHistoryConfig,
}

impl FinderConfig {
    /// Return the finder history configuration.
    pub fn history(&self) -> &FinderHistoryConfig {
        &self.history
    }
}

/// Configuration for the Finder history.
#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct FinderHistoryConfig {
    /// The maximum length of the finder history.
    #[serde(default = "FinderHistoryConfig::default_length")]
    length: usize,
}

impl Default for FinderHistoryConfig {
    fn default() -> Self {
        Self {
            length: Self::default_length(),
        }
    }
}

impl FinderHistoryConfig {
    /// Return the default maximum length of the finder history.
    fn default_length() -> usize {
        DEFAULT_FINDER_HISTORY_LENGTH
    }

    /// Return the maximum length of the finder history.
    pub fn length(&self) -> usize {
        self.length
    }
}

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

mod load_error {
    //! An error loading the configuration.

    use std::fmt::{Display, Error as FmtError, Formatter};
    use std::io::Error as IOError;
    use std::path::PathBuf;

    use serde_yaml_ng::Error as YamlError;

    /// An error loading the configuration.
    pub enum LoadError {
        /// An error reading the configuration file.
        ReadFailed {
            /// The path of the configuration file.
            path: PathBuf,
            /// The error which was encountered.
            error: IOError,
        },
        /// An error parsing the configuration file.
        ParseFailed {
            /// The path of the configuration file.
            path: PathBuf,
            /// The error which was encountered.
            error: YamlError,
        },
        /// There are no request handlers to handle requests with.
        NoRequestHandlers,
        /// There are no connections to the database to handle requests with.
        EmptyDbConnPool,
    }

    impl Display for LoadError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
            match self {
                Self::ReadFailed { path, error } => {
                    write!(
                        formatter,
                        "Failed to read the configuration file {:?}: {}",
                        path, error
                    )
                }
                Self::ParseFailed { path, error } => {
                    write!(
                        formatter,
                        "Failed to parse the configuration file {:?}: {}",
                        path, error
                    )
                }
                Self::NoRequestHandlers => {
                    write!(
                        formatter,
                        "The number of request handlers has to be more than zero"
                    )
                }
                Self::EmptyDbConnPool => {
                    write!(
                        formatter,
                        "The size of the database connection pool has to be more than zero"
                    )
                }
            }
        }
    }
}
pub use load_error::LoadError;
