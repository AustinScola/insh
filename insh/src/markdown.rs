/*!
Turns the markdown which an AI inference engine replies in into lines to show.

Only the parts which turn up in an answer are handled: paragraphs, headings, lists, quotes, tables,
inline code and emphasis, and fenced code blocks, which are colored by the [`highlighter`].

Bold is shown as a color rather than as bold text, because a [`Yarn`] carries the color of each
column and nothing else.
*/

/// Contains the [`Run`] struct.
mod run {
    use crate::color::Color;

    use ansi::Color as AnsiColor;
    use rend::Yarn;

    /// A piece of a line, and the color it is written in.
    #[derive(Debug, Clone)]
    pub struct Run {
        /// The text.
        pub text: String,
        /// The color it is written in, where nothing is whichever the terminal uses.
        pub color: Option<AnsiColor>,
        /// The color it is written on, where nothing is whichever the terminal uses.
        pub background: Option<AnsiColor>,
        /// Whether it is written in bold.
        pub bold: bool,
    }

    impl Run {
        /// Return a piece of a line written in whichever color the terminal uses.
        pub fn plain(text: impl Into<String>) -> Self {
            Self {
                text: text.into(),
                color: None,
                background: None,
                bold: false,
            }
        }

        /// Return a piece of a line written in the given color.
        pub fn colored(text: impl Into<String>, color: Color) -> Self {
            Self {
                text: text.into(),
                color: Some(color.into()),
                background: None,
                bold: false,
            }
        }

        /// Return a piece of a line written on the given color.
        pub fn on(mut self, background: Color) -> Self {
            self.background = Some(background.into());
            self
        }

        /// Return the pieces of a line as a yarn.
        pub fn yarn(runs: &[Run], columns: usize) -> Yarn {
            let mut yarn: Yarn = Yarn::from("");

            for run in runs {
                let mut piece: Yarn = Yarn::from(run.text.as_str());
                if let Some(color) = run.color {
                    piece.color(color);
                }
                if let Some(background) = run.background {
                    piece.background(background);
                }
                if run.bold {
                    piece.bold();
                }
                yarn = yarn.concat(piece);
            }

            yarn.resize(columns);
            yarn
        }
    }
}
use run::Run;

/// Contains the [`Markdown`] struct.
mod markdown {
    use super::Run;
    use crate::color::Color;

    use highlighter::{Highlighter, Language, Span};
    use rend::{Cell, Yarn};

    use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

    /// What is put before an item of a list.
    const BULLET: &str = "• ";

    /// What is put before a quote.
    const QUOTE: &str = "▏ ";

    /// How far an item of a list is indented for each level it is in.
    const INDENT: usize = 2;

    /// The pieces a table is drawn out of.
    ///
    /// These are the light box drawing characters, which every terminal font worth using has.
    const HORIZONTAL: &str = "─";
    /// The upright piece between and around the cells of a row.
    const VERTICAL: &str = "│";
    /// The corners, in reading order.
    const CORNERS: [&str; 4] = ["┌", "┐", "└", "┘"];
    /// The pieces where a rule meets the edge, at the top, the bottom, the left and the right.
    const EDGES: [&str; 4] = ["┬", "┴", "├", "┤"];
    /// The piece where a rule crosses the upright between two cells.
    const CROSS: &str = "┼";
    /// How many blanks pad each side of what is in a cell.
    const PADDING: usize = 1;

    /// Turns markdown into lines to show.
    ///
    /// This keeps the highlighter, which is worth keeping because reading a grammar is slow.
    #[derive(Default)]
    pub struct Markdown {
        /// Works out which part of some code is what.
        highlighter: Highlighter,
    }

    impl Markdown {
        /// Return the lines which some markdown is shown as, wrapped to a width.
        pub fn render(&mut self, text: &str, columns: usize) -> Vec<Yarn> {
            if columns == 0 {
                return Vec::new();
            }

            let mut options = Options::empty();
            options.insert(Options::ENABLE_TABLES);
            options.insert(Options::ENABLE_STRIKETHROUGH);

            let mut state = State::new(columns);
            for event in Parser::new_ext(text, options) {
                self.event(&mut state, event);
            }
            state.finish();

            state.lines
        }

