//! Works out which part of some code is what.

use std::collections::HashMap;

use super::language::Language;
use super::span::{Kind, Span};

use tree_sitter_highlight::{
    HighlightConfiguration, HighlightEvent, Highlighter as TreeSitterHighlighter,
};

/// Works out which part of some code is what.
///
/// Reading a grammar takes long enough to be worth doing once, so this keeps the ones it has read
/// and is meant to be kept around rather than made for every piece of code.
#[derive(Default)]
pub struct Highlighter {
    /// What does the highlighting.
    highlighter: TreeSitterHighlighter,
    /// The grammars which have been read, by the language they are for.
    ///
    /// A language whose grammar could not be read is remembered as nothing, so that it is not tried
    /// again for every code block.
    configurations: HashMap<Language, Option<HighlightConfiguration>>,
}

impl Highlighter {
    /// Return a new highlighter.
    pub fn new() -> Self {
        return Self::default();
    }

    /// Return the pieces of some code along with what each one is.
    ///
    /// Code in a language which is not known, or which the grammar cannot make sense of, comes back
    /// as one piece of nothing in particular rather than as an error. Showing it plainly is better
    /// than not showing it.
    pub fn highlight(&mut self, code: &str, language: Language) -> Vec<Span> {
        let configuration = self
            .configurations
            .entry(language)
            .or_insert_with(|| language.configuration());

        let configuration: &HighlightConfiguration = match configuration {
            Some(configuration) => configuration,
            None => return Self::plain(code),
        };

        let events =
            match self
                .highlighter
                .highlight(configuration, code.as_bytes(), None, None, |_| None)
            {
                Ok(events) => events,
                Err(_) => return Self::plain(code),
            };

        let mut spans: Vec<Span> = Vec::new();
        // What a piece is is whatever was started most recently, since they nest.
        let mut kinds: Vec<Option<Kind>> = Vec::new();

        for event in events {
            match event {
                Ok(HighlightEvent::HighlightStart(highlight)) => {
                    kinds.push(Kind::of_index(highlight.0));
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    kinds.pop();
                }
                Ok(HighlightEvent::Source { start, end }) => {
                    let text: &str = match code.get(start..end) {
                        Some(text) => text,
                        None => continue,
                    };
                    if text.is_empty() {
                        continue;
                    }

                    // The innermost one which says anything is the one which is meant: a name
                    // inside a function call is the name, not the call.
                    let kind: Option<Kind> = kinds.iter().rev().find_map(|kind| *kind);

                    // Runs of the same thing are joined rather than left as one piece per token.
                    match spans.last_mut() {
                        Some(last) if last.kind == kind => last.text.push_str(text),
                        _ => spans.push(Span {
                            text: text.to_string(),
                            kind,
                        }),
                    }
                }
                Err(_) => return Self::plain(code),
            }
        }

        return spans;
    }

    /// Return the code as one piece of nothing in particular.
    fn plain(code: &str) -> Vec<Span> {
        if code.is_empty() {
            return Vec::new();
        }

        return vec![Span {
            text: code.to_string(),
            kind: None,
        }];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    /// Return what the pieces of some code are, joined back up.
    fn text(spans: &[Span]) -> String {
        return spans.iter().map(|span| span.text.as_str()).collect();
    }

    /// Return whether any piece of the code is the given thing.
    fn has(spans: &[Span], kind: Kind) -> bool {
        return spans.iter().any(|span| span.kind == Some(kind));
    }

    #[test_case(Language::Rust, "fn main() {}"; "rust")]
    #[test_case(Language::Python, "def main():\n    pass"; "python")]
    #[test_case(Language::Bash, "echo hi"; "bash")]
    #[test_case(Language::Json, r#"{"a": 1}"#; "json")]
    #[test_case(Language::Go, "package main"; "go")]
    #[test_case(Language::C, "int main(void) { return 0; }"; "c")]
    #[test_case(Language::JavaScript, "const x = 1;"; "javascript")]
    #[test_case(Language::Sql, "SELECT 1;"; "sql")]
    #[test_case(Language::Toml, "a = 1"; "toml")]
    #[test_case(Language::Yaml, "a: 1"; "yaml")]
    fn test_every_language_keeps_the_code_it_was_given(language: Language, code: &str) {
        let spans: Vec<Span> = Highlighter::new().highlight(code, language);

        // Whatever it makes of the code, none of it may go missing.
        assert_eq!(text(&spans), code);
    }

    #[test]
    fn test_rust_is_taken_apart() {
        let code: &str = "// hi\nfn main() {\n    let x = 1;\n}";

        let spans: Vec<Span> = Highlighter::new().highlight(code, Language::Rust);

        assert_eq!(text(&spans), code);
        assert!(has(&spans, Kind::Comment), "the comment was not found");
        assert!(has(&spans, Kind::Keyword), "the keywords were not found");
        // The Rust grammar calls a literal a constant rather than a number.
        assert!(has(&spans, Kind::Constant), "the literal was not found");
    }

    #[test]
    fn test_a_number_is_found() {
        let spans: Vec<Span> = Highlighter::new().highlight(r#"{"a": 12}"#, Language::Json);

        assert!(has(&spans, Kind::Number), "the number was not found");
    }

    #[test]
    fn test_a_string_is_found() {
        let spans: Vec<Span> = Highlighter::new().highlight(r#"let a = "hi";"#, Language::Rust);

        assert!(has(&spans, Kind::String), "the string was not found");
    }

    #[test]
    fn test_code_which_does_not_parse_is_still_all_there() {
        let code: &str = "fn fn fn ((( ";

        let spans: Vec<Span> = Highlighter::new().highlight(code, Language::Rust);

        assert_eq!(text(&spans), code);
    }

    #[test]
    fn test_nothing_gives_nothing() {
        assert!(Highlighter::new().highlight("", Language::Rust).is_empty());
    }

    #[test_case("rs", Some(Language::Rust); "an extension")]
    #[test_case(".rs", Some(Language::Rust); "an extension with a dot")]
    #[test_case("PY", Some(Language::Python); "an extension in capitals")]
    #[test_case("xyz", None; "an extension which is not known")]
    fn test_the_language_of_a_file(extension: &str, expected: Option<Language>) {
        assert_eq!(Language::of_extension(extension), expected);
    }

    #[test_case("rust", Some(Language::Rust); "a name")]
    #[test_case("sh", Some(Language::Bash); "a short name")]
    #[test_case("Shell", Some(Language::Bash); "a name in capitals")]
    #[test_case("brainfuck", None; "a name which is not known")]
    fn test_the_language_of_a_code_block(name: &str, expected: Option<Language>) {
        assert_eq!(Language::of_name(name), expected);
    }
}
