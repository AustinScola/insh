/*!
Configuration options loaded from the YAML file `~/.insh/inshd-config.yaml` if it exists.
*/
use std::fs::{metadata, File};
use std::io::ErrorKind as IOErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use common::paths::INSH_DIR;

use ai_client::Api;
use serde::Deserialize;

/// The number of request handlers to run.
const DEFAULT_NUM_REQUEST_HANDLERS: usize = 8;

/// The maximum length of the Browser history.
const DEFAULT_BROWSER_HISTORY_LENGTH: usize = 1000;

/// The maximum length of the Finder history.
const DEFAULT_FINDER_HISTORY_LENGTH: usize = 1000;

/// The model which replies in a chat.
const DEFAULT_AI_MODEL: &str = "claude-opus-5";

/// What the inference engine is told about how to answer, when nothing is configured to replace
/// it.
const DEFAULT_AI_INSTRUCTIONS: &str = r#"# General
- Be concise.
- Hard cap: 30 words; only more if depth is explicitly asked for.
- No preamble.
- Do not include context in answers.
- Here is an example of how concise I want. If I ask "what color is the sky?" answer "blue" (don't even bother with a period).
- Use bullets when appropriate (for example when ask you to list things).

# Code
- End comments with periods.
"#;

/// The permission bits which let anyone other than the owner read a file.
const OTHERS_CAN_READ: u32 = 0o077;

/// How far apart the vectors of two texts can be before a search stops calling them a match.
///
/// This is a cosine distance, where nothing at all alike is around one. Measured with the model
/// which is embedded, over a set of chat messages: what a search is really after lands under 0.63
/// ("burrito" and "how do I make a burrito" are 0.12 apart, "shell scripting" and "write a bash
/// script to rename every file in a directory" are 0.51), while the nearest thing which is not
/// what was meant is 0.79 ("dinner" and "the best taco recipe") and the rest of an unrelated set
/// sits around 0.9 to 1.1. The gap in the middle is where this goes.
const DEFAULT_AI_SEARCH_THRESHOLD: f64 = 0.7;

/// Configuration options.
#[derive(Deserialize, Debug, Default, Clone, PartialEq)]
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
    /// Configuration of the AI inference engine.
    #[serde(default)]
    ai: AiConfig,
}

impl Config {
    /// Return the default path of the file that configuration is loaded from.
    pub fn default_path() -> PathBuf {
        let mut path: PathBuf = INSH_DIR.clone();
        path.push("inshd-config.yaml");
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
        Self::check_perms(&path);

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

    /// Return the AI configuration.
    pub fn ai(&self) -> &AiConfig {
        &self.ai
    }

    /// Warn if anyone other than the owner can read the configuration file.
    ///
    /// The API key is kept in this file, and nothing else checks how it is protected.
    fn check_perms(path: &Path) {
        let metadata = match metadata(path) {
            Ok(metadata) => metadata,
            Err(_) => return,
        };

        if metadata.permissions().mode() & OTHERS_CAN_READ != 0 {
            log::warn!(
                "The configuration file {:?} can be read by users other than you, and it is where \
                 the AI API key is kept. Consider `chmod 600 {}`.",
                path,
                path.display()
            );
        }
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

/// Configuration for the AI inference engine.
///
/// Nothing here has to be set. When it is not, insh says that no inference engine is configured
/// rather than treating it as a mistake, since most people will not use the chat at all.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct AiConfig {
    /// The URL of the inference engine, without a path.
    #[serde(default)]
    base_url: Option<String>,
    /// The key which authenticates requests.
    #[serde(default)]
    api_key: Option<String>,
    /// The model which replies.
    #[serde(default = "AiConfig::default_model")]
    model: String,
    /// The kind of API the inference engine speaks.
    #[serde(default)]
    api_type: Api,
    /// What the inference engine should be told about how to answer.
    #[serde(default)]
    instructions: AiInstructionsConfig,
    /// Configuration of searching through what was said.
    #[serde(default)]
    search: AiSearchConfig,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            base_url: None,
            api_key: None,
            model: Self::default_model(),
            api_type: Api::default(),
            instructions: AiInstructionsConfig::default(),
            search: AiSearchConfig::default(),
        }
    }
}