        /// Take in one piece of the markdown.
        fn event(&mut self, state: &mut State, event: Event<'_>) {
            match event {
                Event::Text(text) => match state.code.as_mut() {
                    Some(code) => code.push_str(&text),
                    None => state.push(Run {
                        text: text.to_string(),
                        color: state.color,
                        background: None,
                        bold: state.bold,
                    }),
                },
                // Code in the middle of a sentence is set apart by what it is on, the same as a
                // block of it, rather than by a color which means something else.
                Event::Code(code) => {
                    state.push(Run::plain(code.to_string()).on(Color::CodeBackground))
                }
                Event::SoftBreak | Event::HardBreak => state.wrap(),
                Event::Start(tag) => self.start(state, tag),
                Event::End(tag) => self.end(state, tag),
                Event::Rule => {
                    state.wrap();
                    state.push(Run::colored("─".repeat(state.columns), Color::GrayedText));
                    state.wrap();
                }
                _ => {}
            }
        }

        /// Take in the start of something.
        fn start(&mut self, state: &mut State, tag: Tag<'_>) {
            match tag {
                Tag::Paragraph => state.blank(),
                Tag::Heading { level, .. } => {
                    state.blank();
                    state.bold = true;
                    if matches!(level, HeadingLevel::H3 | HeadingLevel::H4) {
                        state.color = Some(Color::LightGrayedText.into());
                    }
                }
                Tag::CodeBlock(kind) => {
                    state.blank();
                    state.language = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(name) => Language::of_name(&name),
                        pulldown_cmark::CodeBlockKind::Indented => None,
                    };
                    state.code = Some(String::new());
                }
                Tag::List(_) => state.depth += 1,
                Tag::Item => {
                    state.wrap();
                    state.indent = state.depth.saturating_sub(1) * INDENT;
                    // The bullet is written in whatever the text around it is, rather than being
                    // picked out in a color of its own.
                    state.push(Run::plain(BULLET));
                }
                Tag::BlockQuote(_) => {
                    state.blank();
                    state.quoted = true;
                }
                Tag::Emphasis | Tag::Strong => state.bold = true,
                Tag::Strikethrough => state.color = Some(Color::GrayedText.into()),
                Tag::Table(alignments) => {
                    state.blank();
                    state.table = Some(Table::new(alignments.len()));
                }
                Tag::TableCell => state.cell = Some(Vec::new()),
                _ => {}
            }
        }

        /// Take in the end of something.
        fn end(&mut self, state: &mut State, tag: TagEnd) {
            match tag {
                TagEnd::Paragraph | TagEnd::Heading(_) => {
                    state.color = None;
                    state.bold = false;
                    state.wrap();
                }
                TagEnd::CodeBlock => self.code_block(state),
                TagEnd::List(_) => {
                    state.depth = state.depth.saturating_sub(1);
                    state.indent = 0;
                }
                TagEnd::Item => state.wrap(),
                TagEnd::BlockQuote(_) => {
                    state.quoted = false;
                    state.wrap();
                }
                TagEnd::Emphasis | TagEnd::Strong => state.bold = false,
                TagEnd::Strikethrough => state.color = None,
                TagEnd::TableCell => {
                    if let (Some(table), Some(cell)) = (state.table.as_mut(), state.cell.take()) {
                        table.cell(cell);
                    }
                }
                TagEnd::TableHead | TagEnd::TableRow => {
                    if let Some(table) = state.table.as_mut() {
                        table.row();
                    }
                }
                TagEnd::Table => {
                    if let Some(table) = state.table.take() {
                        table.write(state);
                    }
                }
                _ => {}
            }
        }

        /// Show the code which a fenced block holds, colored if the language is known.
        fn code_block(&mut self, state: &mut State) {
            let code: String = match state.code.take() {
                Some(code) => code,
                None => return,
            };
            let language: Option<Language> = state.language.take();

            for line in code.trim_end_matches('\n').split('\n') {
                let mut runs: Vec<Run> = match language {
                    Some(language) => self
                        .highlighter
                        .highlight(line, language)
                        .iter()
                        .map(Self::run_of)
                        .collect(),
                    None => vec![Run::plain(line)],
                };

                // The background has to reach across the whole row, or the block ends ragged where
                // each line of code happens to stop.
                let taken: usize = runs.iter().map(|run| Self::width(&run.text)).sum();
                runs.push(Run::plain(" ".repeat(state.columns.saturating_sub(taken))));
                for run in runs.iter_mut() {
                    run.background = Some(Color::CodeBackground.into());
                }

                // Code is not wrapped: a line which is too long is cut off, since folding it makes
                // it harder to read rather than easier.
                state.line(runs);
            }

            state.blank();
        }

        /// Return the piece of code as a run in the color for what it is.
        fn run_of(span: &Span) -> Run {
            return match Color::of_code(span.kind) {
                Some(color) => Run::colored(span.text.as_str(), color),
                None => Run::plain(span.text.as_str()),
            };
        }

