#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Size {
    pub rows: usize,
    pub columns: usize,
}

impl Size {
    pub fn new(rows: usize, columns: usize) -> Self {
        Size { rows, columns }
    }
}

impl From<(u16, u16)> for Size {
    fn from(tuple: (u16, u16)) -> Self {
        let columns: usize = tuple.0.into();
        let rows: usize = tuple.1.into();
        Size { columns, rows }
    }
}
