/*!
This module contains the [`current_dir`] helper function for returning the current directory.
*/
use std::env;
use std::path::PathBuf;

/// Return the current directory.
///
/// If the environment variable `PWD` is set and if it is a valid path, then return that. Otherwise,
/// use the Rust standard library function for getting the current directory.
///
/// NOTE: On Linux, the Rust standard library function calls `getcwd()` which canonicalizes the path
/// by resolving dots, dot-dots, and symbolic links. Prefer `PWD` over the std lib `current_dir`
/// function because we don't want to resolve symlinks.
pub fn current_dir() -> PathBuf {
    if let Ok(pwd) = env::var("PWD") {
        return PathBuf::from(pwd);
    }
    env::current_dir().unwrap() // arbsego: ignore
}