impl AiConfig {
    /// Return the default model.
    fn default_model() -> String {
        DEFAULT_AI_MODEL.to_string()
    }

    /// Return the URL of the inference engine.
    pub fn base_url(&self) -> Option<&String> {
        self.base_url.as_ref()
    }

    /// Return the key which authenticates requests.
    pub fn api_key(&self) -> Option<&String> {
        self.api_key.as_ref()
    }

    /// Return the model which replies.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Return the kind of API the inference engine speaks.
    pub fn api_type(&self) -> Api {
        self.api_type
    }

    /// Return what the inference engine should be told about how to answer, which is nothing if
    /// the instructions have been overridden with nothing.
    pub fn instructions(&self) -> Option<String> {
        self.instructions.resolve()
    }

    /// Return configuration of searching through what was said.
    pub fn search(&self) -> &AiSearchConfig {
        &self.search
    }

    /// Return whether there is enough here to ask an inference engine for a reply.
    ///
    /// Anthropic will not answer without a key. Ollama and LM Studio do not ask for one, so a key
    /// is not required for them.
    pub fn configured(&self) -> bool {
        if self.base_url.is_none() {
            return false;
        }

        if matches!(self.api_type, Api::Anthropic) && self.api_key.is_none() {
            return false;
        }

        true
    }
}

/// Configuration of what the inference engine is told about how to answer.
#[derive(Deserialize, Debug, Default, Clone, Eq, PartialEq)]
pub struct AiInstructionsConfig {
    /// What to say instead of the instructions which come with inshd.
    #[serde(rename = "override", default)]
    overridden: Option<String>,
    /// What to say after the instructions.
    #[serde(default)]
    additional: Option<String>,
}

impl AiInstructionsConfig {
    /// Return what the inference engine is told about how to answer, which is nothing if the
    /// instructions have been overridden with nothing.
    ///
    /// Anything additional comes after whichever instructions are being used, so that it adds to
    /// an override as well as to the ones which come with inshd.
    fn resolve(&self) -> Option<String> {
        let instructions: &str = match &self.overridden {
            Some(overridden) => overridden,
            None => DEFAULT_AI_INSTRUCTIONS,
        }
        .trim();

        let additional: &str = match &self.additional {
            Some(additional) => additional,
            None => return Some(instructions.to_string()).filter(|text| !text.is_empty()),
        }
        .trim();

        let instructions: String = match (instructions.is_empty(), additional.is_empty()) {
            (true, true) => return None,
            (true, false) => additional.to_string(),
            (false, true) => instructions.to_string(),
            (false, false) => format!("{}\n\n{}", instructions, additional),
        };

        Some(instructions)
    }
}

/// Configuration of searching through what was said.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct AiSearchConfig {
    /// How far apart the vectors of two texts can be before a search stops calling them a match.
    #[serde(default = "AiSearchConfig::default_threshold")]
    threshold: f64,
}

impl Default for AiSearchConfig {
    fn default() -> Self {
        Self {
            threshold: Self::default_threshold(),
        }
    }
}

impl AiSearchConfig {
    /// Return the default threshold.
    fn default_threshold() -> f64 {
        DEFAULT_AI_SEARCH_THRESHOLD
    }

