use crate::field::Field;

pub struct Str {
    value: String,
}

impl Str {
    pub fn starts_with(&self, string: &str) -> bool {
        self.value.starts_with(string)
    }
}

impl Field for Str {}

impl From<Str> for String {
    fn from(str: Str) -> String {
        return str.value.clone();
    }
}

impl From<String> for Str {
    fn from(string: String) -> Str {
        Str { value: string }
    }
}

impl From<&str> for Str {
    fn from(string: &str) -> Str {
        Str {
            value: String::from(string),
        }
    }
}
