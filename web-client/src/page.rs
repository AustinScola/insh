//! A page which was read.

use std::fmt::{Display, Error as FmtError, Formatter};

use typed_builder::TypedBuilder;

/// A page which was read.
#[derive(TypedBuilder, Debug, Clone, Eq, PartialEq)]
pub struct Page {
    /// Where the page is.
    #[builder(setter(into))]
    pub url: String,
    /// The title of the page.
    #[builder(default)]
    pub title: Option<String>,
    /// When the page was published, as the provider says it.
    #[builder(default)]
    pub published: Option<String>,
    /// What the page says, or as much of it as was asked for.
    #[builder(default, setter(into))]
    pub text: String,
}

impl Display for Page {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        if let Some(title) = &self.title {
            writeln!(formatter, "Title: {}", title)?;
        }
        write!(formatter, "URL: {}", self.url)?;
        if let Some(published) = &self.published {
            write!(formatter, "\nPublished: {}", published)?;
        }
        if !self.text.is_empty() {
            write!(formatter, "\n\n{}", self.text)?;
        }
        Ok(())
    }
}
