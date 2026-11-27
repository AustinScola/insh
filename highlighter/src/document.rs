//! Code which is kept parsed as it changes.

use std::ops::Range;

use super::language::Language;
use super::span::{Kind, Run};

use tree_sitter::{
    InputEdit, Language as TreeSitterLanguage, Node, Parser, Point, Query, QueryCursor,
    StreamingIterator, Tree,
};

/// Code which is kept parsed as it changes, so that any of its lines can be highlighted without all
/// of it being parsed again.
///
/// The lines are given each time rather than kept here, since whoever is changing them keeps them
/// already. Every change has to be told to [`Document::edit`] as it is made, so that the tree can
/// be moved along with the text; only what changed is parsed again, and not until it is next
/// highlighted.
pub struct Document {
    /// What does the parsing.
    parser: Parser,
    /// What picks out each piece of the code.
    query: Query,
    /// What each name the query captures is, by its position.
    kinds: Vec<Option<Kind>>,
    /// The code, parsed, which is nothing when it could not be.
    tree: Option<Tree>,
    /// Whether the code has changed since it was parsed.
    stale: bool,
}

/// One thing the query picked out of the code.
struct Capture {
    /// Where it starts.
    start: Point,
    /// Where it ends.
    end: Point,
    /// How many nodes of the tree it is inside.
    depth: usize,
    /// Which pattern of the query picked it out.
    pattern: usize,
    /// What it is.
    kind: Kind,
}

impl Document {
    /// Return the lines parsed in the language, which is nothing when its grammar cannot be read.
    pub fn parse(language: Language, lines: &[String]) -> Option<Self> {
        let (grammar, highlights): (TreeSitterLanguage, &str) = language.grammar();

        let mut parser: Parser = Parser::new();
        parser.set_language(&grammar).ok()?;
        let query: Query = Query::new(&grammar, highlights).ok()?;
        let kinds: Vec<Option<Kind>> = query
            .capture_names()
            .iter()
            .map(|name| Kind::of_capture(name))
            .collect();

        let mut document: Self = Self {
            parser,
            query,
            kinds,
            tree: None,
            stale: true,
        };
        document.reparse(lines);

        return Some(document);
    }

    /// Take note that the lines are not the ones which were parsed at all, but some other code
    /// in the same language.
    pub(crate) fn rewrite(&mut self) {
        self.tree = None;
        self.stale = true;
    }

