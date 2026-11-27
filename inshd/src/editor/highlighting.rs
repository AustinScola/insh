//! What each piece of a file being edited is.

use std::ops::Range;

use highlighter::{Document, Language, Run};
use insh_api::Position;

use typed_builder::TypedBuilder;

/// What each piece of a file being edited is.
///
/// The whole file is kept parsed rather than only the lines shown, since what a line is can depend
/// on lines far above it, such as those a comment or a string started on. It is moved along with
/// every change rather than parsed again, so a key pressed costs about as much as what it changed
/// and the lines shown, however long the file is.
#[derive(TypedBuilder)]
pub struct Highlighting {
    /// The language the file is written in, which is nothing for one which is not known.
    language: Option<Language>,
    /// The file, parsed, which is nothing until it is first highlighted.
    #[builder(setter(skip), default)]
    document: Option<Document>,
}

impl Highlighting {
    /// Take note of the text which was removed being replaced by the text which was inserted at
    /// the position, given the lines as they are afterwards.
    pub fn edit(&mut self, lines: &[String], at: Position, removed: &str, inserted: &str) {
        if let Some(document) = &mut self.document {
            document.edit(lines, at.line, at.column, removed, inserted);
        }
    }

    /// Return the runs of the lines in the range, which is nothing for a file in a language which
    /// is not known.
    pub fn runs(&mut self, lines: &[String], range: Range<usize>) -> Vec<Vec<Run>> {
        let language: Language = match self.language {
            Some(language) => language,
            None => return Vec::new(),
        };

        if self.document.is_none() {
            self.document = Document::parse(language, lines);
            // NOTE: A grammar which could not be read once will not be read the next time either.
            if self.document.is_none() {
                self.language = None;
            }
        }
        let document: &mut Document = match &mut self.document {
            Some(document) => document,
            None => return Vec::new(),
        };

        return document.highlight(lines, range);
    }
}

#[cfg(test)]
mod tests {
    use super::Highlighting;

    use highlighter::{Kind, Language, Run};
    use insh_api::Position;

    /// Return the lines of some text.
    fn lines(text: &str) -> Vec<String> {
        return text.split('\n').map(String::from).collect();
    }

    #[test]
    fn test_there_are_runs_for_every_line_and_every_character() {
        let text: Vec<String> = lines("fn main() {\n\n    let x = \"hi\";\n}");
        let mut highlighting: Highlighting = Highlighting::builder()
            .language(Some(Language::Rust))
            .build();

        let runs: Vec<Vec<Run>> = highlighting.runs(&text, 0..text.len());

        assert_eq!(runs.len(), text.len());
        for (line, runs) in text.iter().zip(runs.iter()) {
            let chars: usize = runs.iter().map(|run| run.chars).sum();
            assert_eq!(chars, line.chars().count());
        }
    }

    #[test]
    fn test_an_edit_is_highlighted() {
        let mut highlighting: Highlighting = Highlighting::builder()
            .language(Some(Language::Rust))
            .build();
        highlighting.runs(&lines("x"), 0..1);

        highlighting.edit(&lines("// x"), Position::new(0, 0), "", "// ");
        let runs: Vec<Vec<Run>> = highlighting.runs(&lines("// x"), 0..1);

        assert_eq!(
            runs,
            vec![vec![Run {
                chars: 4,
                kind: Some(Kind::Comment),
            }]]
        );
    }

    #[test]
    fn test_a_language_which_is_not_known_has_no_runs() {
        let mut highlighting: Highlighting = Highlighting::builder().language(None).build();

        assert!(highlighting.runs(&lines("fn"), 0..1).is_empty());
    }
}
