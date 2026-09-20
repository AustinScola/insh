//! A piece of a reply.

/// A piece of a reply.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Chunk {
    /// Text to append to the reply.
    Text(String),
    /// Text to append to what the engine is thinking before it answers.
    Thinking(String),
    /// How many tokens the reply came to, which the engine says near the end of it.
    Tokens {
        /// How many tokens were written in all, thinking included.
        total: usize,
        /// How many of them were the engine thinking rather than answering.
        thinking: usize,
    },
    /// The end of the reply.
    Done,
}
