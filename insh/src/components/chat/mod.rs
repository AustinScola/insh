//! The chat.

mod chat;
pub use chat::{Chat, Effect as ChatEffect, Props as ChatProps};

mod input;
pub use input::{Effect as InputEffect, Event as InputEvent, Input, Props as InputProps};

mod transcript;
pub use transcript::{Effect as TranscriptEffect, Event as TranscriptEvent, Transcript};
