//! A message in a conversation.

use serde::Serialize;
use typed_builder::TypedBuilder;

/// Who sent a message.
#[derive(Serialize, Debug, Clone, Copy, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The person using insh.
    User,
    /// The inference engine.
    Assistant,
}

/// A message in a conversation.
#[derive(Serialize, TypedBuilder, Debug, Clone, Eq, PartialEq)]
pub struct Message {
    /// Who sent the message.
    pub role: Role,
    /// What the message says.
    #[builder(setter(into))]
    pub content: String,
}
