/*!
Configuration options loaded from the YAML file `~/.inshd-config.yaml` if it exists.
*/
use std::fs::File;
use std::io::ErrorKind as IOErrorKind;
use std::path::PathBuf;

use common::paths::HOME_DIR;
use serde::Deserialize;

/// Configuration options.
#[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
pub struct Config {
    /// Configuration of the Searcher.
    #[serde(default)]
    searcher: SearcherConfig,
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
    pub fn load() -> Self {
        let path: PathBuf = Self::default_path();

        let file: File = match File::open(&path) {
            Ok(file) => file,
            Err(error) => match error.kind() {
                IOErrorKind::NotFound => {
                    return Config::default();
                }
                _ => {
                    panic!(
                        "Could not read the configuration file {:?}. Encountered the following error: {}",
                        path, error
                    );
                }
            },
        };

        match serde_yaml::from_reader(file) {
            Ok(config) => config,
            Err(error) => {
                panic!(
                    "Could not parse the configuration file {:?}. Encountered the following error: {}",
                    path, error
                );
            }
        }
    }

    /// Return the searcher configuration.
    pub fn searcher(&self) -> &SearcherConfig {
        &self.searcher
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
