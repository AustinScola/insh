use crate::rendering::{Fabric, Size, Yarn};

pub struct VerticallyCentered {
    #[allow(dead_code)]
    yarns: Box<dyn Iterator<Item = Yarn>>,
}

impl VerticallyCentered {
    pub fn new(yarns: impl Iterator<Item = Yarn> + 'static) -> Self {
        Self {
            yarns: Box::new(yarns),
        }
    }

    pub fn into_fabric(self, _size: Size) -> Fabric {
        todo!()
    }
}
