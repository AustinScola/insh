use crate::rendering::Yarn;

#[allow(dead_code)]
pub struct JoinAndWrap<'a> {
    yarns: Box<dyn Iterator<Item = Yarn> + 'a>,
    wrap: usize,
}

impl JoinAndWrap<'_> {
    pub fn builder() -> Builder<(), ()> {
        Builder::new()
    }
}

impl IntoIterator for JoinAndWrap<'_> {
    type Item = Yarn;
    type IntoIter = JoinAndWrapIter;

    fn into_iter(self) -> Self::IntoIter {
        todo!()
    }
}

pub struct JoinAndWrapIter {}

impl Iterator for JoinAndWrapIter {
    type Item = Yarn;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

mod builder {
    use super::JoinAndWrap;
    use crate::rendering::Yarn;

    pub struct Builder<A, B> {
        yarns: A,
        wrap: B,
    }

    impl<'a> Builder<(), ()> {
        pub fn new() -> Self {
            Self {
                yarns: (),
                wrap: (),
            }
        }

        pub fn yarns(
            self,
            yarns: impl Iterator<Item = Yarn> + 'a,
        ) -> Builder<Box<dyn Iterator<Item = Yarn> + 'a>, ()> {
            Builder {
                yarns: Box::new(yarns),
                wrap: self.wrap,
            }
        }

        #[allow(dead_code)]
        pub fn wrap(self, wrap: usize) -> Builder<(), usize> {
            Builder {
                yarns: self.yarns,
                wrap,
            }
        }
    }

    impl<'a> Builder<(), usize> {
        #[allow(dead_code)]
        pub fn yarns(
            self,
            yarns: impl Iterator<Item = Yarn> + 'a,
        ) -> Builder<Box<dyn Iterator<Item = Yarn> + 'a>, usize> {
            Builder {
                yarns: Box::new(yarns),
                wrap: self.wrap,
            }
        }
    }

    impl<'a> Builder<Box<dyn Iterator<Item = Yarn> + 'a>, ()> {
        pub fn wrap(self, wrap: usize) -> Builder<Box<dyn Iterator<Item = Yarn> + 'a>, usize> {
            Builder {
                yarns: self.yarns,
                wrap,
            }
        }
    }

    impl<'a> Builder<Box<dyn Iterator<Item = Yarn> + 'a>, usize> {
        pub fn build(self) -> JoinAndWrap<'a> {
            JoinAndWrap {
                yarns: self.yarns,
                wrap: self.wrap,
            }
        }
    }
}
use builder::Builder;
