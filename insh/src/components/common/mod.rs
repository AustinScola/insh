//! Common components.

mod dir;
pub use dir::{Dir, Event as DirEvent, Props as DirProps};

mod footer;
pub use footer::{Footer, Info as FooterInfo, Props as FooterProps};

mod phrase;
pub use phrase::{Effect as PhraseEffect, Event as PhraseEvent, Phrase, Props as PhraseProps};
