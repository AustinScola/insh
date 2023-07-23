mod browser;
mod contents;

pub use browser::{Browser, Effect as BrowserEffect, Event as BrowserEvent, Props as BrowserProps};
use contents::{
    Contents, Effect as ContentsEffect, Event as ContentsEvent, Props as ContentsProps,
};
