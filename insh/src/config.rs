/*!
Configuration options loaded from the YAML file `~/.insh-config` if it exists.
*/

/// Configuration options.
mod config {
    use std::fmt::{Display, Formatter, Result as FormatResult};
    use std::fs::File;
    use std::io::{Error as IOError, ErrorKind as IOErrorKind};
    use std::path::PathBuf;

    use super::{BrowserConfig, GeneralConfig, InputConfig, RenderConfig};

    use serde::Deserialize;
    use serde_yaml_ng::Error as YamlParseError;

    /// Configuration options.
    #[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
    pub struct Config {
        /// General configuration.
        #[serde(default)]
        general: GeneralConfig,
        /// Input configuration.
        #[serde(default)]
        input: InputConfig,
        /// Configuration of how the screen is drawn.
        #[serde(default)]
        render: RenderConfig,
        /// Configuration of the Browser.
        #[serde(default)]
        browser: BrowserConfig,
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

            match serde_yaml_ng::from_reader(file) {
                Ok(config) => Ok(config),
                Err(error) => Err(ConfigLoadError::ParseError { path, error }),
            }
        }

        /// Return the general configuration.
        pub fn general(&self) -> &GeneralConfig {
            &self.general
        }

        /// Return the configuration of how input is read.
        pub fn input(&self) -> &InputConfig {
            &self.input
        }

        /// Return the configuration of how the screen is drawn.
        pub fn render(&self) -> &RenderConfig {
            &self.render
        }

        /// Return the browser configuration.
        pub fn browser(&self) -> &BrowserConfig {
            &self.browser
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
        /// One of the default paths could not be determined, such as the home directory.
        ConfigDefaultPathError(ConfigDefaultPathError),
        /// Permission was denied reading the configuration file.
        PermissionDeniedError(PathBuf),
        /// The configuration file could not be read for some other reason.
        OtherFileReadError {
            /// The path of the configuration file.
            path: PathBuf,
            /// The IO error.
            error: IOError,
        },
        /// An error parsing the configuration file.
        ParseError {
            /// The path of the configuration file.
            path: PathBuf,
            /// The YAML parse error.
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
pub use config::Config;

/// Contains general configuration.
mod general {
    use serde::Deserialize;

    /// General configuration options.
    #[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
    pub struct GeneralConfig {
        /// The width of tab characters.
        #[serde(default)]
        tab_width: usize,

        /// Whether the bell sound should be made.
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

        /// Return whether the bell sound should be made.
        pub fn bell(&self) -> bool {
            self.bell
        }
    }
}
pub use general::GeneralConfig;

/// Contains configuration of how input is read.
mod input {
    use std::time::Duration;

    use serde::de::Error as DeserializeError;
    use serde::{Deserialize, Deserializer};

    /// Configuration of how input is read.
    #[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
    pub struct InputConfig {
        /// How many milliseconds to wait for the rest of an escape sequence before deciding that
        /// the escape key was pressed on its own.
        #[serde(
            default = "default_escape_timeout",
            deserialize_with = "deserialize_escape_timeout"
        )]
        escape_timeout: u64,
    }

    /// Return how long to wait for the rest of an escape sequence by default.
    fn default_escape_timeout() -> u64 {
        50
    }

    /// Return how long to wait for the rest of an escape sequence, refusing to wait no time at all.
    ///
    /// Waiting is the only thing which tells a press of the escape key apart from the start of a
    /// sequence, so not waiting means every sequence is read as an escape and then the rest of it
    /// as the characters it is spelled with: an arrow key would type `[A`.
    fn deserialize_escape_timeout<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let milliseconds = u64::deserialize(deserializer)?;

        if milliseconds == 0 {
            return Err(D::Error::custom(
                "the escape timeout cannot be zero, because then no escape sequence would ever be \
                 read as the key it is for",
            ));
        }

        Ok(milliseconds)
    }

    impl Default for InputConfig {
        fn default() -> Self {
            Self {
                escape_timeout: default_escape_timeout(),
            }
        }
    }

    impl InputConfig {
        /// Return how long to wait for the rest of an escape sequence before deciding that the
        /// escape key was pressed on its own.
        ///
        /// Waiting is what tells a press of the escape key apart from the start of a sequence,
        /// which look the same until the rest of the sequence does or does not turn up. Longer is
        /// more reliable over a slow connection, at the cost of the escape key feeling that much
        /// less responsive.
        pub fn escape_timeout(&self) -> Duration {
            Duration::from_millis(self.escape_timeout)
        }
    }
}
pub use input::InputConfig;

/// Contains configuration of how the screen is drawn.
mod render {
    use serde::Deserialize;

    /// Configuration of how the screen is drawn.
    #[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
    pub struct RenderConfig {
        /// How much of the screen is drawn.
        #[serde(default)]
        engine: RenderEngineConfig,
    }

    impl RenderConfig {
        /// Return how much of the screen is drawn.
        pub fn engine(&self) -> RenderEngineConfig {
            self.engine
        }
    }

