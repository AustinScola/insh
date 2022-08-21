/// String helper methods.

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
