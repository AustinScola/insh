/// Extentioned functionality for `std::string::String` provided via extension traits.

/// Return the `strings` joined together using commas, (an Oxoford comma if necessary), and the
/// `conjuction`.
pub fn conjoin(strings: Vec<String>, conjunction: &str) -> String {
    match strings.len() {
        0 => String::new(),
        1 => strings[0].to_string(),
        2 => format!("{} {} {}", strings[0], conjunction, strings[1]),
        _ => {
            strings[0..strings.len() - 1].join(", ")
                + ", "
                + conjunction
                + " "
                + &strings[strings.len() - 1]
        }
    }
}

/// Return the string with the first letter capitalized (if there is a first letter).
pub fn capitalize_first_letter(string: &str) -> String {
    if string.is_empty() {
        return String::new();
    }
    string[0..1].to_uppercase() + &string[1..]
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(vec![], "and", ""; "joining no strings")]
    #[test_case(vec![String::from("foo")], "and", "foo"; "joining a single string")]
    #[test_case(vec![String::from("foo"), String::from("bar")],"and",  "foo and bar"; "oining two string")]
    #[test_case(vec![String::from("foo"), String::from("bar"), String::from("baz")], "and", "foo, bar, and baz"; "joining three string")]
    fn test_conjoin(strings: Vec<String>, conjunction: &str, expected_result: &str) {
        let result: String = conjoin(strings, conjunction);

        assert_eq!(result, expected_result);
    }

    #[test_case("", ""; "capitalizing the first letter of an empty string")]
    #[test_case("a", "A"; "capitalizing the first letter of a string with one character")]
    #[test_case("A", "A"; "capitalizing the first letter of a string with one character that is already capitalized")]
    #[test_case("foo", "Foo"; "capitalizing the first letter of a string that has one word")]
    #[test_case("Foo", "Foo"; "capitalizing the first letter of a string that has one word with an already capitalized first letter")]
    fn test_capitalize_first_letter(string: &str, expected_result: &str) {
        let result: String = capitalize_first_letter(string);

        assert_eq!(result, expected_result);
    }
}

mod detab {
    pub trait DetabExt {
        fn detab(&self, tab_width: usize) -> String;
    }

    impl DetabExt for String {
        fn detab(&self, tab_width: usize) -> String {
            self.as_str().detab(tab_width)
        }
    }

    impl DetabExt for &str {
        fn detab(&self, tab_width: usize) -> String {
            // If the string is empty, then the result is also empty.
            if self.is_empty() {
                return String::new();
            }

            // The resuling string will be at least as long as the input string, so reserve the
            // capacity for that many characters.
            let mut result: String = String::with_capacity(self.len());

            let mut counter: usize = 0;
            for character in self.chars() {
                match character {
                    '\t' => {
                        let number_of_spaces = tab_width - counter;
                        result.push_str(&(" ".repeat(number_of_spaces)));

                        counter = 0;
                    }
                    _ => {
                        result.push(character);

                        counter += 1;
                        if counter == tab_width {
                            counter = 0;
                        }
                    }
                }
            }

            result
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use test_case::test_case;

        #[test_case("", 4, ""; "an empty string")]
        #[test_case("\t", 4, "    "; "just one tab")]
        #[test_case("\t\t\tfoo", 4, "            foo"; "a couple tabs followed by some text")]
        #[test_case("\tf\t", 4, "    f   "; "a tab, a character, then a tab")]
        #[test_case("\tfoo\t", 4, "    foo "; "a tab, a couple characters, then a tab")]
        #[test_case("\tfoo\tbar", 4, "    foo bar"; "a tab, a couple characters, then a tab, then some more characters")]
        #[test_case("\ttoad\t", 4, "    toad    "; "a tab, some characters the same width as a tab, then a tab")]
        fn test_detab(string: &str, tab_width: usize, expected: &str) {
            let result: String = string.detab(tab_width);

            assert_eq!(result, expected)
        }
    }
}
pub use detab::DetabExt;
