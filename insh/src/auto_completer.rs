/// Provides _completions_ of type `C` for _partial_ input of type `P`.
pub trait AutoCompleter<P, C> {
    fn complete(&mut self, partial: P) -> Option<C>;
}
