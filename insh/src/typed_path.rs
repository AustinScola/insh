/*!
This module contains the [`TypedPath`] struct, which works out which path was typed.
*/
use std::path::{Component, PathBuf, MAIN_SEPARATOR as PATH_SEPARATOR};

use typed_builder::TypedBuilder;

/// What a path which was typed means.
#[derive(TypedBuilder)]
pub struct TypedPath {
    /// The home directory, which a tilde stands for.
    home: Option<PathBuf>,
    /// The directory a relative path is from.
    from: PathBuf,
}

impl TypedPath {
    /// Return the path which was typed, which is always an absolute one.
    pub fn resolve(&self, typed: &str) -> PathBuf {
        let path: PathBuf = match &self.home {
            Some(home) => match typed.strip_prefix('~') {
                Some("") => home.clone(),
                Some(rest) => match rest.strip_prefix(PATH_SEPARATOR) {
                    Some(rest) => home.join(rest),
                    None => PathBuf::from(typed),
                },
                None => PathBuf::from(typed),
            },
            None => PathBuf::from(typed),
        };

        // A relative path is from the given directory. It cannot be left relative, because inshd
        // runs from the root rather than from wherever insh was started, so the two would not
        // agree on what it means.
        let path: PathBuf = match path.is_absolute() {
            true => path,
            false => self.from.join(path),
        };

        // NOTE: Going up is done by taking the last part off rather than by asking the filesystem,
        // so that a directory which is reached through a symbolic link stays that way. This is also
        // what takes off the trailing separator which the directory bar shows.
        let mut resolved = PathBuf::new();
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    resolved.pop();
                }
                component => resolved.push(component),
            }
        }

        return resolved;
    }
}
