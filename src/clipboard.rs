use copypasta::{ClipboardContext as CopyPastaClipboardContext, ClipboardProvider};

pub struct Clipboard {
    context: CopyPastaClipboardContext,
}

/// Manages access to the system clipboard.
impl Clipboard {
    /// Return a new clipboard.
    pub fn new() -> Self {
        let context = CopyPastaClipboardContext::new().unwrap();
        Self { context }
    }

    /// Set the contents of the clipboard.
    pub fn copy(&mut self, contents: String) {
        #[cfg(feature = "logging")]
        log::debug!("Setting the clipboard conents to \"{}\"...", contents);

        #[allow(clippy::redundant_clone)]
        self.context.set_contents(contents.clone()).unwrap();

        // NOTE(ascola): We shouldn't have to do this, but setting contents doesn't seem to work
        // on my laptop running Ubuntu 22.04 without it?
        // See https://github.com/alacritty/copypasta/issues/49
        #[allow(unused_variables)]
        let actual_contents: String = self.context.get_contents().unwrap();

        #[cfg(feature = "logging")]
        if actual_contents != contents {
            log::warn!("Failed to set the clipboard contents.");
        } else {
            log::debug!("Successfully set the clipboard contents.");
        }
    }

    #[allow(dead_code)]
    /// Return the contents of the clipboard.
    pub fn paste(&mut self) -> String {
        return self.context.get_contents().unwrap();
    }
}
