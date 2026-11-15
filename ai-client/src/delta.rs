//! What one event of a reply adds to it.

/// What one event of a reply adds to it.
///
/// This is finer than a [`Chunk`](super::chunk::Chunk). Some of it is only worth anything once it
/// has been put together with what comes after it, like the input of a tool call, which arrives a
/// few characters at a time.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Delta {
    /// Text to append to the reply.
    Text(String),
    /// Text to append to what the engine is thinking.
    Thinking {
        /// Which block of the reply the thinking is in, if it is kept to be sent back.
        index: Option<usize>,
        /// The text.
        text: String,
    },
    /// What the engine checks a block of thinking against.
    Signature {
        /// Which block of the reply it signs.
        index: usize,
        /// The signature.
        signature: String,
    },
    /// Thinking which was encrypted before it was sent.
    Redacted {
        /// Which block of the reply it is.
        index: usize,
        /// The encrypted thinking.
        data: String,
    },
    /// A piece of the engine asking to use a tool.
    ToolCall {
        /// Which call this is a piece of.
        index: usize,
        /// What the engine calls this use of the tool, which comes in the first piece.
        id: Option<String>,
        /// The name of the tool, which comes in the first piece.
        name: Option<String>,
        /// JSON to append to what the tool is given.
        input: String,
    },
    /// How many tokens the reply came to.
    Tokens {
        /// How many tokens were written in all, thinking included.
        total: usize,
        /// How many of them were the engine thinking rather than answering.
        thinking: usize,
    },
    /// The end of the reply.
    Done,
}
