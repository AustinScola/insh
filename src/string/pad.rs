use crate::misc::align::Align;

mod pad_options {
    use super::{
        PadCenterOptions, PadCenterOptionsBuilder, PadLeftOptions, PadLeftOptionsBuilder,
        PadRightOptions, PadRightOptionsBuilder,
    };
    use crate::misc::align::Align;

    pub struct PadOptions {
        width: usize,
        align: Align,
        pad_char: String,
    }

    impl PadOptions {
        pub fn builder() -> Builder {
            Builder::new()
        }

        pub fn align(&self) -> Align {
            self.align
        }

        pub fn pad_left_options(&self) -> PadLeftOptions {
            let mut builder: PadLeftOptionsBuilder = PadLeftOptions::builder().width(self.width);
            if self.pad_char != " " {
                builder = builder.pad_char(&self.pad_char);
            }
            builder.build()
        }

        pub fn pad_center_options(&self) -> PadCenterOptions {
            let mut builder: PadCenterOptionsBuilder =
                PadCenterOptions::builder().width(self.width);
            if self.pad_char != " " {
                builder = builder.pad_char(&self.pad_char);
            }
            builder.build()
        }

        pub fn pad_right_options(&self) -> PadRightOptions {
            let mut builder: PadRightOptionsBuilder = PadRightOptions::builder().width(self.width);
            if self.pad_char != " " {
                builder = builder.pad_char(&self.pad_char);
            }
            builder.build()
        }
    }

    mod builder {
        use super::PadOptions;
        use crate::misc::align::Align;

        pub struct Builder {
            width: Option<usize>,
            align: Align,
            pad_char: String,
        }

        impl Builder {
            pub fn new() -> Self {
                Self {
                    width: None,
                    align: Align::Left,
                    pad_char: " ".to_string(),
                }
            }

            pub fn width(mut self, width: usize) -> Self {
                self.width = Some(width);
                self
            }

            pub fn align(mut self, align: Align) -> Self {
                self.align = align;
                self
            }

            #[allow(dead_code)]
            pub fn pad_char(mut self, pad_char: &str) -> Self {
                self.pad_char = pad_char.to_string();
                self
            }

            pub fn build(self) -> PadOptions {
                let Self {
                    width,
                    align,
                    pad_char
                } = self;

                PadOptions {
                    width: width.expect("Width not set."),
                    align,
                    pad_char,
                }
            }
        }
    }
    pub use builder::Builder;
}
pub use pad_options::{Builder as PadOptionsBuilder, PadOptions};

mod pad_left_options {
    pub struct PadLeftOptions {
        width: usize,
        pad_char: String,
    }

    impl PadLeftOptions {
        pub fn builder() -> Builder {
            Builder::new()
        }

        pub fn width(&self) -> usize {
            self.width
        }

        pub fn pad_char(&self) -> &str {
            &self.pad_char
        }
    }

    mod builder {
        use super::PadLeftOptions;

        pub struct Builder {
            width: Option<usize>,
            pad_char: String,
        }

        impl Builder {
            pub fn new() -> Self {
                Self {
                    width: None,
                    pad_char: " ".to_string(),
                }
            }

            pub fn width(mut self, width: usize) -> Self {
                self.width = Some(width);
                self
            }

            pub fn pad_char(mut self, pad_char: &str) -> Self {
                self.pad_char = pad_char.to_string();
                self
            }

            pub fn build(self) -> PadLeftOptions {
                let Self { width, pad_char } = self;

                PadLeftOptions {
                    width: width.expect("Width not set."),
                    pad_char,
                }
            }
        }
    }
    pub use builder::Builder;
}
pub use pad_left_options::{Builder as PadLeftOptionsBuilder, PadLeftOptions};

mod pad_center_options {
    pub struct PadCenterOptions {
        width: usize,
        pad_char: String,
    }

    impl PadCenterOptions {
        pub fn builder() -> Builder {
            Builder::new()
        }

        pub fn width(&self) -> usize {
            self.width
        }

        pub fn pad_char(&self) -> &str {
            &self.pad_char
        }
    }

    mod builder {
        use super::PadCenterOptions;

        pub struct Builder {
            width: Option<usize>,
            pad_char: String,
        }

        impl Builder {
            pub fn new() -> Self {
                Self {
                    width: None,
                    pad_char: " ".to_string(),
                }
            }

            pub fn width(mut self, width: usize) -> Self {
                self.width = Some(width);
                self
            }

            pub fn pad_char(mut self, pad_char: &str) -> Self {
                self.pad_char = pad_char.to_string();
                self
            }

            pub fn build(self) -> PadCenterOptions {
                let Self { width, pad_char } = self;

                PadCenterOptions {
                    width: width.expect("Width not set."),
                    pad_char,
                }
            }
        }
    }
    pub use builder::Builder;
}
pub use pad_center_options::{Builder as PadCenterOptionsBuilder, PadCenterOptions};

mod pad_right_options {
    pub struct PadRightOptions {
        width: usize,
        pad_char: String,
    }

    impl PadRightOptions {
        pub fn builder() -> Builder {
            Builder::new()
        }

        pub fn width(&self) -> usize {
            self.width
        }

        pub fn pad_char(&self) -> &str {
            &self.pad_char
        }
    }

    mod builder {
        use super::PadRightOptions;

        pub struct Builder {
            width: Option<usize>,
            pad_char: String,
        }