        /// Return how many columns some text takes up.
        pub(super) fn width(text: &str) -> usize {
            Cell::columns(text)
        }
    }

    /// A table which is being read.
    struct Table {
        /// The rows, each of which is a list of cells, each of which is a list of runs.
        rows: Vec<Vec<Vec<Run>>>,
        /// The cells of the row being read.
        row: Vec<Vec<Run>>,
        /// How many columns the table has.
        columns: usize,
    }

    impl Table {
        /// Return a new table with the given number of columns.
        fn new(columns: usize) -> Self {
            Self {
                rows: Vec::new(),
                row: Vec::new(),
                columns,
            }
        }

        /// Take in a cell.
        fn cell(&mut self, cell: Vec<Run>) {
            self.row.push(cell);
        }

        /// Take in the end of a row.
        fn row(&mut self) {
            if self.row.is_empty() {
                return;
            }
            self.rows.push(std::mem::take(&mut self.row));
        }

        /// Write the table out in a box, with every column as wide as the widest thing in it.
        fn write(self, state: &mut State) {
            if self.rows.is_empty() || self.columns == 0 {
                return;
            }

            let mut widths: Vec<usize> = vec![0; self.columns];
            for row in &self.rows {
                for (index, cell) in row.iter().enumerate().take(self.columns) {
                    let width: usize = cell.iter().map(|run| Markdown::width(&run.text)).sum();
                    widths[index] = widths[index].max(width);
                }
            }

            state.line(Self::rule(&widths, CORNERS[0], EDGES[0], CORNERS[1]));

            for (number, row) in self.rows.iter().enumerate() {
                state.line(self.cells(row, &widths));

                // Every row is ruled off from the next, so that a cell which is longer than the
                // rest does not run into the one below it when read.
                if number + 1 < self.rows.len() {
                    state.line(Self::rule(&widths, EDGES[2], CROSS, EDGES[3]));
                }
            }

            state.line(Self::rule(&widths, CORNERS[2], EDGES[1], CORNERS[3]));
            state.blank();
        }

        /// Return a rule across the table, with the given pieces at the ends and between columns.
        fn rule(widths: &[usize], left: &str, between: &str, right: &str) -> Vec<Run> {
            let mut line: String = String::from(left);

            for (index, width) in widths.iter().enumerate() {
                if index > 0 {
                    line.push_str(between);
                }
                line.push_str(&HORIZONTAL.repeat(width + PADDING * 2));
            }
            line.push_str(right);

            vec![Run::colored(line, Color::GrayedText)]
        }

        /// Return a row of cells, each padded out to the width of its column.
        fn cells(&self, row: &[Vec<Run>], widths: &[usize]) -> Vec<Run> {
            let mut runs: Vec<Run> = Vec::new();
            let padding: String = " ".repeat(PADDING);

            for (index, width) in widths.iter().enumerate().take(self.columns) {
                runs.push(Run::colored(VERTICAL, Color::GrayedText));
                runs.push(Run::plain(padding.clone()));

                let cell: &[Run] = row.get(index).map(Vec::as_slice).unwrap_or(&[]);
                let mut taken: usize = 0;
                for run in cell {
                    taken += Markdown::width(&run.text);
                    runs.push(run.clone());
                }

                runs.push(Run::plain(" ".repeat(width.saturating_sub(taken))));
                runs.push(Run::plain(padding.clone()));
            }

            runs.push(Run::colored(VERTICAL, Color::GrayedText));
            runs
        }
    }

    /// What has been read of the markdown so far.
    struct State {
        /// The lines which are finished.
        lines: Vec<Yarn>,
        /// The runs of the line being read.
        runs: Vec<Run>,
        /// How wide the lines are.
        columns: usize,
        /// The color what is being read is written in.
        color: Option<ansi::Color>,
        /// Whether what is being read is written in bold.
        bold: bool,
        /// How far what is being read is indented.
        indent: usize,
        /// How many lists what is being read is inside.
        depth: usize,
        /// Whether what is being read is quoted.
        quoted: bool,
        /// The code of the block being read.
        code: Option<String>,
        /// The language of the block being read.
        language: Option<Language>,
        /// The table being read.
        table: Option<Table>,
        /// The cell being read.
        cell: Option<Vec<Run>>,
    }

    impl State {
        /// Return the state of having read nothing.
        fn new(columns: usize) -> Self {
            Self {
                lines: Vec::new(),
                runs: Vec::new(),
                columns,
                color: None,
                bold: false,
                indent: 0,
                depth: 0,
                quoted: false,
                code: None,
                language: None,
                table: None,
                cell: None,
            }
        }

        /// Add a piece to what is being read.
        fn push(&mut self, run: Run) {
            match self.cell.as_mut() {
                Some(cell) => cell.push(run),
                None => self.runs.push(run),
            }
        }

        /// Finish the line which is being read, wrapping it if it is too long.
        fn wrap(&mut self) {
            if self.runs.is_empty() {
                return;
            }

            let runs: Vec<Run> = std::mem::take(&mut self.runs);
            for line in Self::fold(runs, self.room()) {
                self.line(line);
            }
        }

        /// Put a line down as it is.
        fn line(&mut self, runs: Vec<Run>) {
            let mut all: Vec<Run> = Vec::new();
            if self.quoted {
                all.push(Run::colored(QUOTE, Color::GrayedText));
            }
            if self.indent > 0 {
                all.push(Run::plain(" ".repeat(self.indent)));
            }
            all.extend(runs);

            self.lines.push(Run::yarn(&all, self.columns));
        }

        /// Leave a blank line, unless there is already one or nothing has been written yet.
        fn blank(&mut self) {
            self.wrap();

            if self.lines.is_empty() {
                return;
            }
            if let Some(last) = self.lines.last() {
                if last.cells().iter().all(|cell| *cell == Cell::BLANK) {
                    return;
                }
            }

            self.lines.push(Yarn::blank(self.columns));
        }

        /// Return how many columns there are for words after whatever goes before them.
        fn room(&self) -> usize {
            let taken: usize = self.indent
                + if self.quoted {
                    Markdown::width(QUOTE)
                } else {
                    0
                };
            self.columns.saturating_sub(taken).max(1)
        }

        /// Finish whatever is left over.
        fn finish(&mut self) {
            self.wrap();

            // A reply which ends in a blank line is trimmed, since the line after it is the next
            // thing said rather than part of this.
            while let Some(last) = self.lines.last() {
                if last.cells().iter().all(|cell| *cell == Cell::BLANK) {
                    self.lines.pop();
                } else {
                    break;
                }
            }
        }

        /// Break some runs into lines which fit in a width, keeping words whole.
        fn fold(runs: Vec<Run>, columns: usize) -> Vec<Vec<Run>> {
            let mut lines: Vec<Vec<Run>> = Vec::new();
            let mut line: Vec<Run> = Vec::new();
            let mut taken: usize = 0;

            for run in runs {
                // A run is broken on spaces so that a long one wraps rather than running off.
                for (number, word) in run.text.split(' ').enumerate() {
                    let space: bool = number > 0;

                    // A word with no space in it which is wider than the whole line has nowhere
                    // to wrap, so it is broken where it runs out of room. Leaving it whole would
                    // lose everything past the edge, which is what a long path or address is.
                    for (piece, first) in Self::pieces(word, columns) {
                        let space: bool = space && first;
                        let width: usize = Markdown::width(&piece) + usize::from(space);

                        if taken + width > columns && taken > 0 {
                            lines.push(std::mem::take(&mut line));
                            taken = 0;
                        }

                        let text: String = match space && taken > 0 {
                            true => format!(" {}", piece),
                            false => piece,
                        };
                        if text.is_empty() {
                            continue;
                        }

                        taken += Markdown::width(&text);
                        line.push(Run {
                            text,
                            color: run.color,
                            background: run.background,
                            bold: run.bold,
                        });
                    }
                }
            }

            if !line.is_empty() {
                lines.push(line);
            }

            lines
        }

        /// Break a word into the pieces which fit in a width, saying which is the first of them.
        ///
        /// A word which fits is one piece, which is the usual case. One which does not is cut at
        /// the column it runs out of room at, counting the columns each cluster is written in so
        /// that a wide one is not split down the middle.
        fn pieces(word: &str, columns: usize) -> Vec<(String, bool)> {
            if columns == 0 || Markdown::width(word) <= columns {
                return vec![(word.to_string(), true)];
            }

            let mut pieces: Vec<(String, bool)> = Vec::new();
            let mut piece: String = String::new();
            let mut taken: usize = 0;

            for cell in Cell::all(word) {
                let cluster: String = cell.to_string();
                let width: usize = Markdown::width(&cluster);

                if taken + width > columns {
                    pieces.push((std::mem::take(&mut piece), pieces.is_empty()));
                    taken = 0;
                }

                piece.push_str(&cluster);
                taken += width;
            }

            if !piece.is_empty() {
                pieces.push((piece, pieces.is_empty()));
            }

            pieces
        }
    }
}
pub use markdown::Markdown;

