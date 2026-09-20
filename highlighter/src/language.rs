//! A language which code can be highlighted in.

use tree_sitter_highlight::HighlightConfiguration;

use super::span::NAMES;

/// A language which code can be highlighted in.
///
/// Adding one is a variant here, the three lines which name it in [`Language::of_name`] and
/// [`Language::of_extension`], and its grammar in [`Language::configuration`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Language {
    /// Bash, and close enough for other shells.
    Bash,
    /// C.
    C,
    /// Go.
    Go,
    /// JavaScript.
    JavaScript,
    /// JSON.
    Json,
    /// Python.
    Python,
    /// Rust.
    Rust,
    /// SQL.
    Sql,
    /// TOML.
    Toml,
    /// YAML.
    Yaml,
}

impl Language {
    /// Return the language which goes by a name, as a fenced code block in markdown gives it.
    pub fn of_name(name: &str) -> Option<Self> {
        // The name is whatever was typed, so it is matched without regard to case and the usual
        // short forms are allowed.
        return match name.trim().to_lowercase().as_str() {
            "bash" | "sh" | "shell" | "zsh" | "console" => Some(Self::Bash),
            "c" | "h" => Some(Self::C),
            "go" | "golang" => Some(Self::Go),
            "javascript" | "js" | "jsx" | "node" => Some(Self::JavaScript),
            "json" | "jsonc" => Some(Self::Json),
            "python" | "py" => Some(Self::Python),
            "rust" | "rs" => Some(Self::Rust),
            "sql" | "postgres" | "postgresql" | "psql" => Some(Self::Sql),
            "toml" => Some(Self::Toml),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        };
    }

    /// Return the language which a file with the given extension is written in.
    ///
    /// This is here for showing a file rather than a chat, which is the other thing worth
    /// highlighting.
    pub fn of_extension(extension: &str) -> Option<Self> {
        return match extension.trim_start_matches('.').to_lowercase().as_str() {
            "bash" | "sh" | "zsh" => Some(Self::Bash),
            "c" | "h" => Some(Self::C),
            "go" => Some(Self::Go),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "json" => Some(Self::Json),
            "py" | "pyi" => Some(Self::Python),
            "rs" => Some(Self::Rust),
            "sql" => Some(Self::Sql),
            "toml" => Some(Self::Toml),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        };
    }

    /// Return what the language is called.
    pub fn name(&self) -> &'static str {
        return match self {
            Self::Bash => "bash",
            Self::C => "c",
            Self::Go => "go",
            Self::JavaScript => "javascript",
            Self::Json => "json",
            Self::Python => "python",
            Self::Rust => "rust",
            Self::Sql => "sql",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
        };
    }

    /// Return the grammar for the language, ready to be highlighted with.
    ///
    /// The grammars do not all carry the same queries, so the ones which are not there are asked
    /// for as nothing rather than left out.
    pub(crate) fn configuration(&self) -> Option<HighlightConfiguration> {
        let (language, highlights, injections, locals) = match self {
            Self::Bash => (
                tree_sitter_bash::LANGUAGE.into(),
                tree_sitter_bash::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Self::C => (
                tree_sitter_c::LANGUAGE.into(),
                tree_sitter_c::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Self::Go => (
                tree_sitter_go::LANGUAGE.into(),
                tree_sitter_go::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Self::JavaScript => (
                tree_sitter_javascript::LANGUAGE.into(),
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                tree_sitter_javascript::INJECTIONS_QUERY,
                tree_sitter_javascript::LOCALS_QUERY,
            ),
            Self::Json => (
                tree_sitter_json::LANGUAGE.into(),
                tree_sitter_json::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Self::Python => (
                tree_sitter_python::LANGUAGE.into(),
                tree_sitter_python::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Self::Rust => (
                tree_sitter_rust::LANGUAGE.into(),
                tree_sitter_rust::HIGHLIGHTS_QUERY,
                tree_sitter_rust::INJECTIONS_QUERY,
                "",
            ),
            Self::Sql => (
                tree_sitter_sequel::LANGUAGE.into(),
                tree_sitter_sequel::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Self::Toml => (
                tree_sitter_toml_ng::LANGUAGE.into(),
                tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Self::Yaml => (
                tree_sitter_yaml::LANGUAGE.into(),
                tree_sitter_yaml::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
        };

        let mut configuration =
            HighlightConfiguration::new(language, self.name(), highlights, injections, locals)
                .ok()?;
        configuration.configure(&NAMES);

        return Some(configuration);
    }
}