    /// Return how far apart the vectors of two texts can be before a search stops calling them a
    /// match.
    pub fn threshold(&self) -> f64 {
        self.threshold
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
                    // The message from the parser quotes the value it could not make sense of, and
                    // the AI API key is one of the values in this file, so only where the problem
                    // is gets reported. Otherwise a typo on the wrong line writes the key into the
                    // logs.
                    match error.location() {
                        Some(location) => write!(
                            formatter,
                            "Failed to parse the configuration file {:?} at line {} column {}",
                            path,
                            location.line(),
                            location.column()
                        ),
                        None => {
                            write!(
                                formatter,
                                "Failed to parse the configuration file {:?}",
                                path
                            )
                        }
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    /// A value which stands in for an API key.
    const SECRET: &str = "sk-ant-api03-NOT-A-REAL-KEY";

    /// Saying where the inference engine is and how to get in has to be enough on its own.
    ///
    /// Everything else about the AI has a default, so a configuration which sets only these two
    /// must parse and leave the rest alone.
    #[test]
    fn test_only_a_base_url_and_a_key_are_needed() {
        let yaml: String = format!(
            "ai:\n  base_url: https://api.anthropic.com\n  api_key: {}\n",
            SECRET
        );

        let config: Config = serde_yaml_ng::from_str(&yaml).unwrap();

        assert!(config.ai().configured());
        assert_eq!(config.ai().model(), DEFAULT_AI_MODEL);
        assert_eq!(config.ai().api_type(), Api::Anthropic);
        assert_eq!(
            config.ai().instructions(),
            Some(DEFAULT_AI_INSTRUCTIONS.trim().to_string())
        );
        assert_eq!(
            config.ai().search().threshold(),
            DEFAULT_AI_SEARCH_THRESHOLD
        );
    }

    /// The message for a configuration which cannot be parsed must not quote what is in the file.
    ///
    /// The API key lives in this file, and a mistake anywhere in it used to put the offending
    /// value into the daemon logs in the clear.
    #[test]
    fn test_parse_error_does_not_leak_values() {
        let yaml: String = format!(
            "ai:\n  base_url: https://example.com\n  api_type: {}\n",
            SECRET
        );

        let error = match serde_yaml_ng::from_str::<Config>(&yaml) {
            Ok(_) => panic!("The configuration should not have parsed."),
            Err(error) => error,
        };
        let message: String = LoadError::ParseFailed {
            path: PathBuf::from("/home/someone/.insh/inshd-config.yaml"),
            error,
        }
        .to_string();

        assert!(
            !message.contains(SECRET),
            "The message gave away a value from the file: {}",
            message
        );
        // It still has to say where to look.
        assert!(
            message.contains("line"),
            "The message is unhelpful: {}",
            message
        );
    }

    /// The instructions which come with inshd are used when nothing replaces them.
    #[test]
    fn test_instructions_default_to_the_ones_which_come_with_inshd() {
        let config: AiInstructionsConfig = AiInstructionsConfig::default();

        assert_eq!(
            config.resolve(),
            Some(DEFAULT_AI_INSTRUCTIONS.trim().to_string())
        );
    }

    /// An override is said instead of the instructions which come with inshd.
    #[test]
    fn test_an_override_replaces_the_instructions() {
        let config: AiInstructionsConfig = AiInstructionsConfig {
            overridden: Some("Answer in French.".to_string()),
            additional: None,
        };

        assert_eq!(config.resolve(), Some("Answer in French.".to_string()));
    }

    /// Anything additional comes after whichever instructions are being used.
    #[test_case(None, DEFAULT_AI_INSTRUCTIONS ; "the ones which come with inshd")]
    #[test_case(Some("Be terse."), "Be terse." ; "an override")]
    fn test_anything_additional_comes_after(overridden: Option<&str>, before: &str) {
        let config: AiInstructionsConfig = AiInstructionsConfig {
            overridden: overridden.map(str::to_string),
            additional: Some("Answer in French.".to_string()),
        };

        let instructions: String = config.resolve().unwrap();

        assert_eq!(
            instructions,
            format!("{}\n\nAnswer in French.", before.trim())
        );
    }

    /// Overriding the instructions with nothing leaves the inference engine without any.
    #[test]
    fn test_overriding_with_nothing_says_nothing() {
        let config: AiInstructionsConfig = AiInstructionsConfig {
            overridden: Some(String::new()),
            additional: None,
        };

        assert_eq!(config.resolve(), None);
    }
}
