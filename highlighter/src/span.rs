//! A piece of code and what it is.

/// What a piece of code is.
///
/// These are the names which the grammars capture, cut down to the ones worth telling apart when
/// reading. Anything a grammar captures which is not one of these is left plain.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Kind {
    /// A word which is part of the language, like `if` or `fn`.
    Keyword,
    /// Text in quotes.
    String,
    /// A note to whoever is reading rather than to the machine.
    Comment,
    /// A number.
    Number,
    /// The name of a function, where it is called or where it is written.
    Function,
    /// The name of a type.
    Type,
    /// A value which is built into the language, like `true`.
    Constant,
    /// A symbol which does something, like `+`.
    Operator,
    /// A symbol which holds the code together, like a bracket.
    Punctuation,
    /// The name of a variable.
    Variable,
    /// Something attached to a piece of code, like a Rust attribute or an HTML attribute.
    Attribute,
}

/// The names which grammars capture that are worth telling apart.
///
/// A name a grammar captures means whichever of these it matches best (see [`Kind::of_capture`]),
/// and one which matches none of them is left plain.
pub(crate) const NAMES: [&str; 31] = [
    "keyword",
    "keyword.function",
    "keyword.return",
    "keyword.operator",
    "string",
    "string.special",
    "comment",
    "number",
    "function",
    "function.method",
    "function.macro",
    "function.builtin",
    "type",
    "type.builtin",
    "constructor",
    "constant",
    "constant.builtin",
    "boolean",
    "operator",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "property",
    "attribute",
    "comment.documentation",
    "string.special.key",
    "escape",
    "label",
];

impl Kind {
    /// Return what a name which a grammar captures means.
    ///
    /// This is matched the way tree-sitter's own highlighting matches it: the name in [`NAMES`]
    /// which is meant is the one with the most parts, all of which are parts of the name captured.
    /// So `function.method.call` is a `function.method`.
    pub(crate) fn of_capture(name: &str) -> Option<Self> {
        let parts: Vec<&str> = name.split('.').collect();

        let mut best: Option<(&str, usize)> = None;
        for known in NAMES {
            let mut count: usize = 0;
            let matches: bool = known.split('.').all(|part| {
                count += 1;
                parts.contains(&part)
            });
            if matches && best.is_none_or(|(_, most)| count > most) {
                best = Some((known, count));
            }
        }

        return Self::of_name(best?.0);
    }

    /// Return what one of the [`NAMES`] means.
    fn of_name(name: &str) -> Option<Self> {
        return match name {
            name if name.starts_with("keyword") => Some(Self::Keyword),
            name if name.starts_with("string") || name == "escape" => Some(Self::String),
            name if name.starts_with("comment") => Some(Self::Comment),
            name if name.starts_with("number") => Some(Self::Number),
            name if name.starts_with("function") => Some(Self::Function),
            name if name.starts_with("type") || name == "constructor" => Some(Self::Type),
            name if name.starts_with("constant") || name == "boolean" || name == "label" => {
                Some(Self::Constant)
            }
            name if name.starts_with("operator") => Some(Self::Operator),
            name if name.starts_with("punctuation") => Some(Self::Punctuation),
            name if name.starts_with("variable") || name == "property" => Some(Self::Variable),
            name if name.starts_with("attribute") => Some(Self::Attribute),
            _ => None,
        };
    }
}

/// A piece of code and what it is.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Span {
    /// The code.
    pub text: String,
    /// What it is, when it is anything in particular.
    pub kind: Option<Kind>,
}

impl Span {
    /// Return the pieces of the line which the runs say.
    pub fn all(line: &str, runs: &[Run]) -> Vec<Self> {
        let mut characters = line.chars();
        return runs
            .iter()
            .map(|run| Self {
                text: characters.by_ref().take(run.chars).collect(),
                kind: run.kind,
            })
            .collect();
    }
}

/// Characters in a row on a line of code which are all the same thing.
///
/// This is a [`Span`] without its text, for whoever has the text already, such as a client being
/// sent the lines of a file along with what each piece of them is.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Run {
    /// How many characters there are.
    pub chars: usize,
    /// What they are, when they are anything in particular.
    pub kind: Option<Kind>,
}
