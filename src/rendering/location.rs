pub struct Location {
    pub row: usize,
    pub column: usize,
}

impl Location {
    pub fn new(row: usize, column: usize) -> Self {
        Location { row, column }
    }
}

impl Default for Location {
    fn default() -> Self {
        Location { row: 0, column: 0 }
    }
}
