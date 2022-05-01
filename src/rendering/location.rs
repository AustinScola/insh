#[derive(Default)]
pub struct Location {
    pub row: usize,
    pub column: usize,
}

impl Location {
    #[allow(dead_code)]
    pub fn new(row: usize, column: usize) -> Self {
        Location { row, column }
    }
}
