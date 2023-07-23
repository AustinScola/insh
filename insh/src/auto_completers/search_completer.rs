/// Provides suggestions for searches.
use crate::auto_completer::AutoCompleter;
use crate::data::Data;

#[cfg(feature = "logging")]
use std::time::Instant;

/// Provides suggestions for searches.
pub struct SearchCompleter {}

impl SearchCompleter {
    pub fn new() -> Self {
        Self {}
    }
}

impl AutoCompleter<String, String> for SearchCompleter {
    /// For now, the completion strategy is to suggest the most recent search that starts with the
    /// partial string.
    fn complete(&mut self, partial: String) -> Option<String> {
        #[cfg(feature = "logging")]
        let start = Instant::now();

        // NOTE: We might not want to read data from disk each call because this could be slow.
        let data: Data = Data::read();
        let mut searches: Vec<String> = data.searcher.history.into();

        // Searches are stored oldest to newest so we want to iterate in reverse.
        searches.reverse();
        for search in searches.iter() {
            if search.starts_with(&partial) {
                #[cfg(feature = "logging")]
                {
                    let duration = start.elapsed();
                    log::debug!("Found search completion in {}ms.", duration.as_millis());
                }

                return Some(search.to_string());
            }
        }

        None
    }
}