#[cfg(test)]
mod tests {
    use super::Markdown;

    use rend::Yarn;

    use test_case::test_case;

    /// Return the text of the lines which some markdown is shown as.
    fn lines(text: &str, columns: usize) -> Vec<String> {
        Markdown::default()
            .render(text, columns)
            .iter()
            .map(|yarn: &Yarn| {
                yarn.cells()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test_case("", 20; "nothing")]
    #[test_case("hi", 0; "no room")]
    fn test_nothing_to_show(text: &str, columns: usize) {
        assert!(lines(text, columns).is_empty());
    }

    /// A word with nowhere to wrap is broken rather than cut off at the edge.
    ///
    /// A line is truncated to the width it is drawn at, so a long address or path used to lose
    /// everything past the first line of it with nothing to say that it had.
    #[test]
    fn test_a_word_which_does_not_fit_is_broken() {
        let url: &str = "https://example.com/a/very/long/path/which/does/not/fit";

        let lines: Vec<String> = lines(url, 20);

        assert_eq!(
            lines.concat(),
            url,
            "The whole of it has to still be there."
        );
        assert!(
            lines.len() > 1,
            "It has to be broken over more than one line."
        );
        assert!(lines.iter().all(|line| line.chars().count() <= 20));
    }

    /// Breaking a long word does not split a cluster which is written in two columns.
    #[test]
    fn test_a_wide_word_is_broken_between_clusters() {
        let word: String = "🚀".repeat(8);

        let lines: Vec<String> = lines(&word, 5);

        assert_eq!(lines.concat(), word);
        // Two of them are four columns and a third would be six, which is more than there is.
        assert!(lines.iter().all(|line| line.chars().count() <= 2));
    }

    #[test]
    fn test_a_paragraph_is_wrapped() {
        assert_eq!(
            lines("one two three four five", 10),
            vec!["one two", "three four", "five"]
        );
    }

    #[test]
    fn test_emphasis_does_not_show_its_markers() {
        assert_eq!(lines("a **bold** word", 40), vec!["a bold word"]);
    }

    #[test]
    fn test_inline_code_does_not_show_its_markers() {
        assert_eq!(lines("run `ls -l` now", 40), vec!["run ls -l now"]);
    }

    #[test]
    fn test_a_list_is_bulleted() {
        assert_eq!(lines("- one\n- two", 40), vec!["• one", "• two"]);
    }

    #[test]
    fn test_a_code_block_keeps_its_lines() {
        let text: &str = "```rust\nfn main() {\n    let x = 1;\n}\n```";

        assert_eq!(lines(text, 40), vec!["fn main() {", "    let x = 1;", "}"]);
    }

    #[test]
    fn test_a_code_block_in_an_unknown_language_is_still_shown() {
        let text: &str = "```brainfuck\n+++...\n```";

        assert_eq!(lines(text, 40), vec!["+++..."]);
    }

    #[test]
    fn test_a_table_is_ruled_between_every_row() {
        let text: &str = "| a | b |\n| --- | --- |\n| c | d |\n| e | f |";

        assert_eq!(
            lines(text, 40),
            vec![
                "┌───┬───┐",
                "│ a │ b │",
                "├───┼───┤",
                "│ c │ d │",
                "├───┼───┤",
                "│ e │ f │",
                "└───┴───┘",
            ]
        );
    }

    #[test]
    fn test_a_table_is_drawn_in_a_box() {
        let text: &str = "| a | bbbb |\n| --- | --- |\n| cccc | d |";

        assert_eq!(
            lines(text, 40),
            vec![
                "┌──────┬──────┐",
                "│ a    │ bbbb │",
                "├──────┼──────┤",
                "│ cccc │ d    │",
                "└──────┴──────┘",
            ]
        );
    }

    #[test]
    fn test_a_heading_is_shown_without_its_hashes() {
        assert_eq!(lines("# Title", 40), vec!["Title"]);
    }

    #[test]
    fn test_a_quote_is_marked() {
        assert_eq!(lines("> quoted", 40), vec!["▏ quoted"]);
    }

    #[test]
    fn test_paragraphs_are_kept_apart() {
        assert_eq!(lines("one\n\ntwo", 40), vec!["one", "", "two"]);
    }

    #[test]
    fn test_every_line_is_as_wide_as_it_was_asked_for() {
        let text: &str = "# Title\n\nsome words\n\n- a list\n\n```rust\nfn a() {}\n```";

        for yarn in Markdown::default().render(text, 30) {
            assert_eq!(yarn.len(), 30);
        }
    }
}
