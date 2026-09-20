//! A piece of code and what it is.

/// What a piece of code is.
///
/// These are the names which the grammars capture, cut down to the ones worth telling apart when
/// reading. Anything a grammar captures which is not one of these is left plain.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
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

/// The names which are asked of a grammar, in the order the highlights refer to them by.
///
/// Tree-sitter answers with the position in this list rather than the name, so the order here and
/// [`Kind::of_index`] have to agree.
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
    /// Return what the name at the given position in [`NAMES`] means.
    pub(crate) fn of_index(index: usize) -> Option<Self> {
        return match NAMES.get(index)? {
            name if name.starts_with("keyword") => Some(Self::Keyword),
            name if name.starts_with("string") || *name == "escape" => Some(Self::String),
            name if name.starts_with("comment") => Some(Self::Comment),
            name if name.starts_with("number") => Some(Self::Number),
            name if name.starts_with("function") => Some(Self::Function),
            name if name.starts_with("type") || *name == "constructor" => Some(Self::Type),
            name if name.starts_with("constant") || *name == "boolean" || *name == "label" => {
                Some(Self::Constant)
            }
            name if name.starts_with("operator") => Some(Self::Operator),
            name if name.starts_with("punctuation") => Some(Self::Punctuation),
            name if name.starts_with("variable") || *name == "property" => Some(Self::Variable),
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