        impl Builder {
            pub fn new() -> Self {
                Self {
                    width: None,
                    pad_char: " ".to_string(),
                }
            }

            pub fn width(mut self, width: usize) -> Self {
                self.width = Some(width);
                self
            }

            pub fn pad_char(mut self, pad_char: &str) -> Self {
                self.pad_char = pad_char.to_string();
                self
            }

            pub fn build(self) -> PadRightOptions {
                let Self { width, pad_char } = self;

                PadRightOptions {
                    width: width.expect("Width not set."),
                    pad_char,
                }
            }
        }
    }
    pub use builder::Builder;
}
pub use pad_right_options::{Builder as PadRightOptionsBuilder, PadRightOptions};

pub trait Pad {
    fn pad(&self, options: PadOptions) -> String;
}

impl Pad for String {
    fn pad(&self, options: PadOptions) -> String {
        return match options.align() {
            Align::Left => self.pad_left(options.pad_left_options()),
            Align::Center => self.pad_center(options.pad_center_options()),
            Align::Right => self.pad_right(options.pad_right_options()),
        };
    }
}

pub trait PadLeft {
    fn pad_left(&self, options: PadLeftOptions) -> String;
}

impl PadLeft for String {
    fn pad_left(&self, options: PadLeftOptions) -> String {
        let mut padded = self.clone();
        let pad_len = options.width().saturating_sub(self.len());
        padded.extend(std::iter::repeat(options.pad_char()).take(pad_len));
        padded
    }
}

pub trait PadCenter {
    fn pad_center(&self, options: PadCenterOptions) -> String;
}

impl PadCenter for String {
    fn pad_center(&self, options: PadCenterOptions) -> String {
        let len = self.to_string().len();
        let pad_len = options.width().saturating_sub(len);
        let left_pad = pad_len / 2;
        let right_pad = pad_len - left_pad;

        let mut padded = String::new();
        padded.extend(std::iter::repeat(options.pad_char()).take(left_pad));
        padded.push_str(&self.to_string());
        padded.extend(std::iter::repeat(options.pad_char()).take(right_pad));
        padded
    }
}

pub trait PadRight {
    fn pad_right(&self, options: PadRightOptions) -> String;
}

impl PadRight for String {
    fn pad_right(&self, options: PadRightOptions) -> String {
        let len = self.to_string().len();
        let pad_len = options.width().saturating_sub(len);
        std::iter::repeat(options.pad_char()).take(pad_len).collect::<String>() + &self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case("foo", PadOptions::builder().width(5).align(Align::Left).build(), "foo  "; "left align foo with a width of 5")]
    #[test_case("foo", PadOptions::builder().width(5).align(Align::Center).build(), " foo "; "center align foo with a width of 5")]
    #[test_case("foo", PadOptions::builder().width(5).align(Align::Right).build(), "  foo"; "right align foo with a width of 5")]
    #[test_case("42", PadOptions::builder().width(4).pad_char("0").align(Align::Center).build(), "0420"; "center 42 with 0's and a width of 4")]
    fn test_pad(string: &str, options: PadOptions, expected_result: &str) {
        let result: String = string.to_string().pad(options);

        assert_eq!(result, expected_result);
    }


    #[test_case("", PadLeftOptions::builder().width(3).build(), "   "; "an empty string with a width of 5")]
    #[test_case("foo", PadLeftOptions::builder().width(5).build(), "foo  "; "foo with a width of 5")]
    fn test_pad_left(string: &str, options: PadLeftOptions, expected_result: &str) {
        let result: String = string.to_string().pad_left(options);

        assert_eq!(result, expected_result);
    }

    #[test_case("", PadCenterOptions::builder().width(3).build(), "   "; "an empty string with a width of 5")]
    #[test_case("a", PadCenterOptions::builder().width(4).build(), " a  "; "center aligning prefers the left")]
    #[test_case("foo", PadCenterOptions::builder().width(5).build(), " foo "; "foo with a width of 5")]
    fn test_pad_center(string: &str, options: PadCenterOptions, expected_result: &str) {
        let result: String = string.to_string().pad_center(options);

        assert_eq!(result, expected_result);
    }

    #[test_case("", PadRightOptions::builder().width(3).build(), "   "; "an empty string with a width of 5")]
    #[test_case("foo", PadRightOptions::builder().width(5).build(), "  foo"; "foo with a width of 5")]
    fn test_pad_right(string: &str, options: PadRightOptions, expected_result: &str) {
        let result: String = string.to_string().pad_right(options);

        assert_eq!(result, expected_result);
    }
}

//impl<T: Display> Pad for T {
//    fn pad(&self, width: usize, align: Align, pad_char: char) -> String {
//        let len = self.to_string().len();
//        let pad_len = width.saturating_sub(len);
//        let mut padded = self.to_string();
//        if pad_len > 0 {
//            match align {
//                Align::Left => padded.extend(std::iter::repeat(pad_char).take(pad_len)),
//                Align::Center => {
//                    let left_pad = pad_len / 2;
//                    let right_pad = pad_len - left_pad;
//                    padded.extend(std::iter::repeat(pad_char).take(left_pad));
//                    padded.push_str(&self.to_string());
//                    padded.extend(std::iter::repeat(pad_char).take(right_pad));
//                }
//                Align::Right => {
//                    padded = std::iter::repeat(pad_char).take(pad_len).collect::<String>() + &padded;
//                }
//            }
//        }
//        padded
//    }
//}
