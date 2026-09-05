//! Helpers for querying the database.

/// Return a `LIKE` pattern which matches the strings that start with a prefix.
///
/// The characters which are special to `LIKE` are escaped with a backslash, which is the escape
/// character that `LIKE` uses when it is not given another one.
pub fn like_prefix_pattern(prefix: &str) -> String {
    let mut pattern: String = String::with_capacity(prefix.len() + 1);

    for character in prefix.chars() {
        if matches!(character, '\\' | '%' | '_') {
            pattern.push('\\');
        }
        pattern.push(character);
    }
    pattern.push('%');

    return pattern;
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case("foo", "foo%"; "a phrase with nothing to escape")]
    #[test_case("", "%"; "an empty phrase")]
    #[test_case("50%", r"50\%%"; "a percent sign")]
    #[test_case("foo_bar", r"foo\_bar%"; "an underscore")]
    #[test_case(r"a\b", r"a\\b%"; "a backslash")]
    #[test_case("%_", r"\%\_%"; "only special characters")]
    fn test_like_prefix_pattern(prefix: &str, expected: &str) {
        assert_eq!(like_prefix_pattern(prefix), expected);
    }
}
