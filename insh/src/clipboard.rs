//! The system clipboard.

use copypasta::{ClipboardContext as CopyPastaClipboardContext, ClipboardProvider};

/// The system clipboard.
///
/// There is not always one to be had: a terminal on a machine with no window system running has
/// nothing to put anything on. Copying and pasting then do nothing at all rather than bringing
/// insh down, since neither is ever the thing which was being got on with.
pub struct Clipboard {
    /// The clipboard, which is nothing at all when there is none to be had.
    context: Option<CopyPastaClipboardContext>,
}

impl Clipboard {
    /// Return a new clipboard.
    pub fn new() -> Self {
        let context: Option<CopyPastaClipboardContext> = match CopyPastaClipboardContext::new() {
            Ok(context) => Some(context),
            Err(error) => {
                #[cfg(feature = "logging")]
                log::warn!("Failed to reach the clipboard: {}", error);
                #[cfg(not(feature = "logging"))]
                let _ = error;

                None
            }
        };

        Self { context }
    }

    /// Set the contents of the clipboard.
    pub fn copy(&mut self, contents: String) {
        #[cfg(feature = "logging")]
        log::debug!("Setting the clipboard contents to \"{}\"...", contents);

        let context: &mut CopyPastaClipboardContext = match &mut self.context {
            Some(context) => context,
            None => return,
        };

        #[allow(clippy::redundant_clone)]
        if let Err(error) = context.set_contents(contents.clone()) {
            #[cfg(feature = "logging")]
            log::warn!("Failed to set the clipboard contents: {}", error);
            #[cfg(not(feature = "logging"))]
            let _ = error;

            return;
        }

        // NOTE(ascola): We shouldn't have to do this, but setting contents doesn't seem to work
        // on my laptop running Ubuntu 22.04 without it?
        // See https://github.com/alacritty/copypasta/issues/49
        #[allow(unused_variables)]
        let actual_contents: String = context.get_contents().unwrap_or_default();

        #[cfg(feature = "logging")]
        if actual_contents != contents {
            log::warn!("Failed to set the clipboard contents.");
        } else {
            log::debug!("Successfully set the clipboard contents.");
        }
    }

    #[allow(dead_code)]
    /// Return the contents of the clipboard, which is nothing when there is no clipboard to read.
    pub fn paste(&mut self) -> String {
        let context: &mut CopyPastaClipboardContext = match &mut self.context {
            Some(context) => context,
            None => return String::new(),
        };

        return context.get_contents().unwrap_or_default();
    }
}