    /// How much of the screen is drawn.
    #[derive(Deserialize, Debug, Default, Clone, Copy, Eq, PartialEq)]
    #[serde(rename_all = "lowercase")]
    pub enum RenderEngineConfig {
        /// All of the screen is drawn every time.
        Full,
        /// Only the parts of the screen which changed are drawn.
        #[default]
        Incr,
    }
}
pub use render::{RenderConfig, RenderEngineConfig};

/// Contains browser configuration.
mod browser {
    use insh_api::{FileSortOptions, HiddenFileSort};

    use serde::Deserialize;

    /// Configuration for the Browser.
    #[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
    pub struct BrowserConfig {
        /// How the files shown in the browser are sorted.
        #[serde(default = "default_sort")]
        sort: Option<BrowserSortConfig>,

        /// Whether the metadata of the files is shown.
        #[serde(default)]
        metadata: bool,
    }

    /// Return how the files are sorted by default.
    fn default_sort() -> Option<BrowserSortConfig> {
        Some(BrowserSortConfig::default())
    }

    impl Default for BrowserConfig {
        fn default() -> Self {
            Self {
                sort: default_sort(),
                metadata: false,
            }
        }
    }

    impl BrowserConfig {
        /// Return how the files shown in the browser are sorted.
        pub fn sort(&self) -> Option<&BrowserSortConfig> {
            self.sort.as_ref()
        }

        /// Return whether the metadata of the files is shown in the Browser.
        pub fn metadata(&self) -> bool {
            self.metadata
        }
    }

    /// Configuration for how the files are sorted.
    #[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
    pub struct BrowserSortConfig {
        /// Whether the case of filenames is ignored.
        #[serde(default = "enabled")]
        case_insensitive: bool,

        /// How hidden files are sorted.
        #[serde(default)]
        hidden: BrowserSortHiddenConfig,
    }

    /// Return that an option is enabled by default.
    fn enabled() -> bool {
        true
    }

    impl Default for BrowserSortConfig {
        fn default() -> Self {
            Self {
                case_insensitive: enabled(),
                hidden: BrowserSortHiddenConfig::default(),
            }
        }
    }

    impl BrowserSortConfig {
        /// Return whether the case of filenames is ignored.
        pub fn case_insensitive(&self) -> bool {
            self.case_insensitive
        }

        /// Return how hidden files are sorted.
        pub fn hidden(&self) -> BrowserSortHiddenConfig {
            self.hidden
        }
    }

    impl From<&BrowserSortConfig> for FileSortOptions {
        fn from(sort: &BrowserSortConfig) -> Self {
            FileSortOptions::builder()
                .case_insensitive(sort.case_insensitive)
                .hidden(sort.hidden.into())
                .build()
        }
    }

    /// How hidden files are sorted.
    #[derive(Deserialize, Debug, Default, Clone, Copy, Eq, PartialEq)]
    #[serde(rename_all = "lowercase")]
    pub enum BrowserSortHiddenConfig {
        /// Hidden files are sorted before all of the other files.
        First,
        /// Hidden files are sorted after all of the other files.
        #[default]
        Last,
        /// Hidden files are sorted as if they were not hidden.
        Mixed,
    }

    impl From<BrowserSortHiddenConfig> for HiddenFileSort {
        fn from(hidden: BrowserSortHiddenConfig) -> Self {
            match hidden {
                BrowserSortHiddenConfig::First => HiddenFileSort::First,
                BrowserSortHiddenConfig::Last => HiddenFileSort::Last,
                BrowserSortHiddenConfig::Mixed => HiddenFileSort::Mixed,
            }
        }
    }
}
pub use browser::BrowserConfig;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    use test_case::test_case;

    #[test_case("render:\n  engine: full\n", RenderEngineConfig::Full; "drawing all of the screen")]
    #[test_case("render:\n  engine: incr\n", RenderEngineConfig::Incr; "drawing only what changed")]
    #[test_case("render: {}\n", RenderEngineConfig::Incr; "only what changed by default")]
    #[test_case("", RenderEngineConfig::Incr; "only what changed when nothing is configured")]
    fn test_the_render_engine_can_be_set(yaml: &str, expected_engine: RenderEngineConfig) {
        let config: Config = serde_yaml_ng::from_str(yaml).unwrap();

        assert_eq!(config.render().engine(), expected_engine);
    }

    #[test]
    fn test_the_escape_timeout_can_be_set() {
        let config: Config = serde_yaml_ng::from_str("input:\n  escape_timeout: 250\n").unwrap();

        assert_eq!(config.input().escape_timeout(), Duration::from_millis(250));
    }

    #[test]
    fn test_an_escape_timeout_of_zero_is_refused() {
        let error = serde_yaml_ng::from_str::<Config>("input:\n  escape_timeout: 0\n")
            .expect_err("An escape timeout of zero should not be allowed.");

        assert!(
            error.to_string().contains("cannot be zero"),
            "unhelpful error: {}",
            error
        );
    }

    #[test]
    fn test_the_escape_timeout_has_a_default() {
        assert_eq!(
            Config::default().input().escape_timeout(),
            Duration::from_millis(50)
        );
        assert_eq!(
            serde_yaml_ng::from_str::<Config>("input: {}\n")
                .unwrap()
                .input()
                .escape_timeout(),
            Duration::from_millis(50)
        );
    }
}
