//! Works out which part of some code is what.

use std::collections::HashMap;

use super::document::Document;
use super::language::Language;
use super::span::Span;

/// Works out which part of some code is what.
///
/// Reading a grammar takes long enough to be worth doing once, so this keeps a [`Document`] for
/// each language it has been asked about and is meant to be kept around rather than made for every
/// piece of code.
#[derive(Default)]
pub struct Highlighter {
    /// The documents which code is parsed in, by the language they are for.
    ///
    /// A language whose grammar could not be read is remembered as nothing, so that it is not tried
    /// again for every piece of code.
    documents: HashMap<Language, Option<Document>>,
}

impl Highlighter {
    /// Return a new highlighter.
    pub fn new() -> Self {
        return Self::default();
    }

    /// Return the pieces of each line of some code along with what each one is.
    ///
    /// The code is parsed as a whole, so a line which is inside something started on a line above
    /// it, like a comment, is shown as being in it. Code in a language whose grammar cannot be
    /// read comes back as one piece of nothing in particular per line rather than as an error, since
    /// showing it plainly is better than not showing it.
    pub fn highlight(&mut self, code: &str, language: Language) -> Vec<Vec<Span>> {
        let lines: Vec<String> = code.split('\n').map(String::from).collect();

        let document: &mut Document = match self.document(language) {
            Some(document) => document,
            None => return lines.iter().map(|line| Self::plain(line)).collect(),
        };

        return document
            .highlight(&lines, 0..lines.len())
            .iter()
            .zip(lines.iter())
            .map(|(runs, line)| Span::all(line, runs))
            .collect();
    }

    /// Return the document for the language, ready for some other code to be parsed in it, which
    /// is nothing when its grammar cannot be read.
    ///
    /// This is for highlighting only some of the lines of some code, which
    /// [`Document::highlight`] can be asked for as many times as is wanted.
    pub fn document(&mut self, language: Language) -> Option<&mut Document> {
        let document: &mut Option<Document> = self.documents.entry(language).or_insert_with(|| {
            return Document::parse(language, &[]);
        });

        let document: &mut Document = document.as_mut()?;
        document.rewrite();
        return Some(document);
    }

    /// Return the line as one piece of nothing in particular.
    fn plain(line: &str) -> Vec<Span> {
        if line.is_empty() {
            return Vec::new();
        }

        return vec![Span {
            text: line.to_string(),
            kind: None,
        }];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Kind;

    use test_case::test_case;

    /// Return what the pieces of some code are, joined back up.
    fn text(lines: &[Vec<Span>]) -> String {
        return lines
            .iter()
            .map(|spans| {
                spans
                    .iter()
                    .map(|span| span.text.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");
    }

    /// Return whether any piece of the code is the given thing.
    fn has(lines: &[Vec<Span>], kind: Kind) -> bool {
        return lines.iter().flatten().any(|span| span.kind == Some(kind));
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
        let spans: Vec<Vec<Span>> = Highlighter::new().highlight(code, language);

        // Whatever it makes of the code, none of it may go missing.
        assert_eq!(text(&spans), code);
    }

    #[test]
    fn test_rust_is_taken_apart() {
        let code: &str = "// hi\nfn main() {\n    let x = 1;\n}";

        let spans: Vec<Vec<Span>> = Highlighter::new().highlight(code, Language::Rust);

        assert_eq!(text(&spans), code);
        assert!(has(&spans, Kind::Comment), "the comment was not found");
        assert!(has(&spans, Kind::Keyword), "the keywords were not found");
        // The Rust grammar calls a literal a constant rather than a number.
        assert!(has(&spans, Kind::Constant), "the literal was not found");
    }

    #[test]
    fn test_a_number_is_found() {
        let spans: Vec<Vec<Span>> = Highlighter::new().highlight(r#"{"a": 12}"#, Language::Json);

        assert!(has(&spans, Kind::Number), "the number was not found");
    }

    #[test]
    fn test_a_string_is_found() {
        let spans: Vec<Vec<Span>> =
            Highlighter::new().highlight(r#"let a = "hi";"#, Language::Rust);

        assert!(has(&spans, Kind::String), "the string was not found");
    }

    #[test]
    fn test_code_which_does_not_parse_is_still_all_there() {
        let code: &str = "fn fn fn ((( ";

        let spans: Vec<Vec<Span>> = Highlighter::new().highlight(code, Language::Rust);

        assert_eq!(text(&spans), code);
    }

    #[test]
    fn test_nothing_gives_one_line_of_nothing() {
        assert_eq!(
            Highlighter::new().highlight("", Language::Rust),
            vec![Vec::<Span>::new()]
        );
    }

    #[test]
    fn test_a_line_inside_a_comment_started_above_it_is_a_comment() {
        let spans: Vec<Vec<Span>> = Highlighter::new().highlight("/*\nfn\n*/", Language::Rust);

        assert_eq!(spans[1][0].kind, Some(Kind::Comment));
    }

    #[test]
    fn test_one_piece_of_code_does_not_change_the_next() {
        let mut highlighter: Highlighter = Highlighter::new();
        highlighter.highlight("/*", Language::Rust);

        let spans: Vec<Vec<Span>> = highlighter.highlight("fn", Language::Rust);

        assert_eq!(spans[0][0].kind, Some(Kind::Keyword));
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
