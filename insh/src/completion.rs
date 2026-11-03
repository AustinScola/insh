/*!
This module contains the [`Completion`] struct, which is what has been typed and what it was
suggested it is the start of.
*/
use typed_builder::TypedBuilder;

/// What has been typed, and the whole of what it was suggested it is the start of.
#[derive(TypedBuilder)]
pub struct Completion<'a> {
    /// What has been typed.
    typed: &'a str,
    /// What it was suggested it is the start of.
    suggested: &'a str,
    /// The characters which the parts of what is typed are separated by.
    separators: &'a [char],
    /// Whether a separator with a backslash before it is part of what it is in rather than the end
    /// of it, the way a space is in a word split up the way a shell splits one.
    #[builder(default)]
    escapable: bool,
}

impl Completion<'_> {
    /// Return what has been typed with the next part of what was suggested on the end of it, which
    /// is nothing at all when what was suggested is not more of what has been typed.
    ///
    /// The next part runs up to and takes in the first separator, so that the part after that is
    /// the next one taken. A part which has been typed all of but its separator is finished with
    /// just that, which is what makes `~` a part on its own.
    pub fn next_part(&self) -> Option<String> {
        let rest: &str = self.suggested.strip_prefix(self.typed)?;
        if rest.is_empty() {
            return None;
        }

        let mut escaped: bool = self.escapable && self.typed.ends_with('\\');
        for (at, character) in rest.char_indices() {
            if !escaped && self.separators.contains(&character) {
                let end: usize = at + character.len_utf8();
                return Some(format!("{}{}", self.typed, &rest[..end]));
            }
            escaped = self.escapable && !escaped && character == '\\';
        }

        return Some(self.suggested.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use test_case::test_case;

    #[test_case("~/fo", "~/foo/bar/baz/", Some("~/foo/"); "the rest of the part being typed")]
    #[test_case("~/foo", "~/foo/bar/baz/", Some("~/foo/"); "a part which only needs its separator")]
    #[test_case("~", "~/foo/", Some("~/"); "the home directory, which is a part on its own")]
    #[test_case("~/foo/bar/", "~/foo/bar/baz", Some("~/foo/bar/baz"); "the last part, which has no separator after it")]
    #[test_case("~/foo/", "~/foo/", None; "nothing left")]
    #[test_case("/data", "~/foo/", None; "a suggestion which is not more of what was typed")]
    fn test_next_part(typed: &str, suggested: &str, expected: Option<&str>) {
        let completion = Completion::builder()
            .typed(typed)
            .suggested(suggested)
            .separators(&['/'])
            .build();

        assert_eq!(completion.next_part().as_deref(), expected);
    }

    #[test_case("set-d", "set-dir ~/foo/", Some("set-dir "); "the command, before any of what comes after it")]
    #[test_case("set-dir ", "set-dir ~/foo/", Some("set-dir ~/"); "the first part of the directory")]
    #[test_case("set-dir ~/my", "set-dir ~/my\\ dir/", Some("set-dir ~/my\\ dir/"); "a space with a backslash before it, which is not a separator")]
    fn test_next_part_of_a_command(typed: &str, suggested: &str, expected: Option<&str>) {
        let completion = Completion::builder()
            .typed(typed)
            .suggested(suggested)
            .separators(&[' ', '/'])
            .escapable(true)
            .build();

        assert_eq!(completion.next_part().as_deref(), expected);
    }
}