    /// Take note of the text which was removed being replaced by the text which was inserted, at
    /// the character of the line, given the lines as they are afterwards.
    pub fn edit(
        &mut self,
        lines: &[String],
        line: usize,
        column: usize,
        removed: &str,
        inserted: &str,
    ) {
        self.stale = true;

        let text: &str = match lines.get(line) {
            Some(text) => text,
            None => {
                // NOTE: A tree moved along wrongly would be wrong from then on, so the whole of
                // the code is parsed again instead.
                self.tree = None;
                return;
            }
        };
        let tree: &mut Tree = match &mut self.tree {
            Some(tree) => tree,
            None => return,
        };

        let byte: usize = match text.char_indices().nth(column) {
            Some((byte, _)) => byte,
            None => text.len(),
        };
        let start_byte: usize = lines[..line]
            .iter()
            .map(|line| line.len() + 1)
            .sum::<usize>()
            + byte;
        let start_position: Point = Point::new(line, byte);
        let (old_end_byte, old_end_position): (usize, Point) =
            Self::after(start_byte, start_position, removed);
        let (new_end_byte, new_end_position): (usize, Point) =
            Self::after(start_byte, start_position, inserted);

        tree.edit(&InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position,
            old_end_position,
            new_end_position,
        });
    }

    /// Return the runs of each of the lines in the range.
    pub fn highlight(&mut self, lines: &[String], range: Range<usize>) -> Vec<Vec<Run>> {
        if self.stale {
            self.reparse(lines);
        }

        let range: Range<usize> = range.start.min(lines.len())..range.end.min(lines.len());
        if range.is_empty() {
            return Vec::new();
        }

        // What each byte of each of the lines is.
        let mut kinds: Vec<Vec<Option<Kind>>> = lines[range.clone()]
            .iter()
            .map(|line| vec![None; line.len()])
            .collect();

        for capture in self.captures(lines, range.clone()) {
            let rows = capture.start.row.max(range.start)..=capture.end.row.min(range.end - 1);
            for row in rows {
                let bytes: &mut Vec<Option<Kind>> = &mut kinds[row - range.start];
                let from: usize = match row == capture.start.row {
                    true => capture.start.column.min(bytes.len()),
                    false => 0,
                };
                let to: usize = match row == capture.end.row {
                    true => capture.end.column.min(bytes.len()),
                    false => bytes.len(),
                };
                for kind in bytes.iter_mut().take(to).skip(from) {
                    *kind = Some(capture.kind);
                }
            }
        }

        return lines[range]
            .iter()
            .zip(kinds.iter())
            .map(|(line, kinds)| Self::runs(line, kinds))
            .collect();
    }

    /// Return what the query picks out of the lines in the range, in the order they are to be
    /// colored in.
    ///
    /// Something inside something else is colored after it, so that it is what shows. Where more
    /// than one thing takes up exactly the same text, only one is colored, which is the way
    /// tree-sitter's own highlighting picks it: the outermost node, by the last pattern which
    /// picked it out.
    fn captures(&self, lines: &[String], range: Range<usize>) -> Vec<Capture> {
        let tree: &Tree = match &self.tree {
            Some(tree) => tree,
            None => return Vec::new(),
        };

        let mut captures: Vec<Capture> = Vec::new();
        let mut cursor: QueryCursor = QueryCursor::new();
        cursor.set_point_range(Point::new(range.start, 0)..Point::new(range.end, 0));
        let mut matches = cursor.matches(&self.query, tree.root_node(), |node: Node| {
            Self::chunks(lines, node)
        });
        while let Some(found) = matches.next() {
            for capture in found.captures() {
                if let Some(Some(kind)) = self.kinds.get(capture.index as usize) {
                    captures.push(Capture {
                        start: capture.node.start_position(),
                        end: capture.node.end_position(),
                        depth: Self::depth(capture.node),
                        pattern: found.pattern_index,
                        kind: *kind,
                    });
                }
            }
        }

        captures.sort_by(|a, b| {
            a.start
                .cmp(&b.start)
                .then(b.end.cmp(&a.end))
                .then(a.depth.cmp(&b.depth))
                .then(b.pattern.cmp(&a.pattern))
        });
        captures.dedup_by(|later, first| later.start == first.start && later.end == first.end);

        return captures;
    }

    /// Return how many nodes of the tree the node is inside.
    fn depth(node: Node) -> usize {
        let mut depth: usize = 0;
        let mut node: Node = node;
        while let Some(parent) = node.parent() {
            depth += 1;
            node = parent;
        }
        return depth;
    }

    /// Parse the lines again, which is only what changed when the tree has been moved along with
    /// the changes.
    fn reparse(&mut self, lines: &[String]) {
        let mut read = |_byte: usize, point: Point| Self::read(lines, point);
        self.tree = self
            .parser
            .parse_with_options(&mut read, self.tree.as_ref(), None);
        self.stale = false;
    }

    /// Return the text from the point up to the end of its line, or the newline at the end of the
    /// line, or nothing at all at the end of the lines.
    fn read(lines: &[String], point: Point) -> &[u8] {
        return match lines.get(point.row) {
            Some(line) if point.column < line.len() => &line.as_bytes()[point.column..],
            Some(_) if point.row + 1 < lines.len() => b"\n",
            _ => &[],
        };
    }

    /// Return the text of the node, in pieces.
    fn chunks<'a>(lines: &'a [String], node: Node) -> std::vec::IntoIter<&'a [u8]> {
        let (start, end): (Point, Point) = (node.start_position(), node.end_position());

        let mut chunks: Vec<&[u8]> = Vec::new();
        let rows = lines.iter().enumerate().take(end.row + 1).skip(start.row);
        for (row, line) in rows {
            let line: &[u8] = line.as_bytes();
            let from: usize = match row == start.row {
                true => start.column.min(line.len()),
                false => 0,
            };
            let to: usize = match row == end.row {
                true => end.column.min(line.len()),
                false => line.len(),
            };
            if row > start.row {
                chunks.push(b"\n");
            }
            chunks.push(&line[from..to.max(from)]);
        }

        return chunks.into_iter();
    }

    /// Return the byte and the point which the text ends at, put in at the byte and the point.
    fn after(byte: usize, point: Point, text: &str) -> (usize, Point) {
        let point: Point = match text.rfind('\n') {
            Some(newline) => Point::new(
                point.row + text.matches('\n').count(),
                text.len() - newline - 1,
            ),
            None => Point::new(point.row, point.column + text.len()),
        };
        return (byte + text.len(), point);
    }

    /// Return the runs of the line, given what each of its bytes is.
    fn runs(line: &str, kinds: &[Option<Kind>]) -> Vec<Run> {
        let mut runs: Vec<Run> = Vec::new();
        for (byte, _) in line.char_indices() {
            let kind: Option<Kind> = kinds[byte];
            match runs.last_mut() {
                Some(last) if last.kind == kind => last.chars += 1,
                _ => runs.push(Run { chars: 1, kind }),
            }
        }
        return runs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;

    use test_case::test_case;

    /// Return the lines of some text.
    fn lines(text: &str) -> Vec<String> {
        return text.split('\n').map(String::from).collect();
    }

    /// Return the text with the removed text replaced by the inserted text at the character of the
    /// line.
    fn replaced(text: &str, line: usize, column: usize, removed: &str, inserted: &str) -> String {
        let start: usize = text.split('\n').take(line).map(|line| line.len() + 1).sum();
        let line: &str = text.split('\n').nth(line).unwrap();
        let byte: usize = start
            + line
                .char_indices()
                .nth(column)
                .map_or(line.len(), |(byte, _)| byte);
        assert_eq!(&text[byte..byte + removed.len()], removed);
        return format!(
            "{}{}{}",
            &text[..byte],
            inserted,
            &text[byte + removed.len()..]
        );
    }

    /// Return the pieces of each of the lines in the range.
    fn highlighted(
        document: &mut Document,
        text: &[String],
        range: Range<usize>,
    ) -> Vec<Vec<Span>> {
        return document
            .highlight(text, range.clone())
            .iter()
            .zip(text[range].iter())
            .map(|(runs, line)| Span::all(line, runs))
            .collect();
    }

    /// Return the kind of the piece of the line which the text is.
    fn kind_of(spans: &[Vec<Span>], line: usize, text: &str) -> Option<Kind> {
        return spans[line]
            .iter()
            .find(|span| span.text.contains(text))
            .unwrap_or_else(|| panic!("{:?} is not on line {}", text, line))
            .kind;
    }

    #[test]
    fn test_every_character_of_every_line_is_kept() {
        let text: Vec<String> = lines("fn main() {\n\n    let é = \"hi\";\n}");
        let mut document: Document = Document::parse(Language::Rust, &text).unwrap();

        let spans: Vec<Vec<Span>> = highlighted(&mut document, &text, 0..text.len());

        let kept: Vec<String> = spans
            .iter()
            .map(|spans| spans.iter().map(|span| span.text.as_str()).collect())
            .collect();
        assert_eq!(kept, text);
    }

    #[test]
    fn test_a_comment_started_above_the_lines_asked_for_is_still_a_comment() {
        let text: Vec<String> = lines("/*\nhidden\n*/\nfn");
        let mut document: Document = Document::parse(Language::Rust, &text).unwrap();

        let spans: Vec<Vec<Span>> = highlighted(&mut document, &text, 1..2);

        assert_eq!(
            spans,
            vec![vec![Span {
                text: String::from("hidden"),
                kind: Some(Kind::Comment),
            }]]
        );
    }

    #[test]
    fn test_the_innermost_piece_is_what_shows() {
        let text: Vec<String> = lines("let a = \"x\\ny\";");
        let mut document: Document = Document::parse(Language::Rust, &text).unwrap();

        let spans: Vec<Vec<Span>> = highlighted(&mut document, &text, 0..1);

        assert_eq!(kind_of(&spans, 0, "let"), Some(Kind::Keyword));
        assert_eq!(kind_of(&spans, 0, "x"), Some(Kind::String));
    }

    #[test_case("a.b();", "b", Some(Kind::Function); "a method which is called")]
    #[test_case("a.b;", "b", Some(Kind::Variable); "a field")]
    #[test_case("Ok(a);", "Ok", Some(Kind::Function); "a function named in capitals")]
    #[test_case("let a: Ok;", "Ok", Some(Kind::Type); "a type")]
    fn test_what_is_picked_out_by_more_than_one_pattern_is_what_tree_sitter_would_say(
        code: &str,
        text: &str,
        expected: Option<Kind>,
    ) {
        let code: Vec<String> = lines(code);
        let mut document: Document = Document::parse(Language::Rust, &code).unwrap();

        let spans: Vec<Vec<Span>> = highlighted(&mut document, &code, 0..1);

        assert_eq!(kind_of(&spans, 0, text), expected);
    }

    #[test_case(0, 0, "", "// "; "commenting a line out")]
    #[test_case(1, 4, "", "/* "; "starting a comment which runs to the end")]
    #[test_case(0, 0, "fn a() {}\n", ""; "removing a whole line")]
    #[test_case(1, 3, "b", "bb\n\n"; "replacing a character with lines")]
    #[test_case(2, 12, "\"é\"", "1"; "replacing a string with a number")]
    #[test_case(1, 0, "fn b() {\n    let", "x"; "removing across lines")]
    #[test_case(3, 1, "", "\nfn c() {}"; "adding a line at the end")]
    fn test_an_edit_is_highlighted_the_way_the_text_is_when_parsed_afresh(
        line: usize,
        column: usize,
        removed: &str,
        inserted: &str,
    ) {
        let before: &str = "fn a() {}\nfn b() {\n    let é = \"é\";\n}";
        let after: String = replaced(before, line, column, removed, inserted);
        let (before, after): (Vec<String>, Vec<String>) = (lines(before), lines(&after));

        let mut document: Document = Document::parse(Language::Rust, &before).unwrap();
        document.edit(&after, line, column, removed, inserted);
        let edited: Vec<Vec<Run>> = document.highlight(&after, 0..after.len());

        let afresh: Vec<Vec<Run>> = Document::parse(Language::Rust, &after)
            .unwrap()
            .highlight(&after, 0..after.len());
        assert_eq!(edited, afresh);
    }

    #[test]
    fn test_edits_made_one_after_another_before_highlighting_are_all_kept() {
        let mut text: String = String::from("fn a() {}");
        let mut document: Document = Document::parse(Language::Rust, &lines(&text)).unwrap();

        for (line, column, removed, inserted) in [
            (0, 9, "", "\n"),
            (1, 0, "", "// b"),
            (0, 3, "a", "long_name"),
            (1, 0, "// ", ""),
        ] {
            text = replaced(&text, line, column, removed, inserted);
            document.edit(&lines(&text), line, column, removed, inserted);
        }
        let edited: Vec<Vec<Run>> = document.highlight(&lines(&text), 0..2);

        let afresh: Vec<Vec<Run>> = Document::parse(Language::Rust, &lines(&text))
            .unwrap()
            .highlight(&lines(&text), 0..2);
        assert_eq!(edited, afresh);
        assert_eq!(text, "fn long_name() {}\nb");
    }

    #[test]
    fn test_a_rewritten_document_is_parsed_afresh() {
        let mut document: Document = Document::parse(Language::Rust, &lines("/*")).unwrap();

        document.rewrite();
        let spans: Vec<Vec<Span>> = highlighted(&mut document, &lines("fn"), 0..1);

        assert_eq!(kind_of(&spans, 0, "fn"), Some(Kind::Keyword));
    }

    #[test]
    fn test_lines_past_the_end_are_nothing() {
        let text: Vec<String> = lines("fn");
        let mut document: Document = Document::parse(Language::Rust, &text).unwrap();

        assert!(document.highlight(&text, 5..9).is_empty());
    }

    #[test_case("function.method.call", Some(Kind::Function); "a name with more parts")]
    #[test_case("comment.documentation", Some(Kind::Comment); "a name exactly")]
    #[test_case("embedded", None; "a name which is not known")]
    fn test_what_a_captured_name_means(name: &str, expected: Option<Kind>) {
        assert_eq!(Kind::of_capture(name), expected);
    }
}
