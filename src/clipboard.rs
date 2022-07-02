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
        self.context.set_contents(contents).unwrap();
    }

    #[allow(dead_code)]
    /// Return the contents of the clipboard.
    pub fn paste(&mut self) -> String {
        return self.context.get_contents().unwrap();
    }
}
