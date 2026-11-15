//! A page which a search found.

use std::fmt::{Display, Error as FmtError, Formatter};

use typed_builder::TypedBuilder;

/// A page which a search found.
#[derive(TypedBuilder, Debug, Clone, Eq, PartialEq)]
pub struct SearchResult {
    /// Where the page is.
    #[builder(setter(into))]
    pub url: String,
    /// The title of the page.
    #[builder(default)]
    pub title: Option<String>,
    /// When the page was published, as the provider says it.
    #[builder(default)]
    pub published: Option<String>,
    /// The passages of the page which have most to do with what was searched for.
    #[builder(default)]
    pub highlights: Vec<String>,
}

impl Display for SearchResult {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), FmtError> {
        if let Some(title) = &self.title {
            writeln!(formatter, "Title: {}", title)?;
        }
        write!(formatter, "URL: {}", self.url)?;
        if let Some(published) = &self.published {
            write!(formatter, "\nPublished: {}", published)?;
        }
        for highlight in &self.highlights {
            write!(formatter, "\n\n{}", highlight)?;
        }
        Ok(())
    }
}
